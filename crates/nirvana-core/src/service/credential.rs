//! Credentials of connections.
//!
//! A credential can be listed, updated or moved between secret stores. Its
//! token is never returned.

use nirvana_entity::connection::Entity as ConnectionEntity;
use nirvana_entity::{connection, credential};
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder, Set,
};
use secrecy::{ExposeSecret, SecretString};
use tracing::debug;

use crate::domain::{Credential, SecretLocation, SecretStore};
use crate::errors::{DomainError, NirvanaError};
use crate::secrets::{
    SecretError, delete_from_keychain, keyring_account, read_from_keychain, set_in_keychain,
};
use crate::service::Nirvana;

impl Nirvana {
    /// Returns where the token of every connection in the active profile is
    /// stored, oldest connection first.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable, a
    /// connection has no credential or uses an unsupported secret store, or
    /// the database query fails.
    pub async fn list_credentials(&self) -> Result<Vec<Credential>, NirvanaError> {
        let profile = self.find_active_profile().await?;
        ConnectionEntity::find()
            .find_also_related(credential::Entity)
            .filter(connection::COLUMN.profile_id.eq(profile.id))
            .order_by_asc(connection::COLUMN.id)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|(connection, credential)| {
                let credential = credential.ok_or(DomainError::MissingCredential(connection.id))?;
                to_credential(connection, credential)
            })
            .collect()
    }

    /// Returns where the token of the connection named `name` in the active
    /// profile is stored.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable,
    /// the connection does not exist or has no credential, its secret store is
    /// unsupported, or the database query fails.
    pub async fn get_credential(&self, name: &str) -> Result<Credential, NirvanaError> {
        let profile = self.find_active_profile().await?;
        let (connection, credential) = self.find_credential(profile.id, name).await?;
        to_credential(connection, credential)
    }

    /// Replaces the token of the connection named `name` in the active
    /// profile. A keyring token keeps its account.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable,
    /// the connection does not exist or has no credential, its secret store is
    /// unsupported, the keyring is unavailable or a database query fails.
    pub async fn update_credential(
        &self,
        name: &str,
        token: SecretString,
    ) -> Result<(), NirvanaError> {
        let profile = self.find_active_profile().await?;
        let (_, credential) = self.find_credential(profile.id, name).await?;

        debug!(%name, secret_store = %credential.secret_store, "updating credential");
        match credential.secret_store {
            SecretStore::Plaintext => {
                let mut credential: credential::ActiveModel = credential.into();
                credential.credential = Set(token.expose_secret().to_string());
                credential.update(&self.db).await?;
            }
            SecretStore::Keyring => {
                set_in_keychain(&self.keyring, credential.credential, token).await?;
            }
            SecretStore::Env => {
                return Err(SecretError::UnsupportedStore(credential.secret_store).into());
            }
        }
        Ok(())
    }

    /// Moves the token of the connection named `name` in the active profile
    /// to the secret store `to`.
    ///
    /// The keyring is written or read before the row is updated, so the row
    /// is left untouched when the keyring is unavailable. A new keyring entry
    /// is removed again if the row cannot be updated. An old keyring entry
    /// that cannot be removed is only logged.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable,
    /// the connection does not exist or has no credential, the token is
    /// already in `to`, either store is `env`, the keyring is unavailable or
    /// a database query fails.
    pub async fn move_credential(&self, name: &str, to: SecretStore) -> Result<(), NirvanaError> {
        let profile = self.find_active_profile().await?;
        let (connection, credential) = self.find_credential(profile.id, name).await?;
        let from = credential.secret_store;

        match (from, to) {
            (SecretStore::Env, _) | (_, SecretStore::Env) => {
                Err(SecretError::UnsupportedStore(SecretStore::Env).into())
            }
            (SecretStore::Plaintext, SecretStore::Plaintext)
            | (SecretStore::Keyring, SecretStore::Keyring) => {
                Err(DomainError::CredentialAlreadyIn(name.to_owned(), to).into())
            }
            (SecretStore::Plaintext, SecretStore::Keyring) => {
                debug!(%name, %from, %to, "moving credential");
                let token = SecretString::from(credential.credential.clone());
                let account = keyring_account(&profile.name, &connection.name);
                let account = set_in_keychain(&self.keyring, account, token).await?;
                let moved = set_credential(&self.db, credential, to, account.clone()).await;
                if moved.is_err() {
                    delete_from_keychain(&self.keyring, account).await;
                }
                Ok(moved?)
            }
            (SecretStore::Keyring, SecretStore::Plaintext) => {
                debug!(%name, %from, %to, "moving credential");
                let account = credential.credential.clone();
                let token =
                    read_from_keychain(&self.keyring, connection.id, account.clone()).await?;
                set_credential(&self.db, credential, to, token).await?;
                delete_from_keychain(&self.keyring, account).await;
                Ok(())
            }
        }
    }

    async fn find_credential(
        &self,
        profile_id: i64,
        name: &str,
    ) -> Result<(connection::Model, credential::Model), NirvanaError> {
        let Some((connection, credential)) =
            ConnectionEntity::find_by_name_in_profile((name.to_owned(), profile_id))
                .find_also_related(credential::Entity)
                .one(&self.db)
                .await?
        else {
            return Err(DomainError::ConnectionNotFound(name.to_owned()).into());
        };
        let credential = credential.ok_or(DomainError::MissingCredential(connection.id))?;
        Ok((connection, credential))
    }
}

