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
    use crate::secrets::read_credential;
    use crate::service::test_service::TestConnection::{
        Jira, JiraDc, KeyringJira, KeyringLinear, Linear,
    };
    use crate::service::test_service::{TestConnection, TestService};

    const PLAINTEXT_CONNECTIONS: [TestConnection; 3] = [Jira, JiraDc, Linear];
    const KEYRING_CONNECTIONS: [TestConnection; 2] = [KeyringJira, KeyringLinear];
    const CONNECTIONS: [TestConnection; 5] = [Jira, KeyringJira, JiraDc, Linear, KeyringLinear];

    async fn credential_row(
        service: &TestService,
        connection: TestConnection,
    ) -> credential::Model {
        let profile = service.find_active_profile().await.unwrap();
        service
            .find_credential(profile.id, connection.name())
            .await
            .unwrap()
            .1
    }

    async fn expected_credential(service: &TestService, connection: TestConnection) -> Credential {
        let location = match service.connection_data(connection).secret_store {
            SecretStore::Plaintext => SecretLocation::Plaintext,
            SecretStore::Keyring => {
                let account = credential_row(service, connection).await.credential;
                assert_account(&account, connection);
                SecretLocation::Keyring { account }
            }
            SecretStore::Env => unreachable!(),
        };
        Credential {
            connection: connection.name().to_string(),
            location,
        }
    }

    async fn token(service: &TestService, connection: TestConnection) -> String {
        let id = service.connection(connection).await.model.id;
        read_credential(&service.keyring, id, &service.db)
            .await
            .unwrap()
            .expose_secret()
            .to_string()
    }

    fn tokens(connections: &[TestConnection]) -> Vec<&'static str> {
        let mut tokens: Vec<_> = connections.iter().map(|c| c.token()).collect();
        tokens.sort_unstable();
        tokens
    }

    fn assert_account(account: &str, connection: TestConnection) {
        let prefix = format!("nirvana_test_{}_", connection.name());
        let key = account.strip_prefix(&prefix).unwrap();
        assert_eq!(key.len(), 8, "{account}");
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()), "{account}");
    }

    fn unavailable_keyring(service: &mut Nirvana) {
        service.keyring = Arc::new(|| Err(keyring_core::Error::NoDefaultStore));
    }

    #[tokio::test]
    async fn list_credentials_returns_locations_without_tokens() {
        let service = TestService::new(&CONNECTIONS).await;
        let mut expected = Vec::new();
        for connection in CONNECTIONS {
            expected.push(expected_credential(&service, connection).await);
        }

        let credentials = service.list_credentials().await.unwrap();

        assert_eq!(credentials, expected);
    }

    #[tokio::test]
    async fn get_credential_returns_location_without_token() {
        let service = TestService::new(&CONNECTIONS).await;

        for connection in CONNECTIONS {
            let credential = service.get_credential(connection.name()).await.unwrap();

            assert_eq!(credential, expected_credential(&service, connection).await);
        }
    }

    #[tokio::test]
    async fn update_credential_replaces_plaintext_token() {
        let service = TestService::new(&PLAINTEXT_CONNECTIONS).await;

        for connection in PLAINTEXT_CONNECTIONS {
            let row = credential_row(&service, connection).await;

            service
                .update_credential(connection.name(), "new-token".into())
                .await
                .unwrap();

            assert_eq!(
                credential_row(&service, connection).await,
                credential::Model {
                    credential: "new-token".to_string(),
                    ..row
                },
                "{}",
                connection.name()
            );
        }
    }

    #[tokio::test]
    async fn update_credential_replaces_keyring_token() {
        let service = TestService::new(&KEYRING_CONNECTIONS).await;

        for connection in KEYRING_CONNECTIONS {
            let row = credential_row(&service, connection).await;
            let new_token = format!("new-{}", connection.token());

            service
                .update_credential(connection.name(), new_token.clone().into())
                .await
                .unwrap();

            assert_eq!(
                credential_row(&service, connection).await,
                row,
                "{}",
                connection.name()
            );
            assert_eq!(
                token(&service, connection).await,
                new_token,
                "{}",
                connection.name()
            );
        }
        assert_eq!(
            service.keyring_passwords(),
            ["new-keyring-jira-token", "new-keyring-linear-token"]
        );
    }

    #[tokio::test]
    async fn update_credential_rejects_env() {
        let service = TestService::new(&[Jira]).await;
        let row = credential_row(&service, Jira).await;
        set_credential(&service.db, row, SecretStore::Env, "NIRVANA_TOKEN".into())
            .await
            .unwrap();

        let err = service
            .update_credential(Jira.name(), "new-token".into())
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Secret(SecretError::UnsupportedStore(SecretStore::Env))
        ));
    }

    #[tokio::test]
    async fn move_credential_moves_plaintext_to_keyring() {
        let service = TestService::new(&PLAINTEXT_CONNECTIONS).await;

        for connection in PLAINTEXT_CONNECTIONS {
            service
                .move_credential(connection.name(), SecretStore::Keyring)
                .await
                .unwrap();

            let row = credential_row(&service, connection).await;
            assert_eq!(
                row.secret_store,
                SecretStore::Keyring,
                "{}",
                connection.name()
            );
            assert_account(&row.credential, connection);
            assert_eq!(
                token(&service, connection).await,
                connection.token(),
                "{}",
                connection.name()
            );
        }
        assert_eq!(service.keyring_passwords(), tokens(&PLAINTEXT_CONNECTIONS));
    }

    #[tokio::test]
    async fn move_credential_moves_keyring_to_plaintext() {
        let service = TestService::new(&KEYRING_CONNECTIONS).await;

        for connection in KEYRING_CONNECTIONS {
            service
                .move_credential(connection.name(), SecretStore::Plaintext)
                .await
                .unwrap();

            let row = credential_row(&service, connection).await;
            assert_eq!(
                row.secret_store,
                SecretStore::Plaintext,
                "{}",
                connection.name()
            );
            assert_eq!(row.credential, connection.token(), "{}", connection.name());
        }
        assert!(service.keyring_passwords().is_empty());
    }

    #[tokio::test]
    async fn move_credential_rejects_same_store() {
        let service = TestService::new(&[Jira, KeyringJira]).await;

        for (connection, store) in [
            (Jira, SecretStore::Plaintext),
            (KeyringJira, SecretStore::Keyring),
        ] {
            let name = connection.name();
            let row = credential_row(&service, connection).await;

            let err = service.move_credential(name, store).await.unwrap_err();

            assert!(
                matches!(
                    &err,
                    NirvanaError::Domain(DomainError::CredentialAlreadyIn(n, s)) if n == name && *s == store
                ),
                "{name}: {err:?}"
            );
            assert_eq!(credential_row(&service, connection).await, row, "{name}");
        }
        assert_eq!(service.keyring_passwords(), [KeyringJira.token()]);
    }

    #[tokio::test]
    async fn move_credential_rejects_env() {
        let service = TestService::new(&[Jira]).await;
        let row = credential_row(&service, Jira).await;

        let err = service
            .move_credential(Jira.name(), SecretStore::Env)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Secret(SecretError::UnsupportedStore(SecretStore::Env))
        ));
        assert_eq!(credential_row(&service, Jira).await, row);
    }

    #[tokio::test]
    async fn credential_changes_reject_missing_connection() {
        let mut service = TestService::new(&[Jira]).await;
        service.create_profile("work".to_string()).await.unwrap();

        for (profile, name) in [("test", "missing"), ("work", Jira.name())] {
            service.switch_profile(profile).await.unwrap();

            let got = service.get_credential(name).await.map(|_| ());
            let updated = service.update_credential(name, "new-token".into()).await;
            let moved = service.move_credential(name, SecretStore::Keyring).await;

            for err in [got.unwrap_err(), updated.unwrap_err(), moved.unwrap_err()] {
                assert!(
                    matches!(
                        &err,
                        NirvanaError::Domain(DomainError::ConnectionNotFound(n)) if n == name
                    ),
                    "{profile}: {err:?}"
                );
            }
        }
        service.switch_profile("test").await.unwrap();
        assert_eq!(
            credential_row(&service, Jira).await.credential,
            Jira.token()
        );
    }

    #[tokio::test]
    async fn move_to_unavailable_keyring_keeps_plaintext_row() {
        let mut service = TestService::new(&[Jira]).await;
        let row = credential_row(&service, Jira).await;
        unavailable_keyring(&mut service);

        let err = service
            .move_credential(Jira.name(), SecretStore::Keyring)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Secret(SecretError::KeyringUnavailable(_))
        ));
        assert_eq!(credential_row(&service, Jira).await, row);
    }

    #[tokio::test]
    async fn move_to_keyring_removes_entry_when_row_update_fails() {
        let service = TestService::new(&[Jira]).await;
        let row = credential_row(&service, Jira).await;
        service
            .db
            .execute_unprepared(
                "CREATE TRIGGER fail_update BEFORE UPDATE ON credentials \
                 BEGIN SELECT RAISE(ABORT, 'update failed'); END",
            )
            .await
            .unwrap();

        let err = service
            .move_credential(Jira.name(), SecretStore::Keyring)
            .await
            .unwrap_err();

        assert!(matches!(err, NirvanaError::Db(_)), "{err:?}");
        assert_eq!(credential_row(&service, Jira).await, row);
        assert!(service.keyring_passwords().is_empty());
    }

    #[tokio::test]
    async fn update_on_unavailable_keyring_keeps_token() {
        let mut service = TestService::new(&[KeyringJira]).await;
        let keyring = service.keyring.clone();
        unavailable_keyring(&mut service);

        let err = service
            .update_credential(KeyringJira.name(), "new-token".into())
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Secret(SecretError::KeyringUnavailable(_))
        ));
        service.keyring = keyring;
        assert_eq!(token(&service, KeyringJira).await, KeyringJira.token());
    }

    #[tokio::test]
    async fn move_from_unavailable_keyring_keeps_keyring_row() {
        let mut service = TestService::new(&[KeyringJira]).await;
        let row = credential_row(&service, KeyringJira).await;
        unavailable_keyring(&mut service);

        let err = service
            .move_credential(KeyringJira.name(), SecretStore::Plaintext)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Secret(SecretError::KeyringUnavailable(_))
        ));
        assert_eq!(credential_row(&service, KeyringJira).await, row);
    }
}
