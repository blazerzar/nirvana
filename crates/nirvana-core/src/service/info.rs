use crate::errors::NirvanaError;
use crate::service::{AppInfo, Nirvana};
use crate::{secrets, storage};

impl Nirvana {
    pub async fn info(&self) -> Result<AppInfo, NirvanaError> {
        Ok(AppInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            config_file: self.paths.config_file.clone(),
            db_file: self.paths.db_file.clone(),
            log_file: self.paths.log_file.clone(),
            sqlite_version: storage::sqlite_version(&self.db).await?,
            keyring: self.keyring_status().await,
            is_dev: self.paths.is_dev,
        })
    }

    /// Returns the keyring backend name, or why the keyring is unavailable.
    pub async fn keyring_status(&self) -> Result<String, String> {
        secrets::status(&self.keyring).await
    }
}

#[cfg(test)]
mod tests {
    use crate::service::test_support::test_service;

    #[tokio::test]
    async fn info_reports_sqlite_version() {
        let service = test_service().await;

        let info = service.info().await.unwrap();

        assert!(info.sqlite_version.starts_with("3."));
    }

    #[tokio::test]
    async fn info_reports_keyring_vendor() {
        let service = test_service().await;

        let info = service.info().await.unwrap();

        assert_eq!(info.keyring, Ok("Mock store".to_string()));
    }
}
