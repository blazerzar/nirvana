mod config;
mod connection;
mod credential;
mod info;
mod logs;
mod profile;
mod publish;
#[cfg(test)]
mod test_service;
#[cfg(feature = "test-support")]
mod testing;
mod ticket;
mod tracking;

use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use tracing::warn;

use crate::config::{AppConfig, ConfigError};
use crate::domain::Connection;
use crate::errors::NirvanaError;
use crate::integration::{self, Integration, IntegrationError};
use crate::paths::AppPaths;
use crate::secrets::{StoreFactory, native_store, read_credential};
use crate::storage;

pub struct Nirvana {
    paths: AppPaths,
    config: Result<RwLock<AppConfig>, Arc<ConfigError>>,
    db: DatabaseConnection,
    keyring: StoreFactory,
    http: Result<reqwest::Client, Arc<reqwest::Error>>,
}

impl Nirvana {
    pub async fn new() -> Result<Self, NirvanaError> {
        Self::open(AppPaths::resolve()?, native_store()).await
    }

    async fn open(paths: AppPaths, keyring: StoreFactory) -> Result<Self, NirvanaError> {
        let mut config = AppConfig::load(&paths).await.map_err(Arc::new);
        let db = storage::open(&paths.db_file).await?;
        match &mut config {
            Ok(config) => {
                config.core.profile = profile::resolve(&db, config.core.profile.take()).await?
            }
            Err(e) => warn!("config unavailable: {e}"),
        }
        let http = integration::http_client().map_err(Arc::new);
        Ok(Self {
            paths,
            config: config.map(RwLock::new),
            db,
            keyring,
            http,
        })
    }

    async fn config(&self) -> Result<RwLockReadGuard<'_, AppConfig>, NirvanaError> {
        match &self.config {
            Ok(config) => Ok(config.read().await),
            Err(e) => Err(e.clone().into()),
        }
    }

    fn http(&self) -> Result<reqwest::Client, IntegrationError> {
        self.http
            .clone()
            .map_err(IntegrationError::ClientUnavailable)
    }

    /// Reads the token of `connection` and builds its integration.
    async fn integration(&self, connection: &Connection) -> Result<Integration, NirvanaError> {
        let token = read_credential(&self.keyring, connection.model.id, &self.db).await?;
        Integration::new(connection, self.http()?, token)
    }

    /// Returns the config for a change. Hold the guard until the changed
    /// config is saved, so concurrent changes are not lost.
    async fn config_mut(&self) -> Result<RwLockWriteGuard<'_, AppConfig>, NirvanaError> {
        match &self.config {
            Ok(config) => Ok(config.write().await),
            Err(e) => Err(e.clone().into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::mock_store;

    #[tokio::test]
    async fn open_with_invalid_config_keeps_database_available() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::in_dir(dir.path());
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        std::fs::write(&paths.config_file, "not toml").unwrap();

        let service = Nirvana::open(paths, mock_store()).await.unwrap();

        assert!(matches!(
            service.config().await,
            Err(NirvanaError::ConfigUnavailable(_))
        ));
        service.list_profiles().await.unwrap();
    }
}
