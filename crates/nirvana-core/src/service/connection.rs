//! Connections.
//!
//! A connection belongs to a profile and points to a backend such as Jira.
//! Its token is stored in the `credentials` table, either as plaintext or as
//! the account of a keyring entry. Hosts are stored normalized, without
//! scheme or trailing slash.

use nirvana_entity::connection::Entity as ConnectionEntity;
use nirvana_entity::{connection, credential, jira_settings, publish_entry, slot, ticket};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, DbErr, EntityTrait, ModelTrait, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait, Set, SqlErr, TransactionTrait,
};
use tracing::info;

use crate::domain::{Connection, ConnectionData, Kind, Settings};
use crate::errors::{DomainError, NirvanaError};
use crate::integration::Integration;
use crate::secrets::{SecretStore, delete_from_keychain, read_credential, store_secret};
use crate::service::Nirvana;

impl Nirvana {
    /// Adds a connection to the active profile and stores its token in the
    /// chosen secret store.
    ///
    /// A keyring entry is removed again if the connection cannot be saved.
    ///
    /// # Errors
    ///
    /// Returns an error if the name is empty, a Jira Cloud connection has no
    /// identity, a Jira DC or Linear connection has one, no profile is
    /// active, the config is unavailable, the profile already has a connection
    /// with that name, the token cannot be stored or the database insert fails.
    pub async fn add_connection(
        &self,
        connection: ConnectionData,
    ) -> Result<Connection, NirvanaError> {
        connection.validate()?;
        let ConnectionData {
            name,
            kind,
            host,
            identity,
            secret_store,
            token,
        } = connection;
        let profile = self.find_active_profile().await?;

        info!(%secret_store, "adding connection");
        let secret = store_secret(&self.keyring, &profile.name, &name, secret_store, token).await?;

        let model = connection::ActiveModel {
            name: Set(name.clone()),
            kind: Set(kind.as_str().to_string()),
            host: Set(normalize_host(&host)),
            identity: Set(identity),
            profile_id: Set(profile.id),
            ..Default::default()
        };
        let credential = credential::ActiveModel {
            secret_store: Set(secret_store),
            credential: Set(secret.clone()),
            ..Default::default()
        };
        let added = insert_connection(&self.db, model, credential, kind)
            .await
            .map_err(|e| match e.sql_err() {
                Some(SqlErr::UniqueConstraintViolation(_)) => {
                    NirvanaError::from(DomainError::ConnectionExists(name))
                }
                _ => e.into(),
            });
        if added.is_err() && secret_store == SecretStore::Keyring {
            delete_from_keychain(&self.keyring, secret).await;
        }

        added
    }

    /// Renames the connection named `name` in the active profile to
    /// `new_name`. The keyring entry keeps its account, so the token stays
    /// readable.
    ///
    /// # Errors
    ///
    /// Returns an error if the new name is empty, no profile is active, the
    /// config is unavailable, the connection does not exist, the profile
    /// already has a connection with the new name or a database query fails.
    pub async fn rename_connection(
        &self,
        name: &str,
        new_name: String,
    ) -> Result<(), NirvanaError> {
        if new_name.trim().is_empty() {
            return Err(DomainError::EmptyConnectionName.into());
        }

        let profile = self.find_active_profile().await?;
        let Some(connection) =
            ConnectionEntity::find_by_name_in_profile((name.to_owned(), profile.id))
                .one(&self.db)
                .await?
        else {
            return Err(DomainError::ConnectionNotFound(name.to_owned()).into());
        };

        let mut connection: connection::ActiveModel = connection.into();
        connection.name = Set(new_name.clone());
        connection
            .update(&self.db)
            .await
            .map_err(|e| match e.sql_err() {
                Some(SqlErr::UniqueConstraintViolation(_)) => {
                    NirvanaError::from(DomainError::ConnectionExists(new_name))
                }
                _ => e.into(),
            })?;
        Ok(())
    }

    /// Returns the connections of the active profile, oldest first.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable,
    /// a connection has invalid settings or no credential, or the database
    /// query fails.
    pub async fn list_connections(&self) -> Result<Vec<Connection>, NirvanaError> {
        let profile = self.find_active_profile().await?;
        self.find_connections(profile.id).await
    }

