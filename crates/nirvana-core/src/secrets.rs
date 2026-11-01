//! Connection tokens and where they are stored.
//!
//! Each connection has one `credentials` row. Its `secret_store` decides what
//! the `credential` column holds:
//!
//! - `plaintext`: the token.
//! - `keyring`: the keyring account `nirvana_<profile>_<connection>_<key>`,
//!   with a random 8 hex digit key. The token is stored in the OS keyring
//!   under service `nirvana` and that account (Keychain on macOS, Credential
//!   Manager on Windows, Secret Service on Linux).
//! - `env`: not supported yet.
//!
//! Keyring calls are synchronous and can show unlock dialogs, so every call,
//! including the `StoreFactory` that creates the store and dropping the store,
//! runs in `spawn_blocking`.

use std::sync::Arc;

use keyring_core::CredentialStore;
pub use nirvana_entity::credential::{Entity as CredentialEntity, SecretStore};
use sea_orm::{DatabaseConnection, DbErr, EntityTrait};
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;
use tracing::warn;

use crate::errors::NirvanaError;

#[cfg(target_os = "macos")]
type Store = apple_native_keyring_store::keychain::Store;
#[cfg(target_os = "windows")]
type Store = windows_native_keyring_store::store::Store;
#[cfg(target_os = "linux")]
type Store = zbus_secret_service_keyring_store::store::Store;

const KEYRING_SERVICE: &str = "nirvana";

pub(crate) type StoreFactory =
    Arc<dyn Fn() -> keyring_core::Result<Arc<CredentialStore>> + Send + Sync>;

pub(crate) fn native_store() -> StoreFactory {
    Arc::new(|| Ok(Store::new()?))
}

#[cfg(test)]
pub(crate) fn mock_store() -> StoreFactory {
    let store = keyring_core::mock::Store::new().unwrap();
    Arc::new(move || Ok(store.clone()))
}

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("keyring unavailable: {0}")]
    KeyringUnavailable(String),
    #[error("no keyring entry for connection {connection_id}")]
    MissingEntry { connection_id: i64 },
    #[error("unsupported secret store: {0}")]
    UnsupportedStore(SecretStore),
    #[error(transparent)]
    Keyring(#[from] keyring_core::Error),
    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),
}

/// Secret stores offered to users when adding a connection. Env store is only
/// used for CI and testing.
pub fn selectable_stores() -> &'static [SecretStore] {
    &[SecretStore::Keyring, SecretStore::Plaintext]
}

fn unavailable(error: keyring_core::Error) -> SecretError {
    SecretError::KeyringUnavailable(error.to_string())
}

