mod connection;
mod delete;
mod edit;
mod info;
mod publish;
mod settings;
mod slots;
mod tracking;

use std::path::PathBuf;

use sea_orm::DatabaseConnection;

use crate::config::AppConfig;
pub use crate::domain::{Change, SlotCreate, SlotEdit, SlotSort};
use crate::errors::NirvanaError;
use crate::paths::AppPaths;
use crate::storage;

pub struct Nirvana {
    paths: AppPaths,
    config: AppConfig,
    db: DatabaseConnection,
}

pub struct AppInfo {
    pub version: String,
    pub config_file: PathBuf,
    pub db_file: PathBuf,
    pub log_file: PathBuf,
    pub is_dev: bool,
}

impl Nirvana {
    pub async fn new() -> Result<Self, NirvanaError> {
        let paths = AppPaths::resolve()?;
        let config = AppConfig::load(&paths.config_file)?;
        let db = storage::open(&paths.db_file).await?;
        Ok(Self { paths, config, db })
    }
}