    pub(super) async fn find_connections(
        &self,
        profile_id: i64,
    ) -> Result<Vec<Connection>, NirvanaError> {
        Ok(ConnectionEntity::load()
            .with(credential::Entity)
            .with(jira_settings::Entity)
            .filter(connection::COLUMN.profile_id.eq(profile_id))
            .order_by_asc(connection::COLUMN.id)
            .all(&self.db)
            .await?
            .into_iter()
            .map(Connection::new)
            .collect::<Result<_, _>>()?)
    }

    /// Deletes the connection named `name` from the active profile with its
    /// credential, settings and publish entries, and removes its keyring
    /// entry.
    ///
    /// With `delete_tickets` its tickets are deleted with their slots.
    /// Otherwise its tickets are kept without a connection. A keyring entry
    /// that cannot be removed is only logged.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable,
    /// the connection does not exist or a database query fails.
    pub async fn delete_connection(
        &self,
        name: &str,
        delete_tickets: bool,
    ) -> Result<(), NirvanaError> {
        let profile = self.find_active_profile().await?;
        let txn = self.db.begin().await?;
        let Some((connection, credential)) =
            ConnectionEntity::find_by_name_in_profile((name.to_owned(), profile.id))
                .find_also_related(credential::Entity)
                .one(&txn)
                .await?
        else {
            return Err(DomainError::ConnectionNotFound(name.to_owned()).into());
        };

        publish_entry::Entity::delete_many()
            .filter(publish_entry::COLUMN.connection_id.eq(connection.id))
            .exec(&txn)
            .await?;
        if delete_tickets {
            let tickets = ticket::Entity::find()
                .select_only()
                .column(ticket::Column::Id)
                .filter(ticket::COLUMN.connection_id.eq(connection.id))
                .into_query();
            slot::Entity::delete_many()
                .filter(slot::COLUMN.ticket_id.in_subquery(tickets))
                .exec(&txn)
                .await?;
            ticket::Entity::delete_many()
                .filter(ticket::COLUMN.connection_id.eq(connection.id))
                .exec(&txn)
                .await?;
        } else {
            ticket::Entity::update_many()
                .col_expr(ticket::Column::ConnectionId, Expr::value(None::<i64>))
                .filter(ticket::COLUMN.connection_id.eq(connection.id))
                .exec(&txn)
                .await?;
        }
        connection.delete(&txn).await?;
        txn.commit().await?;

        if let Some(credential) = credential
            && credential.secret_store == SecretStore::Keyring
        {
            delete_from_keychain(&self.keyring, credential.credential).await;
        }
        Ok(())
    }

    /// Tests the connection named `name` in the active profile by reading its
    /// token and signing in to its backend.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable,
    /// the connection does not exist, a Jira Cloud connection has no
    /// identity, the token cannot be read, the backend cannot be reached or
    /// rejects the token, or a database query fails.
    pub async fn test_connection(&self, name: &str) -> Result<(), NirvanaError> {
        let profile = self.find_active_profile().await?;
        let connection = self
            .find_connections(profile.id)
            .await?
            .into_iter()
            .find(|connection| connection.model.name == name)
            .ok_or_else(|| DomainError::ConnectionNotFound(name.to_owned()))?;

        info!(kind = %connection.kind, "testing connection");
        let token = read_credential(&self.keyring, connection.model.id, &self.db).await?;
        Integration::new(&connection, token)?
            .test_connection()
            .await?;
        Ok(())
    }
}

