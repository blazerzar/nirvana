//! Connections.
//!
//! A connection belongs to a profile and points to a backend such as Jira.
//! Its token is stored in the `credentials` table, either as plaintext or as
//! the account of a keyring entry. Hosts are stored normalized, without
//! scheme or trailing slash.

use nirvana_entity::connection::Entity as ConnectionEntity;
use nirvana_entity::{connection, credential, jira_settings};
use sea_orm::{ActiveModelTrait, Set, SqlErr, TransactionTrait};
use tracing::info;

use crate::domain::{Connection, ConnectionData, Kind};
use crate::errors::{DomainError, NirvanaError};
use crate::secrets::{SecretStore, delete_from_keychain, store_secret};
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
    /// identity, a Jira DC connection has one, no profile is active, the
    /// config is unavailable, the profile already has a connection with that
    /// name, the token cannot be stored or the database insert fails.
    pub async fn add_connection(
        &self,
        connection: ConnectionData,
    ) -> Result<Connection, NirvanaError> {
        if connection.name.trim().is_empty() {
            return Err(DomainError::EmptyConnectionName.into());
        }
        match (
            connection.kind,
            connection.identity.as_deref().map(str::trim),
        ) {
            (Kind::JiraCloud, None | Some("")) => {
                return Err(DomainError::MissingIdentity(connection.kind).into());
            }
            (Kind::JiraDc, Some(_)) => {
                return Err(DomainError::UnexpectedIdentity(connection.kind).into());
            }
            (Kind::JiraCloud, Some(_)) | (Kind::JiraDc, None) => {}
        }

        let profile = self.find_active_profile().await?;

        info!(secret_store = %connection.secret_store, "adding connection");
        let secret_store = connection.secret_store;
        let secret = store_secret(
            &self.keyring,
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
                kind: Set(connection.kind.as_str().to_string()),
                host: Set(normalize_host(&connection.host)),
                identity: Set(connection.identity),
                profile_id: Set(profile.id),
                ..Default::default()
            }
            .insert(&txn)
            .await
            .map_err(|e| match e.sql_err() {
                Some(SqlErr::UniqueConstraintViolation(_)) => {
                    NirvanaError::from(DomainError::ConnectionExists(connection.name))
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
            let jira_settings = match connection.kind {
                Kind::JiraCloud | Kind::JiraDc => Some(
                    jira_settings::ActiveModel {
                        connection_id: Set(model.id),
                        ..Default::default()
                    }
                    .insert(&txn)
                    .await?,
                ),
            };
            let connection = Connection::new(model, jira_settings)?;
            txn.commit().await?;
            Ok(connection)
        }
        .await;
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
    use sea_orm::EntityTrait;
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
