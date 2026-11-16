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
//! - `env`: the name of the environment variable holding the token, which
//!   is `NIRVANA_` followed by uppercase letters, digits and underscores. The
//!   name is checked when adding and when reading a connection. An empty
//!   variable counts as not set.
//!
//! The token of an existing connection can be updated in place, or moved
//! between `plaintext` and `keyring`. The keyring is written before the row,
//! so the row is left untouched when the keyring is unavailable.
//!
//! Keyring calls are synchronous and can show unlock dialogs, so every call,
//! including the `StoreFactory` that creates the store and dropping the store,
//! runs in `spawn_blocking`.

use std::env;
use std::sync::Arc;

use keyring_core::CredentialStore;
use nirvana_entity::credential::Entity as CredentialEntity;
use sea_orm::{DatabaseConnection, DbErr, EntityTrait};
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;
use tracing::warn;

use crate::domain::SecretStore;
use crate::errors::NirvanaError;

#[cfg(target_os = "macos")]
type Store = apple_native_keyring_store::keychain::Store;
#[cfg(target_os = "windows")]
type Store = windows_native_keyring_store::store::Store;
#[cfg(target_os = "linux")]
type Store = zbus_secret_service_keyring_store::store::Store;

pub(crate) const KEYRING_SERVICE: &str = "nirvana";
const ENV_PREFIX: &str = "NIRVANA_";

pub(crate) type StoreFactory =
    Arc<dyn Fn() -> keyring_core::Result<Arc<CredentialStore>> + Send + Sync>;

#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) fn native_store() -> StoreFactory {
    Arc::new(|| Ok(Store::new()?))
}

#[cfg(any(test, feature = "test-support"))]
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
    #[error("environment variable name must match {ENV_PREFIX}[A-Z0-9_]+")]
    InvalidEnvName,
    #[error("environment variable {0} is not set or empty")]
    MissingEnv(String),
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
/// `nirvana_<profile>_<connection>_<key>` and that account is returned. For
/// `env` the token is the name of the environment variable.
pub(crate) async fn store_secret(
    keyring: &StoreFactory,
    profile: &str,
    connection: &str,
    store: SecretStore,
    token: SecretString,
) -> Result<String, NirvanaError> {
    match store {
        SecretStore::Plaintext => Ok(token.expose_secret().to_string()),
        SecretStore::Keyring => {
            let account = keyring_account(profile, connection);
            Ok(set_in_keychain(keyring, account, token).await?)
        }
        SecretStore::Env => {
            let name = token.expose_secret();
            check_env_name(name)?;
            Ok(name.to_string())
        }
    }
}

fn check_env_name(name: &str) -> Result<(), SecretError> {
    match name.strip_prefix(ENV_PREFIX) {
        Some(rest)
            if !rest.is_empty()
                && rest
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_') =>
        {
            Ok(())
        }
        _ => Err(SecretError::InvalidEnvName),
    }
}

pub(crate) fn keyring_account(profile: &str, connection: &str) -> String {
    format!(
        "nirvana_{profile}_{connection}_{:08x}",
        rand::random::<u32>()
    )
}

/// Saves `token` in the keyring under `account`, replacing an existing token,
/// and returns `account`.
pub(crate) async fn set_in_keychain(
    keyring: &StoreFactory,
    account: String,
    token: SecretString,
) -> Result<String, SecretError> {
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
/// `keyring` credentials and from the environment for `env` credentials.
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
        SecretStore::Env => read_env(&credential.credential, |name| env::var(name).ok())?,
    };
    Ok(token.into())
}

fn read_env(name: &str, var: impl Fn(&str) -> Option<String>) -> Result<String, SecretError> {
    check_env_name(name)?;
    var(name)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| SecretError::MissingEnv(name.to_string()))
}

pub(crate) async fn read_from_keychain(
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
    if let Err(e) = delete_entry(keyring, account.clone()).await {
        warn!(%account, error = %e, "failed to delete keyring entry");
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

    #[test]
    fn env_reads_set_variable() {
        let token = read_env("NIRVANA_TOKEN", |name| {
            (name == "NIRVANA_TOKEN").then(|| "env-token".to_string())
        });

        assert_eq!(token.unwrap(), "env-token");
    }

    #[test]
    fn env_reports_missing_variable() {
        for value in [None, Some(String::new())] {
            let err = read_env("NIRVANA_TOKEN", |_| value.clone()).unwrap_err();

            assert!(
                matches!(&err, SecretError::MissingEnv(name) if name == "NIRVANA_TOKEN"),
                "{err:?}"
            );
        }
    }

    #[test]
    fn env_name_is_nirvana_prefix_with_uppercase_digits_and_underscores() {
        for name in ["NIRVANA_TOKEN", "NIRVANA_JIRA_2"] {
            assert!(check_env_name(name).is_ok(), "{name}");
        }
        for name in [
            "HOME",
            "NIRVANA_",
            "NIRVANA_TOKEN ",
            "NIRVANA_token",
            "NIRVANA_A=B",
        ] {
            assert!(
                matches!(check_env_name(name), Err(SecretError::InvalidEnvName)),
                "{name}"
            );
        }
    }
}
