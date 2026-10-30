mod connection;
mod delete;
mod edit;
mod info;
mod publish;
mod slots;
#[cfg(test)]
mod test_support;
mod tracking;

use std::path::PathBuf;
use std::sync::Arc;

use nirvana_entity::profile;
use nirvana_entity::profile::Entity as Profile;
use sea_orm::{DatabaseConnection, EntityTrait, QueryOrder};
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
            Ok(config) => config.core.profile = resolve_profile(&db, config.core.profile).await?,
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

async fn resolve_profile(
    db: &DatabaseConnection,
    configured: Option<i64>,
) -> Result<Option<i64>, NirvanaError> {
    if let Some(id) = configured
        && Profile::find_by_id(id).one(db).await?.is_some()
    {
        return Ok(Some(id));
    }
    Ok(Profile::find()
        .order_by_asc(profile::COLUMN.id)
        .one(db)
        .await?
        .map(|profile| profile.id))
}
