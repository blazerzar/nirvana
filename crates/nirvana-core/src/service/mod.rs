mod connection;
mod delete;
mod edit;
mod info;
mod profile;
mod publish;
mod slots;
#[cfg(test)]
mod test_support;
mod tracking;

use std::path::PathBuf;
use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tracing::warn;

use crate::config::{AppConfig, ConfigError};
pub use crate::domain::{Change, SlotCreate, SlotEdit, SlotSort};
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

pub struct AppInfo {
    pub version: String,
    pub config_file: PathBuf,
    pub db_file: PathBuf,
    pub log_file: PathBuf,
    pub sqlite_version: String,
    pub keyring: Result<String, String>,
    pub is_dev: bool,
}

impl Nirvana {
    pub async fn new() -> Result<Self, NirvanaError> {
        let paths = AppPaths::resolve()?;
        let mut config = AppConfig::load(&paths).map_err(Arc::new);
        let db = storage::open(&paths.db_file).await?;
        match &mut config {
            Ok(config) => config.core.profile = profile::resolve(&db, config.core.profile).await?,
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
}
