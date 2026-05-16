use std::env;
use std::path::PathBuf;

pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
    pub config_file: PathBuf,
    pub db_file: PathBuf,
    pub log_file: PathBuf,
    pub is_dev: bool,
}

impl AppPaths {
    pub fn resolve() -> Self {
        let is_dev = Self::is_dev_mode();

        let (config_dir, data_dir, log_dir) = if is_dev {
            let exe_dir = Self::exe_dir();
            (exe_dir.clone(), exe_dir.clone(), exe_dir.clone())
        } else {
            let (config_dir, data_dir, log_dir) = Self::platform_dirs();
            (
                env::var("NIRVANA_CONFIG_DIR")
                    .map(PathBuf::from)
                    .unwrap_or(config_dir),
                env::var("NIRVANA_DATA_DIR")
                    .map(PathBuf::from)
                    .unwrap_or(data_dir),
                env::var("NIRVANA_LOG_DIR")
                    .map(PathBuf::from)
                    .unwrap_or(log_dir),
            )
        };

        let config_file = config_dir.join("config.toml");
        let db_file = data_dir.join("nirvana.db");
        let log_file = log_dir.join("nirvana.log");

        Self {
            config_dir,
            data_dir,
            log_dir,
            config_file,
            db_file,
            log_file,
            is_dev,
        }
    }

    pub fn ensure_dirs_exist(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.log_dir)?;
        Ok(())
    }

    fn is_dev_mode() -> bool {
        match env::var("NIRVANA_DEV").as_deref() {
            Ok("0") => false,
            Ok("1") => true,
            _ => cfg!(debug_assertions),
        }
    }

    fn exe_dir() -> PathBuf {
        env::current_exe()
            .expect("failed to get current exe path")
            .parent()
            .expect("exe has no parent directory")
            .to_path_buf()
    }

    fn platform_dirs() -> (PathBuf, PathBuf, PathBuf) {
        let dirs = directories::ProjectDirs::from("", "", "nirvana")
            .expect("failed to determine platform directories");
        (
            dirs.config_dir().to_path_buf(),
            dirs.data_dir().to_path_buf(),
            dirs.state_dir()
                .map(|d| d.to_path_buf())
                // Windows and macOS do not have state dir, macOS falls back
                // to the same dir as config and data while Windows falls
                // back  to %LocalAppData%
                .unwrap_or_else(|| dirs.data_local_dir().to_path_buf()),
        )
    }
}
