use nirvana_entity::connection::Entity as ConnectionEntity;
use nirvana_entity::profile::Entity as Profile;
use nirvana_entity::{connection, credential, jira_settings, profile, ticket};
use sea_orm::{
    ActiveModelTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set, SqlErr,
    TransactionTrait,
};
use tracing::info;

use crate::domain::{Connection, ConnectionData};
use crate::errors::{DbError, NirvanaError, TrackingError};
use crate::integration;
use crate::secrets::{SecretStore, delete_from_keychain, read_credential, store_secret};
use crate::service::Nirvana;

impl Nirvana {
    pub async fn list_connections(&self) -> Result<Vec<Connection>, NirvanaError> {
        ConnectionEntity::find()
            .find_also_related(jira_settings::Entity)
            .order_by_asc(connection::COLUMN.id)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|(model, jira_settings)| Ok(Connection::new(model, jira_settings)?))
            .collect()
    }

    pub async fn get_active_connection(&self) -> Result<Option<Connection>, NirvanaError> {
        let Some(connection_id) = self.config.core.active_connection else {
            return Ok(None);
        };

        Ok(Some(self.get_connection(connection_id).await?))
    }

    pub fn active_connection(&self) -> Option<i64> {
        self.config.core.active_connection
    }

    pub async fn set_active_connection(&mut self, id: i64) -> Result<(), NirvanaError> {
        if (ConnectionEntity::find_by_id(id).one(&self.db).await?).is_none() {
            return Err(TrackingError::ConnectionNotFound(id).into());
        }

        self.config.core.active_connection = Some(id);
        self.config.save(&self.paths)?;
        Ok(())
    }

    pub async fn delete_connection(&mut self, id: i64) -> Result<(), NirvanaError> {
        if (ConnectionEntity::find_by_id(id).one(&self.db).await?).is_none() {
            return Err(TrackingError::ConnectionNotFound(id).into());
        }

        let tickets = ticket::Entity::find()
            .filter(ticket::COLUMN.connection_id.eq(id))
            .count(&self.db)
            .await?;
        if tickets > 0 {
            return Err(TrackingError::ConnectionHasHistory.into());
        }

        let credential = credential::Entity::find_by_id(id).one(&self.db).await?;
        ConnectionEntity::delete_by_id(id).exec(&self.db).await?;
        if let Some(credential) = credential
            && credential.secret_store == SecretStore::Keyring
        {
            delete_from_keychain(credential.credential).await;
        }

        if self.config.core.active_connection == Some(id) {
            self.config.core.active_connection = (ConnectionEntity::find()
                .order_by_asc(connection::COLUMN.id)
                .one(&self.db)
                .await?)
                .map(|m| m.id);
            self.config.save(&self.paths)?;
        }

        Ok(())
    }

    pub async fn add_connection(
        &self,
        connection: ConnectionData,
    ) -> Result<Connection, NirvanaError> {
        let profile = Profile::find()
            .order_by_asc(profile::COLUMN.id)
            .one(&self.db)
            .await?
            .ok_or(TrackingError::NoActiveProfile)?;

        info!(secret_store = %connection.secret_store, "adding connection");
        let secret_store = connection.secret_store;
        let secret = store_secret(
            &profile.name,
            &connection.name,
            secret_store,
            connection.token,
        )
        .await?;

        let added = async {
            let txn = self.db.begin().await?;
            let model = connection::ActiveModel {
                name: Set(connection.name.clone()),
                kind: Set(connection.kind),
                host: Set(normalize_host(&connection.host)),
                identity: Set(connection.identity),
                profile_id: Set(profile.id),
                ..Default::default()
            }
            .insert(&txn)
            .await
            .map_err(|e| match e.sql_err() {
                Some(SqlErr::UniqueConstraintViolation(_)) => {
                    DbError::DuplicateName(connection.name)
                }
                _ => e.into(),
            })?;
            credential::ActiveModel {
                connection_id: Set(model.id),
                secret_store: Set(secret_store),
                credential: Set(secret.clone()),
            }
            .insert(&txn)
            .await?;
            let jira_settings = match model.kind.as_str() {
                "jira-cloud" | "jira-dc" => Some(
                    jira_settings::ActiveModel {
                        connection_id: Set(model.id),
                        ..Default::default()
                    }
                    .insert(&txn)
                    .await?,
                ),
                _ => None,
            };
            let connection = Connection::new(model, jira_settings)?;
            txn.commit().await?;
            Ok(connection)
        }
        .await;
        if added.is_err() && secret_store == SecretStore::Keyring {
            delete_from_keychain(secret).await;
        }

        added
    }

    pub async fn test_connection_data(
        &self,
        mut connection: ConnectionData,
    ) -> Result<(), NirvanaError> {
        connection.host = normalize_host(&connection.host);
        let candidate = connection::Model {
            id: 0,
            name: connection.name,
            kind: connection.kind,
            host: connection.host,
            identity: connection.identity,
            created_at: 0,
            updated_at: 0,
            profile_id: 0,
        };
        let settings = jira_settings::Model {
            connection_id: 0,
            publish: false,
            squash: false,
        };
        let candidate = Connection::new(candidate, Some(settings))?;
        let integ = integration::build_integration(&candidate, &connection.token);
        integ.test_connection().await?;
        Ok(())
    }

    pub async fn test_connection(&self) -> Result<(), NirvanaError> {
        let connection_id = self
            .config
            .core
            .active_connection
            .ok_or(TrackingError::NoActiveConnection)?;

        let connection = self.get_connection(connection_id).await?;
        let token = read_credential(connection_id, &self.db).await?;
        let integ = integration::build_integration(&connection, &token);
        integ.test_connection().await?;
        Ok(())
    }

    pub fn issue_urls<'a>(
        &self,
        connection: &Connection,
        tickets: impl IntoIterator<Item = &'a ticket::Model>,
    ) -> Result<Vec<String>, NirvanaError> {
        let integ = integration::Integration::build_for_url(connection);
        Ok(tickets
            .into_iter()
            .map(|ticket| integ.get_issue_link(&ticket.ticket_key))
            .collect())
    }
}

fn normalize_host(url: &str) -> String {
    let s = url.trim();
    let s = s.strip_prefix("https://").unwrap_or(s);
    let s = s.strip_prefix("http://").unwrap_or(s);
    let s = s.strip_suffix('/').unwrap_or(s);
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::test_support::test_api;

    #[tokio::test]
    async fn rejects_duplicate_connection_name() {
        let api = test_api().await;

        let result = api
            .add_connection(ConnectionData {
                name: "Test Jira".to_string(),
                kind: "jira-cloud".to_string(),
                host: "https://other.atlassian.net".to_string(),
                identity: "tester@example.com".to_string(),
                secret_store: SecretStore::Plaintext,
                token: "token".into(),
            })
            .await;

        assert!(matches!(
            result,
            Err(NirvanaError::Db(DbError::DuplicateName(name))) if name == "Test Jira"
        ));
        assert_eq!(api.list_connections().await.unwrap().len(), 1);
    }
}
