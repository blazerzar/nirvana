use std::path::PathBuf;

pub struct AppInfo {
    pub version: String,
    pub config_file: PathBuf,
    pub db_file: PathBuf,
    pub log_file: PathBuf,
    pub sqlite_version: String,
    pub keyring: Result<String, String>,
    pub is_dev: bool,
}
