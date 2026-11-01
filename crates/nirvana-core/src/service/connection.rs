//! Connections.
//!
//! A connection belongs to a profile and points to a backend such as Jira.
//! Its token is stored in the `credentials` table, either as plaintext or as
//! the account of a keyring entry. Hosts are stored normalized, without
//! scheme or trailing slash.

use nirvana_entity::{connection, credential, jira_settings};
use sea_orm::{ActiveModelTrait, Set, SqlErr, TransactionTrait};
use tracing::info;

use crate::domain::{Connection, ConnectionData};
use crate::errors::{DbError, NirvanaError};
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
    /// Returns an error if no profile is active, the config is unavailable,
    /// the profile already has a connection with that name, the token cannot
    /// be stored, the kind is unsupported or the database insert fails.
    pub async fn add_connection(
        &self,
        connection: ConnectionData,
    ) -> Result<Connection, NirvanaError> {
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
            delete_from_keychain(&self.keyring, secret).await;
        }

        added
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
            kind: "jira-cloud".to_string(),
            host: "https://other.atlassian.net/".to_string(),
            identity: "tester@example.com".to_string(),
            secret_store: SecretStore::Keyring,
            token: "keyring-token".into(),
        }
    }

    fn keyring_passwords(api: &Nirvana) -> Vec<String> {
        let store = (api.keyring)().unwrap();
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
        let mut api = test_service().await;
        let work = api.create_profile("work".to_string()).await.unwrap();
        api.switch_profile("work".to_string()).await.unwrap();

        let added = api.add_connection(keyring_data("Work Jira")).await.unwrap();

        assert_eq!(added.model.profile_id, work.id);
        assert_eq!(added.model.host, "other.atlassian.net");
        assert_eq!(added.kind.as_str(), "jira-cloud");
        assert_eq!(api.active_profile().await.unwrap().connections.len(), 1);
    }

    #[tokio::test]
    async fn add_connection_allows_same_name_in_another_profile() {
        let mut api = test_service().await;
        api.create_profile("work".to_string()).await.unwrap();
        api.switch_profile("work".to_string()).await.unwrap();

        let added = api
            .add_connection(ConnectionData {
                secret_store: SecretStore::Plaintext,
                ..keyring_data("Test Jira")
            })
            .await;

        assert!(added.is_ok());
    }

    #[tokio::test]
    async fn keyring_connection_stores_account_and_reads_token() {
        let api = test_service().await;
        let connection = api
            .add_connection(keyring_data("Keyring Jira"))
            .await
            .unwrap();
        let credential = credential::Entity::find_by_id(connection.model.id)
            .one(&api.db)
            .await
            .unwrap()
            .unwrap()
            .credential;

        let key = credential
            .strip_prefix("nirvana_test_Keyring Jira_")
            .unwrap();
        assert_eq!(key.len(), 8);
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));

        let token = read_credential(&api.keyring, connection.model.id, &api.db)
            .await
            .unwrap();
        assert_eq!(token.expose_secret(), "keyring-token");
    }

    #[tokio::test]
    async fn rejected_keyring_connection_leaves_no_entry() {
        let api = test_service().await;
        api.add_connection(keyring_data("Keyring Jira"))
            .await
            .unwrap();

        let result = api
            .add_connection(ConnectionData {
                token: "duplicate-token".into(),
                ..keyring_data("Keyring Jira")
            })
            .await;

        assert!(matches!(
            result,
            Err(NirvanaError::Db(DbError::DuplicateName(_)))
        ));
        assert_eq!(keyring_passwords(&api), ["keyring-token"]);
    }

    #[tokio::test]
    async fn rejects_duplicate_connection_name() {
        let api = test_service().await;

        let result = api
            .add_connection(ConnectionData {
                secret_store: SecretStore::Plaintext,
                ..keyring_data("Test Jira")
            })
            .await;

        assert!(matches!(
            result,
            Err(NirvanaError::Db(DbError::DuplicateName(name))) if name == "Test Jira"
        ));
        assert_eq!(api.active_profile().await.unwrap().connections.len(), 1);
    }
}
