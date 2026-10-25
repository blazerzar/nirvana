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
//! including creating and dropping the store, runs in `spawn_blocking`.

use keyring_core::api::CredentialStoreApi;
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
pub async fn status() -> Result<String, String> {
    tokio::task::spawn_blocking(|| {
        Store::new()
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
    profile: &str,
    connection: &str,
    store: SecretStore,
    token: SecretString,
) -> Result<String, NirvanaError> {
    match store {
        SecretStore::Plaintext => Ok(token.expose_secret().to_string()),
        SecretStore::Keyring => Ok(store_in_keychain(profile, connection, token).await?),
        SecretStore::Env => Err(SecretError::UnsupportedStore(store).into()),
    }
}

async fn store_in_keychain(
    profile: &str,
    connection: &str,
    token: SecretString,
) -> Result<String, SecretError> {
    let account = format!(
        "nirvana_{profile}_{connection}_{:08x}",
        rand::random::<u32>()
    );
    tokio::task::spawn_blocking(move || {
        Store::new()
            .map_err(unavailable)?
            .build(KEYRING_SERVICE, &account, None)?
            .set_password(token.expose_secret())?;
        Ok(account)
    })
    .await?
}

/// Returns the token of a connection, reading it from the keyring for
/// `keyring` credentials.
pub(crate) async fn read_credential(
    connection_id: i64,
    db: &DatabaseConnection,
) -> Result<SecretString, NirvanaError> {
    let credential = CredentialEntity::find_by_id(connection_id)
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound(format!(
            "credential for connection {connection_id}"
        )))?;

    match credential.secret_store {
        SecretStore::Plaintext => Ok(credential.credential.into()),
        SecretStore::Keyring => Ok(read_from_keychain(connection_id, credential.credential)
            .await?
            .into()),
        SecretStore::Env => Err(SecretError::UnsupportedStore(credential.secret_store).into()),
    }
}

async fn read_from_keychain(connection_id: i64, account: String) -> Result<String, SecretError> {
    tokio::task::spawn_blocking(move || {
        Store::new()
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
pub(crate) async fn delete_from_keychain(account: String) {
    let deleted = tokio::task::spawn_blocking(move || {
        match Store::new()
            .map_err(unavailable)?
            .build(KEYRING_SERVICE, &account, None)?
            .delete_credential()
        {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretError::from(e)),
        }
    })
    .await
    .unwrap_or_else(|e| Err(e.into()));
    if let Err(e) = deleted {
        warn!(error = %e, "failed to delete keyring entry");
    }
}