/// Inserts the connection with its credential and the default settings of
/// its kind in one transaction.
async fn insert_connection(
    db: &DatabaseConnection,
    model: connection::ActiveModel,
    mut credential: credential::ActiveModel,
    kind: Kind,
) -> Result<Connection, DbErr> {
    let txn = db.begin().await?;
    let model = model.insert(&txn).await?;
    credential.connection_id = Set(model.id);
    let credential = credential.insert(&txn).await?;
    let settings = match kind {
        Kind::JiraCloud | Kind::JiraDc => Settings::Jira(
            jira_settings::ActiveModel {
                connection_id: Set(model.id),
                ..Default::default()
            }
            .insert(&txn)
            .await?
            .into(),
        ),
        Kind::Linear => Settings::Linear,
    };
    txn.commit().await?;
    Ok(Connection {
        model,
        kind,
        settings,
        secret_store: credential.secret_store,
    })
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
    use secrecy::ExposeSecret;

    use super::*;
    use crate::secrets::read_credential;
    use crate::service::test_support::test_service;

    fn keyring_data(name: &str) -> ConnectionData {
        ConnectionData {
            name: name.to_string(),
            kind: Kind::JiraCloud,
            host: "https://other.atlassian.net/".to_string(),
            identity: Some("tester@example.com".to_string()),
            secret_store: SecretStore::Keyring,
            token: "keyring-token".into(),
        }
    }

    fn keyring_passwords(service: &Nirvana) -> Vec<String> {
        let store = (service.keyring)().unwrap();
        let spec = [("service", "nirvana")].into();
        store
            .search(&spec)
            .unwrap()
            .into_iter()
            .filter_map(|entry| match entry.get_password() {
                Ok(password) => Some(password),
                Err(keyring_core::Error::NoEntry) => None,
                Err(e) => panic!("keyring read failed: {e}"),
            })
            .collect()
    }

    #[tokio::test]
    async fn add_connection_adds_to_active_profile() {
        let mut service = test_service().await;
        let work = service.create_profile("work".to_string()).await.unwrap();
        service.switch_profile("work").await.unwrap();

        let added = service
            .add_connection(keyring_data("Work Jira"))
            .await
            .unwrap();

        assert_eq!(added.model.profile_id, work.id);
        assert_eq!(added.model.host, "other.atlassian.net");
        assert_eq!(added.kind, Kind::JiraCloud);
        assert_eq!(service.active_profile().await.unwrap().connections.len(), 1);
    }

    #[tokio::test]
    async fn add_connection_allows_same_name_in_another_profile() {
        let mut service = test_service().await;
        service.create_profile("work".to_string()).await.unwrap();
        service.switch_profile("work").await.unwrap();

        let added = service
            .add_connection(ConnectionData {
                secret_store: SecretStore::Plaintext,
                ..keyring_data("Test Jira")
            })
            .await;

        assert!(added.is_ok());
    }

    #[tokio::test]
    async fn keyring_connection_stores_account_and_reads_token() {
        let service = test_service().await;
        let connection = service
            .add_connection(keyring_data("Keyring Jira"))
            .await
            .unwrap();
        let credential = credential::Entity::find_by_id(connection.model.id)
            .one(&service.db)
            .await
            .unwrap()
            .unwrap()
            .credential;

        let key = credential
            .strip_prefix("nirvana_test_Keyring Jira_")
            .unwrap();
        assert_eq!(key.len(), 8);
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));

        let token = read_credential(&service.keyring, connection.model.id, &service.db)
            .await
            .unwrap();
        assert_eq!(token.expose_secret(), "keyring-token");
    }

    #[tokio::test]
    async fn rejected_keyring_connection_leaves_no_entry() {
        let service = test_service().await;
        service
            .add_connection(keyring_data("Keyring Jira"))
            .await
            .unwrap();

        let result = service
            .add_connection(ConnectionData {
                token: "duplicate-token".into(),
                ..keyring_data("Keyring Jira")
            })
            .await;

        assert!(matches!(
            result,
            Err(NirvanaError::Domain(DomainError::ConnectionExists(_)))
        ));
        assert_eq!(keyring_passwords(&service), ["keyring-token"]);
    }

    #[tokio::test]
    async fn add_connection_rejects_duplicate_name() {
        let service = test_service().await;

        let result = service
            .add_connection(ConnectionData {
                secret_store: SecretStore::Plaintext,
                ..keyring_data("Test Jira")
            })
            .await;

        assert!(matches!(
            result,
            Err(NirvanaError::Domain(DomainError::ConnectionExists(name))) if name == "Test Jira"
        ));
        assert_eq!(service.active_profile().await.unwrap().connections.len(), 1);
    }

    #[tokio::test]
    async fn add_connection_rejects_empty_name() {
        let service = test_service().await;

        let result = service.add_connection(keyring_data(" ")).await;

        assert!(matches!(
            result,
            Err(NirvanaError::Domain(DomainError::EmptyConnectionName))
        ));
        assert!(keyring_passwords(&service).is_empty());
        assert_eq!(service.active_profile().await.unwrap().connections.len(), 1);
    }

    #[tokio::test]
    async fn add_connection_rejects_jira_cloud_without_identity() {
        let service = test_service().await;

        for identity in [None, Some(" ".to_string())] {
            let result = service
                .add_connection(ConnectionData {
                    identity,
                    ..keyring_data("Cloud Jira")
                })
                .await;

            assert!(matches!(
                result,
                Err(NirvanaError::Domain(DomainError::MissingIdentity(
                    Kind::JiraCloud
                )))
            ));
        }
        assert!(keyring_passwords(&service).is_empty());
        assert_eq!(service.active_profile().await.unwrap().connections.len(), 1);
    }

    #[tokio::test]
    async fn add_connection_rejects_jira_dc_with_identity() {
        let service = test_service().await;

        let result = service
            .add_connection(ConnectionData {
                kind: Kind::JiraDc,
                ..keyring_data("DC Jira")
            })
            .await;

        assert!(matches!(
            result,
            Err(NirvanaError::Domain(DomainError::UnexpectedIdentity(
                Kind::JiraDc
            )))
        ));
        assert!(keyring_passwords(&service).is_empty());
        assert_eq!(service.active_profile().await.unwrap().connections.len(), 1);
    }

    #[tokio::test]
    async fn add_connection_adds_jira_dc_without_identity() {
        let service = test_service().await;

        let added = service
            .add_connection(ConnectionData {
                kind: Kind::JiraDc,
                identity: None,
                ..keyring_data("DC Jira")
            })
            .await
            .unwrap();

        assert_eq!(added.kind, Kind::JiraDc);
        assert_eq!(added.model.identity, None);
    }

    #[tokio::test]
    async fn add_connection_rejects_linear_with_identity() {
        let service = test_service().await;

        let result = service
            .add_connection(ConnectionData {
                kind: Kind::Linear,
                ..keyring_data("Linear")
            })
            .await;

        assert!(matches!(
            result,
            Err(NirvanaError::Domain(DomainError::UnexpectedIdentity(
                Kind::Linear
            )))
        ));
        assert!(keyring_passwords(&service).is_empty());
        assert_eq!(service.active_profile().await.unwrap().connections.len(), 1);
    }

    #[tokio::test]
    async fn add_connection_adds_linear_without_settings() {
        let service = test_service().await;

        let added = service
            .add_connection(ConnectionData {
                kind: Kind::Linear,
                identity: None,
                ..keyring_data("Linear")
            })
            .await
            .unwrap();

        assert_eq!(added.kind, Kind::Linear);
        assert_eq!(added.model.identity, None);
        assert!(matches!(added.settings, Settings::Linear));
        assert!(
            jira_settings::Entity::find_by_id(added.model.id)
                .one(&service.db)
                .await
                .unwrap()
                .is_none()
        );
        let loaded = service.active_profile().await.unwrap().connections;
        assert!(matches!(loaded[1].settings, Settings::Linear));
    }

    async fn connection_names(service: &Nirvana) -> Vec<String> {
        service
            .active_profile()
            .await
            .unwrap()
            .connections
            .into_iter()
            .map(|connection| connection.model.name)
            .collect()
    }

    #[tokio::test]
    async fn rename_connection_keeps_keyring_token() {
        let service = test_service().await;
        let added = service
            .add_connection(keyring_data("Keyring Jira"))
            .await
            .unwrap();

        service
            .rename_connection("Keyring Jira", "Work Jira".to_string())
            .await
            .unwrap();

        assert_eq!(connection_names(&service).await, ["Test Jira", "Work Jira"]);
        let token = read_credential(&service.keyring, added.model.id, &service.db)
            .await
            .unwrap();
        assert_eq!(token.expose_secret(), "keyring-token");
    }

    #[tokio::test]
    async fn rename_connection_only_renames_in_active_profile() {
        let mut service = test_service().await;
        service.create_profile("work".to_string()).await.unwrap();
        service.switch_profile("work").await.unwrap();

        let err = service
            .rename_connection("Test Jira", "Work Jira".to_string())
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::ConnectionNotFound(name)) if name == "Test Jira"
        ));
        service.switch_profile("test").await.unwrap();
        assert_eq!(connection_names(&service).await, ["Test Jira"]);
    }

    #[tokio::test]
    async fn rename_connection_rejects_duplicate_name() {
        let service = test_service().await;
        service
            .add_connection(keyring_data("Keyring Jira"))
            .await
            .unwrap();

        let err = service
            .rename_connection("Keyring Jira", "Test Jira".to_string())
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::ConnectionExists(name)) if name == "Test Jira"
        ));
        assert_eq!(
            connection_names(&service).await,
            ["Test Jira", "Keyring Jira"]
        );
    }

    #[tokio::test]
    async fn rename_connection_rejects_missing_connection() {
        let service = test_service().await;

        let err = service
            .rename_connection("missing", "Work Jira".to_string())
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::ConnectionNotFound(name)) if name == "missing"
        ));
    }

    #[tokio::test]
    async fn rename_connection_rejects_empty_name() {
        let service = test_service().await;

        let err = service
            .rename_connection("Test Jira", " ".to_string())
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::EmptyConnectionName)
        ));
        assert_eq!(connection_names(&service).await, ["Test Jira"]);
    }

    #[tokio::test]
    async fn list_connections_returns_active_profile_connections() {
        let mut service = test_service().await;
        service
            .add_connection(keyring_data("Keyring Jira"))
            .await
            .unwrap();

        let connections = service.list_connections().await.unwrap();

        assert_eq!(
            connections
                .iter()
                .map(|c| (c.model.name.as_str(), c.secret_store))
                .collect::<Vec<_>>(),
            [
                ("Test Jira", SecretStore::Plaintext),
                ("Keyring Jira", SecretStore::Keyring)
            ]
        );
        service.create_profile("work".to_string()).await.unwrap();
        service.switch_profile("work").await.unwrap();
        assert!(service.list_connections().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_connections_rejects_missing_credential() {
        let service = test_service().await;
        let id = service.list_connections().await.unwrap()[0].model.id;
        credential::Entity::delete_by_id(id)
            .exec(&service.db)
            .await
            .unwrap();

        let err = service.list_connections().await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::MissingCredential(missing)) if missing == id
        ));
    }

    #[tokio::test]
    async fn list_connections_rejects_missing_settings() {
        let service = test_service().await;
        let id = service.list_connections().await.unwrap()[0].model.id;
        jira_settings::Entity::delete_by_id(id)
            .exec(&service.db)
            .await
            .unwrap();

        let err = service.list_connections().await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::MissingConnectionSettings(missing)) if missing == id
        ));
    }

    struct History {
        deleted_connection: i64,
        deleted_ticket: i64,
        kept_ticket: i64,
        kept_slot: i64,
        kept_connection: i64,
    }

    /// Adds a ticket of `connection_id` with one slot and returns their ids.
    async fn add_slot(service: &Nirvana, connection_id: i64, key: &str) -> (i64, i64) {
        let ticket = ticket::ActiveModel {
            profile_id: Set(service.find_active_profile().await.unwrap().id),
            connection_id: Set(Some(connection_id)),
            ticket_key: Set(key.to_string()),
            synced_at: Set(0),
            ..Default::default()
        }
        .insert(&service.db)
        .await
        .unwrap();
        let slot = slot::ActiveModel {
            ticket_id: Set(ticket.id),
            profile_id: Set(ticket.profile_id),
            started_at: Set(0),
            stopped_at: Set(Some(60)),
            ..Default::default()
        }
        .insert(&service.db)
        .await
        .unwrap();
        (ticket.id, slot.id)
    }

    /// Adds a published slot to "Test Jira" and to a new keyring connection
    /// "Keyring Jira". The "Test Jira" slot is published to both.
    async fn add_history(service: &Nirvana) -> History {
        let kept_connection = service.list_connections().await.unwrap()[0].model.id;
        let deleted_connection = service
            .add_connection(keyring_data("Keyring Jira"))
            .await
            .unwrap()
            .model
            .id;
        let (deleted_ticket, deleted_slot) = add_slot(service, deleted_connection, "DEL-1").await;
        let (kept_ticket, kept_slot) = add_slot(service, kept_connection, "KEEP-1").await;

        for (slot_id, connection_id) in [
            (deleted_slot, deleted_connection),
            (kept_slot, kept_connection),
            (kept_slot, deleted_connection),
        ] {
            publish_entry::ActiveModel {
                slot_id: Set(slot_id),
                connection_id: Set(connection_id),
                published_at: Set(Some(60)),
                error: Set(None),
            }
            .insert(&service.db)
            .await
            .unwrap();
        }

        History {
            deleted_connection,
            deleted_ticket,
            kept_ticket,
            kept_slot,
            kept_connection,
        }
    }

    async fn assert_cascaded(service: &Nirvana, connection_id: i64) {
        let credential = credential::Entity::find_by_id(connection_id)
            .one(&service.db)
            .await
            .unwrap();
        let settings = jira_settings::Entity::find_by_id(connection_id)
            .one(&service.db)
            .await
            .unwrap();
        assert_eq!((credential, settings), (None, None));
    }

    async fn publish_entries(service: &Nirvana) -> Vec<(i64, i64)> {
        publish_entry::Entity::find()
            .all(&service.db)
            .await
            .unwrap()
            .into_iter()
            .map(|entry| (entry.slot_id, entry.connection_id))
            .collect()
    }

    #[tokio::test]
    async fn delete_connection_keeps_tickets_without_connection() {
        let service = test_service().await;
        let history = add_history(&service).await;

        service
            .delete_connection("Keyring Jira", false)
            .await
            .unwrap();

        assert_eq!(connection_names(&service).await, ["Test Jira"]);
        assert!(keyring_passwords(&service).is_empty());
        assert_cascaded(&service, history.deleted_connection).await;
        let ticket = ticket::Entity::find_by_id(history.deleted_ticket)
            .one(&service.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ticket.connection_id, None);
        assert_eq!(
            slot::Entity::find().all(&service.db).await.unwrap().len(),
            2
        );
        assert_eq!(
            publish_entries(&service).await,
            [(history.kept_slot, history.kept_connection)]
        );
    }

    #[tokio::test]
    async fn delete_connection_deletes_tickets_and_slots() {
        let service = test_service().await;
        let history = add_history(&service).await;

        service
            .delete_connection("Keyring Jira", true)
            .await
            .unwrap();

        assert_eq!(connection_names(&service).await, ["Test Jira"]);
        assert!(keyring_passwords(&service).is_empty());
        assert_cascaded(&service, history.deleted_connection).await;
        let tickets = ticket::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(
            tickets.iter().map(|t| t.id).collect::<Vec<_>>(),
            [history.kept_ticket]
        );
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(
            slots.iter().map(|s| s.id).collect::<Vec<_>>(),
            [history.kept_slot]
        );
        assert_eq!(
            publish_entries(&service).await,
            [(history.kept_slot, history.kept_connection)]
        );
    }

    #[tokio::test]
    async fn delete_connection_rejects_missing_connection() {
        let service = test_service().await;

        let err = service
            .delete_connection("missing", true)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::ConnectionNotFound(name)) if name == "missing"
        ));
        assert_eq!(connection_names(&service).await, ["Test Jira"]);
    }

    #[tokio::test]
    async fn test_connection_rejects_missing_connection() {
        let service = test_service().await;

        let err = service.test_connection("missing").await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::ConnectionNotFound(name)) if name == "missing"
        ));
    }

    #[test]
    fn normalize_host_strips_scheme_whitespace_and_trailing_slash() {
        for host in [
            "example.com",
            "https://example.com",
            "http://example.com/",
            " https://example.com/ ",
        ] {
            assert_eq!(normalize_host(host), "example.com");
        }
    }
}
