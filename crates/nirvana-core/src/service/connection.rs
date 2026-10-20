use nirvana_entity::connection::Entity as Connection;
use nirvana_entity::profile::Entity as Profile;
use nirvana_entity::{connection, credential, profile, slot, ticket};
use sea_orm::{
    ActiveModelTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};

use crate::domain::ConnectionData;
use crate::errors::{DbError, NirvanaError, TrackingError};
use crate::integration;
use crate::service::Nirvana;

impl Nirvana {
    pub async fn list_connections(&self) -> Result<Vec<connection::Model>, NirvanaError> {
        Ok(Connection::find()
            .order_by_asc(connection::COLUMN.id)
            .all(&self.db)
            .await?)
    }

    pub async fn get_active_connection(&self) -> Result<Option<connection::Model>, NirvanaError> {
        let Some(connection_id) = self.config.core.active_connection else {
            return Ok(None);
        };

        Ok(Some(self.get_connection(connection_id).await?))
    }

    pub fn active_connection(&self) -> Option<i64> {
        self.config.core.active_connection
    }

    pub async fn set_active_connection(&mut self, id: i64) -> Result<(), NirvanaError> {
        if (Connection::find_by_id(id).one(&self.db).await?).is_none() {
            return Err(TrackingError::ConnectionNotFound(id).into());
        }

        self.config.core.active_connection = Some(id);
        self.config.save(&self.paths)?;
        Ok(())
    }

    pub async fn delete_connection(&mut self, id: i64) -> Result<(), NirvanaError> {
        if (Connection::find_by_id(id).one(&self.db).await?).is_none() {
            return Err(TrackingError::ConnectionNotFound(id).into());
        }

        let tickets = ticket::Entity::find()
            .filter(ticket::COLUMN.connection_id.eq(id))
            .count(&self.db)
            .await?;
        let slots = slot::Entity::find()
            .filter(slot::COLUMN.connection_id.eq(id))
            .count(&self.db)
            .await?;
        if tickets > 0 || slots > 0 {
            return Err(TrackingError::ConnectionHasHistory.into());
        }

        Connection::delete_by_id(id).exec(&self.db).await?;

        if self.config.core.active_connection == Some(id) {
            self.config.core.active_connection = (Connection::find()
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
    ) -> Result<connection::Model, NirvanaError> {
        if connection.secret_store != "plaintext" {
            unimplemented!("only plaintext secret store is currently supported");
        }

        let profile = Profile::find()
            .order_by_asc(profile::COLUMN.id)
            .one(&self.db)
            .await?
            .ok_or(TrackingError::NoActiveProfile)?;

        let duplicates = Connection::find()
            .filter(connection::COLUMN.profile_id.eq(profile.id))
            .filter(connection::COLUMN.name.eq(&connection.name))
            .count(&self.db)
            .await?;
        if duplicates > 0 {
            return Err(DbError::DuplicateName(connection.name).into());
        }

        let txn = self.db.begin().await?;
        let model = connection::ActiveModel {
            name: Set(connection.name),
            kind: Set(connection.kind),
            host: Set(normalize_host(&connection.host)),
            identity: Set(connection.identity),
            secret_store: Set(connection.secret_store),
            profile_id: Set(profile.id),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
        credential::ActiveModel {
            connection_id: Set(model.id),
            credential: Set(connection.token),
        }
        .insert(&txn)
        .await?;
        txn.commit().await?;

        Ok(model)
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
            secret_store: connection.secret_store,
            created_at: 0,
            updated_at: 0,
            profile_id: 0,
        };
        let integ = integration::build_integration(&candidate, &connection.token)?;
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
        let token = self.get_token(connection_id).await?;
        let integ = integration::build_integration(&connection, &token)?;
        integ.test_connection().await?;
        Ok(())
    }

    pub(crate) async fn get_token(&self, connection_id: i64) -> Result<String, NirvanaError> {
        let credential = credential::Entity::find_by_id(connection_id)
            .one(&self.db)
            .await?
            .ok_or(DbErr::RecordNotFound(format!(
                "credential for connection {connection_id}"
            )))?;
        Ok(credential.credential)
    }

    pub fn issue_urls<'a>(
        &self,
        connection: &connection::Model,
        tickets: impl IntoIterator<Item = &'a ticket::Model>,
    ) -> Result<Vec<String>, NirvanaError> {
        let integ = integration::Integration::build_for_url(connection)?;
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
