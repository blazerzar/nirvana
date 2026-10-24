use crate::errors::NirvanaError;
use crate::service::{AppInfo, Nirvana};
use crate::storage;

impl Nirvana {
    pub async fn info(&self) -> Result<AppInfo, NirvanaError> {
        Ok(AppInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            config_file: self.paths.config_file.clone(),
            db_file: self.paths.db_file.clone(),
            log_file: self.paths.log_file.clone(),
            sqlite_version: storage::sqlite_version(&self.db).await?,
            is_dev: self.paths.is_dev,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::service::test_support::test_api;

    #[tokio::test]
    async fn info_reports_sqlite_version() {
        let api = test_api().await;

        let info = api.info().await.unwrap();

        assert!(info.sqlite_version.starts_with("3."));
    }
}