/// Returns the keyring backend name, or why the keyring is unavailable.
pub(crate) async fn status(keyring: &StoreFactory) -> Result<String, String> {
    let keyring = keyring.clone();
    tokio::task::spawn_blocking(move || {
        keyring()
            .map(|store| vendor_name(&store.vendor()))
            .map_err(|e| e.to_string())
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
}

fn vendor_name(vendor: &str) -> String {
    vendor.split(',').next().unwrap_or(vendor).to_string()
}

/// Returns the `credential` column value for a new connection. For
/// `keyring` the token is saved in the keyring under
/// `nirvana_<profile>_<connection>_<key>` and that account is returned.
pub(crate) async fn store_secret(
    keyring: &StoreFactory,
    profile: &str,
    connection: &str,
    store: SecretStore,
    token: SecretString,
) -> Result<String, NirvanaError> {
    match store {
        SecretStore::Plaintext => Ok(token.expose_secret().to_string()),
        SecretStore::Keyring => Ok(store_in_keychain(keyring, profile, connection, token).await?),
        SecretStore::Env => Err(SecretError::UnsupportedStore(store).into()),
    }
}

async fn store_in_keychain(
    keyring: &StoreFactory,
    profile: &str,
    connection: &str,
    token: SecretString,
) -> Result<String, SecretError> {
    let account = format!(
        "nirvana_{profile}_{connection}_{:08x}",
        rand::random::<u32>()
    );
    let keyring = keyring.clone();
    tokio::task::spawn_blocking(move || {
        keyring()
            .map_err(unavailable)?
            .build(KEYRING_SERVICE, &account, None)?
            .set_password(token.expose_secret())?;
        Ok(account)
    })
    .await?
}

/// Returns the token of a connection, reading it from the keyring for
/// `keyring` credentials.
#[cfg_attr(not(test), expect(dead_code))]
pub(crate) async fn read_credential(
    keyring: &StoreFactory,
    connection_id: i64,
    db: &DatabaseConnection,
) -> Result<SecretString, NirvanaError> {
    let credential = CredentialEntity::find_by_id(connection_id)
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound(format!(
            "credential for connection {connection_id}"
        )))?;

    let token = match credential.secret_store {
        SecretStore::Plaintext => credential.credential,
        SecretStore::Keyring => {
            read_from_keychain(keyring, connection_id, credential.credential).await?
        }
        SecretStore::Env => {
            return Err(SecretError::UnsupportedStore(credential.secret_store).into());
        }
    };
    Ok(token.into())
}

async fn read_from_keychain(
    keyring: &StoreFactory,
    connection_id: i64,
    account: String,
) -> Result<String, SecretError> {
    let keyring = keyring.clone();
    tokio::task::spawn_blocking(move || {
        keyring()
            .map_err(unavailable)?
            .build(KEYRING_SERVICE, &account, None)?
            .get_password()
            .map_err(|e| match e {
                keyring_core::Error::NoEntry => SecretError::MissingEntry { connection_id },
                e => e.into(),
            })
    })
    .await?
}

/// Deletes the keyring entry of `account`. Failures are logged instead of
/// returned.
pub(crate) async fn delete_from_keychain(keyring: &StoreFactory, account: String) {
    if let Err(e) = delete_entry(keyring, account).await {
        warn!(error = %e, "failed to delete keyring entry");
    }
}

async fn delete_entry(keyring: &StoreFactory, account: String) -> Result<(), SecretError> {
    let keyring = keyring.clone();
    tokio::task::spawn_blocking(move || {
        match keyring()
            .map_err(unavailable)?
            .build(KEYRING_SERVICE, &account, None)?
            .delete_credential()
        {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretError::from(e)),
        }
    })
    .await?
}

#[cfg(test)]
mod tests {
    //! The `native_keyring` tests use the OS keyring and are ignored by default.
    //! Run them with `mise run test-keyring` on macOS, Windows, Ubuntu with
    //! GNOME and Fedora with Plasma. On Linux every provider reports
    //! "Secret Service store", so check the provider with
    //! `busctl --user status org.freedesktop.secrets | grep -E '^(Comm|Exe)='`.

    use super::*;

    #[cfg(target_os = "macos")]
    const NATIVE_VENDOR: &str = "macOS Keychain Store";
    #[cfg(target_os = "windows")]
    const NATIVE_VENDOR: &str = "Windows Credential Manager";
    #[cfg(target_os = "linux")]
    const NATIVE_VENDOR: &str = "Secret Service store";

    async fn assert_round_trip(keyring: StoreFactory) {
        let account = store_secret(
            &keyring,
            "blaz",
            "jira-dc",
            SecretStore::Keyring,
            "very-secret-token".into(),
        )
        .await
        .unwrap();

        let read = read_from_keychain(&keyring, 1, account.clone()).await;
        let deleted = delete_entry(&keyring, account.clone()).await;
        let read_after_delete = read_from_keychain(&keyring, 1, account.clone()).await;

        assert_ne!(account, "very-secret-token");
        assert_eq!(read.unwrap(), "very-secret-token");
        deleted.unwrap();
        assert!(
            matches!(
                read_after_delete,
                Err(SecretError::MissingEntry { connection_id: 1 })
            ),
            "{read_after_delete:?}"
        );
    }

    #[tokio::test]
    async fn keyring_secret_round_trips() {
        assert_round_trip(mock_store()).await;
    }

    #[tokio::test]
    #[ignore = "uses the OS keyring, run with `mise run test-keyring`"]
    async fn native_keyring_round_trips() {
        assert_round_trip(native_store()).await;
    }

    #[tokio::test]
    #[ignore = "uses the OS keyring, run with `mise run test-keyring`"]
    async fn native_keyring_is_detected() {
        assert_eq!(status(&native_store()).await, Ok(NATIVE_VENDOR.to_string()));
    }

    #[tokio::test]
    async fn plaintext_secret_is_the_token() {
        let secret = store_secret(
            &mock_store(),
            "blaz",
            "jira-dc",
            SecretStore::Plaintext,
            "not-so-secret-token".into(),
        )
        .await
        .unwrap();

        assert_eq!(secret, "not-so-secret-token");
    }
}
