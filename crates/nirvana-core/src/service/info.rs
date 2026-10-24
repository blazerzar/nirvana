use crate::service::{AppInfo, Nirvana};

impl Nirvana {
    pub fn info(&self) -> AppInfo {
        AppInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            config_file: self.paths.config_file.clone(),
            db_file: self.paths.db_file.clone(),
            log_file: self.paths.log_file.clone(),
            is_dev: self.paths.is_dev,
        }
    }
}