fn to_credential(
    connection: connection::Model,
    credential: credential::Model,
) -> Result<Credential, NirvanaError> {
    let location = match credential.secret_store {
        SecretStore::Plaintext => SecretLocation::Plaintext,
        SecretStore::Keyring => SecretLocation::Keyring {
            account: credential.credential,
        },
        SecretStore::Env => {
            return Err(SecretError::UnsupportedStore(credential.secret_store).into());
        }
    };
    Ok(Credential {
        connection: connection.name,
        location,
    })
}

async fn set_credential(
    db: &DatabaseConnection,
    credential: credential::Model,
    store: SecretStore,
    value: String,
) -> Result<(), DbErr> {
    let mut credential: credential::ActiveModel = credential.into();
    credential.secret_store = Set(store);
    credential.credential = Set(value);
    credential.update(db).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sea_orm::ConnectionTrait;

    use super::*;
    use crate::domain::{ConnectionData, Kind};
    use crate::secrets::read_credential;
    use crate::service::test_support::{keyring_passwords, test_service};

    async fn add_keyring_connection(service: &Nirvana) -> i64 {
        service
            .add_connection(ConnectionData {
                name: "Keyring Jira".to_string(),
                kind: Kind::JiraDc,
                host: "jira.example.com".to_string(),
                identity: None,
                secret_store: SecretStore::Keyring,
                token: "keyring-token".into(),
            })
            .await
            .unwrap()
            .model
            .id
    }

    async fn credential_row(service: &Nirvana, name: &str) -> credential::Model {
        let profile = service.find_active_profile().await.unwrap();
        service.find_credential(profile.id, name).await.unwrap().1
    }

    async fn token(service: &Nirvana, connection_id: i64) -> String {
        read_credential(&service.keyring, connection_id, &service.db)
            .await
            .unwrap()
            .expose_secret()
            .to_string()
    }

    fn assert_key(account: &str, prefix: &str) {
        let key = account.strip_prefix(prefix).unwrap();
        assert_eq!(key.len(), 8);
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
    }

    fn unavailable_keyring(service: &mut Nirvana) {
        service.keyring = Arc::new(|| Err(keyring_core::Error::NoDefaultStore));
    }

    #[tokio::test]
    async fn list_credentials_returns_locations_without_tokens() {
        let service = test_service().await;
        add_keyring_connection(&service).await;
        let account = credential_row(&service, "Keyring Jira").await.credential;

        let credentials = service.list_credentials().await.unwrap();

        assert_eq!(
            credentials,
            [
                Credential {
                    connection: "Test Jira".to_string(),
                    location: SecretLocation::Plaintext,
                },
                Credential {
                    connection: "Keyring Jira".to_string(),
                    location: SecretLocation::Keyring { account },
                },
            ]
        );
    }

    #[tokio::test]
    async fn get_credential_returns_location_without_token() {
        let service = test_service().await;
        add_keyring_connection(&service).await;
        let account = credential_row(&service, "Keyring Jira").await.credential;

        let plaintext = service.get_credential("Test Jira").await.unwrap();
        let keyring = service.get_credential("Keyring Jira").await.unwrap();

        assert_eq!(
            [plaintext, keyring],
            [
                Credential {
                    connection: "Test Jira".to_string(),
                    location: SecretLocation::Plaintext,
                },
                Credential {
                    connection: "Keyring Jira".to_string(),
                    location: SecretLocation::Keyring { account },
                },
            ]
        );
    }

    #[tokio::test]
    async fn update_credential_replaces_plaintext_token() {
        let service = test_service().await;
        let row = credential_row(&service, "Test Jira").await;

        service
            .update_credential("Test Jira", "new-token".into())
            .await
            .unwrap();

        assert_eq!(
            credential_row(&service, "Test Jira").await,
            credential::Model {
                credential: "new-token".to_string(),
                ..row
            }
        );
    }

    #[tokio::test]
    async fn update_credential_replaces_keyring_token() {
        let service = test_service().await;
        let id = add_keyring_connection(&service).await;
        let row = credential_row(&service, "Keyring Jira").await;

        service
            .update_credential("Keyring Jira", "new-token".into())
            .await
            .unwrap();

        assert_eq!(credential_row(&service, "Keyring Jira").await, row);
        assert_eq!(token(&service, id).await, "new-token");
        assert_eq!(keyring_passwords(&service), ["new-token"]);
    }

    #[tokio::test]
    async fn update_credential_rejects_env() {
        let service = test_service().await;
        let row = credential_row(&service, "Test Jira").await;
        set_credential(&service.db, row, SecretStore::Env, "NIRVANA_TOKEN".into())
            .await
            .unwrap();

        let err = service
            .update_credential("Test Jira", "new-token".into())
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Secret(SecretError::UnsupportedStore(SecretStore::Env))
        ));
    }

    #[tokio::test]
    async fn move_credential_moves_plaintext_to_keyring() {
        let service = test_service().await;
        let id = credential_row(&service, "Test Jira").await.connection_id;

        service
            .move_credential("Test Jira", SecretStore::Keyring)
            .await
            .unwrap();

        let row = credential_row(&service, "Test Jira").await;
        assert_eq!(row.secret_store, SecretStore::Keyring);
        assert_key(&row.credential, "nirvana_test_Test Jira_");
        assert_eq!(token(&service, id).await, "token");
        assert_eq!(keyring_passwords(&service), ["token"]);
    }

    #[tokio::test]
    async fn move_credential_moves_keyring_to_plaintext() {
        let service = test_service().await;
        let id = add_keyring_connection(&service).await;

        service
            .move_credential("Keyring Jira", SecretStore::Plaintext)
            .await
            .unwrap();

        let row = credential_row(&service, "Keyring Jira").await;
        assert_eq!(row.secret_store, SecretStore::Plaintext);
        assert_eq!(row.credential, "keyring-token");
        assert_eq!(token(&service, id).await, "keyring-token");
        assert!(keyring_passwords(&service).is_empty());
    }

    #[tokio::test]
    async fn move_credential_rejects_same_store() {
        let service = test_service().await;
        add_keyring_connection(&service).await;

        for (name, store) in [
            ("Test Jira", SecretStore::Plaintext),
            ("Keyring Jira", SecretStore::Keyring),
        ] {
            let row = credential_row(&service, name).await;

            let err = service.move_credential(name, store).await.unwrap_err();

            assert!(matches!(
                err,
                NirvanaError::Domain(DomainError::CredentialAlreadyIn(n, s)) if n == name && s == store
            ));
            assert_eq!(credential_row(&service, name).await, row);
        }
        assert_eq!(keyring_passwords(&service), ["keyring-token"]);
    }

    #[tokio::test]
    async fn move_credential_rejects_env() {
        let service = test_service().await;
        let row = credential_row(&service, "Test Jira").await;

        let err = service
            .move_credential("Test Jira", SecretStore::Env)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Secret(SecretError::UnsupportedStore(SecretStore::Env))
        ));
        assert_eq!(credential_row(&service, "Test Jira").await, row);
    }

    #[tokio::test]
    async fn credential_changes_reject_missing_connection() {
        let mut service = test_service().await;
        service.create_profile("work".to_string()).await.unwrap();

        for profile in ["test", "work"] {
            service.switch_profile(profile).await.unwrap();
            let name = if profile == "test" {
                "missing"
            } else {
                "Test Jira"
            };

            let got = service.get_credential(name).await.map(|_| ());
            let updated = service.update_credential(name, "new-token".into()).await;
            let moved = service.move_credential(name, SecretStore::Keyring).await;

            for err in [got.unwrap_err(), updated.unwrap_err(), moved.unwrap_err()] {
                assert!(matches!(
                    err,
                    NirvanaError::Domain(DomainError::ConnectionNotFound(n)) if n == name
                ));
            }
        }
        service.switch_profile("test").await.unwrap();
        assert_eq!(
            credential_row(&service, "Test Jira").await.credential,
            "token"
        );
    }

    #[tokio::test]
    async fn move_to_unavailable_keyring_keeps_plaintext_row() {
        let mut service = test_service().await;
        let row = credential_row(&service, "Test Jira").await;
        unavailable_keyring(&mut service);

        let err = service
            .move_credential("Test Jira", SecretStore::Keyring)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Secret(SecretError::KeyringUnavailable(_))
        ));
        assert_eq!(credential_row(&service, "Test Jira").await, row);
    }

    #[tokio::test]
    async fn move_to_keyring_removes_entry_when_row_update_fails() {
        let service = test_service().await;
        let row = credential_row(&service, "Test Jira").await;
        service
            .db
            .execute_unprepared(
                "CREATE TRIGGER fail_update BEFORE UPDATE ON credentials \
                 BEGIN SELECT RAISE(ABORT, 'update failed'); END",
            )
            .await
            .unwrap();

        let err = service
            .move_credential("Test Jira", SecretStore::Keyring)
            .await
            .unwrap_err();

        assert!(matches!(err, NirvanaError::Db(_)), "{err:?}");
        assert_eq!(credential_row(&service, "Test Jira").await, row);
        assert!(keyring_passwords(&service).is_empty());
    }

    #[tokio::test]
    async fn update_on_unavailable_keyring_keeps_token() {
        let mut service = test_service().await;
        let id = add_keyring_connection(&service).await;
        let keyring = service.keyring.clone();
        unavailable_keyring(&mut service);

        let err = service
            .update_credential("Keyring Jira", "new-token".into())
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Secret(SecretError::KeyringUnavailable(_))
        ));
        service.keyring = keyring;
        assert_eq!(token(&service, id).await, "keyring-token");
    }

    #[tokio::test]
    async fn move_from_unavailable_keyring_keeps_keyring_row() {
        let mut service = test_service().await;
        add_keyring_connection(&service).await;
        let row = credential_row(&service, "Keyring Jira").await;
        unavailable_keyring(&mut service);

        let err = service
            .move_credential("Keyring Jira", SecretStore::Plaintext)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Secret(SecretError::KeyringUnavailable(_))
        ));
        assert_eq!(credential_row(&service, "Keyring Jira").await, row);
    }
}
