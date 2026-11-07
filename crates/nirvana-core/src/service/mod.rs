mod connection;
mod credential;
mod info;
mod logs;
mod profile;
#[cfg(test)]
mod test_support;

use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tracing::warn;

use crate::config::{AppConfig, ConfigError};
use crate::errors::NirvanaError;
use crate::paths::AppPaths;
use crate::secrets::{StoreFactory, native_store};
use crate::storage;

pub struct Nirvana {
    paths: AppPaths,
    config: Result<AppConfig, Arc<ConfigError>>,
    db: DatabaseConnection,
    keyring: StoreFactory,
}

impl Nirvana {
    pub async fn new() -> Result<Self, NirvanaError> {
        let paths = AppPaths::resolve()?;
        let mut config = AppConfig::load(&paths).map_err(Arc::new);
        let db = storage::open(&paths.db_file).await?;
        match &mut config {
            Ok(config) => {
                config.core.profile = profile::resolve(&db, config.core.profile.take()).await?
            }
            Err(e) => warn!("config unavailable: {e}"),
        }
        Ok(Self {
            paths,
            config,
            db,
            keyring: native_store(),
        })
    }

    fn config(&self) -> Result<&AppConfig, NirvanaError> {
        self.config.as_ref().map_err(|e| e.clone().into())
    }

    fn config_mut(&mut self) -> Result<&mut AppConfig, NirvanaError> {
        self.config.as_mut().map_err(|e| e.clone().into())
    }
}
