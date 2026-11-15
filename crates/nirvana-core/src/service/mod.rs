mod config;
mod connection;
mod credential;
mod info;
mod logs;
mod profile;
mod publish;
#[cfg(test)]
mod test_service;
#[cfg(test)]
mod test_support;
#[cfg(feature = "test-support")]
mod testing;
mod ticket;
mod tracking;

use std::sync::Arc;

use sea_orm::DatabaseConnection;
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
    config: Result<AppConfig, Arc<ConfigError>>,
    db: DatabaseConnection,
    keyring: StoreFactory,
    http: Result<reqwest::Client, Arc<reqwest::Error>>,
}

impl Nirvana {
    pub async fn new() -> Result<Self, NirvanaError> {
        Self::open(AppPaths::resolve()?, native_store()).await
    }

    async fn open(paths: AppPaths, keyring: StoreFactory) -> Result<Self, NirvanaError> {
        let mut config = AppConfig::load(&paths).map_err(Arc::new);
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
            config,
            db,
            keyring,
            http,
        })
    }

    fn config(&self) -> Result<&AppConfig, NirvanaError> {
        self.config.as_ref().map_err(|e| e.clone().into())
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

    fn config_mut(&mut self) -> Result<&mut AppConfig, NirvanaError> {
        self.config.as_mut().map_err(|e| e.clone().into())
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
            service.config(),
            Err(NirvanaError::ConfigUnavailable(_))
        ));
        service.list_profiles().await.unwrap();
    }
}
