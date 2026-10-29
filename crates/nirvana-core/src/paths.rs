//! Locations of the config file, database and log file.
//!
//! # Dev mode
//!
//! Enabled in debug builds, or forced with `NIRVANA_DEV=1` (`NIRVANA_DEV=0`
//! disables it in debug builds). All files are placed next to the executable,
//! e.g. `target/debug/` for `cargo run`. The `NIRVANA_*_DIR` overrides are
//! ignored.
//!
//! # Release mode
//!
//! Linux (respects `XDG_CONFIG_HOME`, `XDG_DATA_HOME` and `XDG_STATE_HOME`):
//!
//! - config: `~/.config/nirvana/config.toml`
//! - database: `~/.local/share/nirvana/nirvana.db`
//! - log: `~/.local/state/nirvana/nirvana.log`
//!
//! macOS:
//!
//! - config: `~/Library/Application Support/nirvana/config.toml`
//! - database: `~/Library/Application Support/nirvana/nirvana.db`
//! - log: `~/Library/Logs/nirvana/nirvana.log`
//!
//! Windows:
//!
//! - config: `%APPDATA%\nirvana\config\config.toml`
//! - database: `%APPDATA%\nirvana\data\nirvana.db`
//! - log: `%LOCALAPPDATA%\nirvana\data\nirvana.log`
//!
//! Each directory can be overridden with `NIRVANA_CONFIG_DIR`,
//! `NIRVANA_DATA_DIR` and `NIRVANA_LOG_DIR`.

use std::path::{Path, PathBuf};
use std::{env, fs, io};

use directories::ProjectDirs;

pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    #[allow(unused)]
    pub log_dir: PathBuf,
    pub config_file: PathBuf,
    pub db_file: PathBuf,
    pub log_file: PathBuf,
    pub is_dev: bool,
}

impl AppPaths {
    /// Resolves the config, data and log locations for the current mode and
    /// platform, as described in the [module docs](self).
    ///
    /// Reads `NIRVANA_DEV` and, in release mode, the `NIRVANA_*_DIR`
    /// overrides. Empty values are ignored. Directories are not created.
    ///
    /// # Errors
    ///
    /// Returns an error if the platform directories or the current executable
    /// path cannot be determined.
    pub fn resolve() -> io::Result<Self> {
        Self::resolve_with(|key| env::var(key).ok())
    }

    fn resolve_with(var: impl Fn(&str) -> Option<String>) -> io::Result<Self> {
        let var = |key: &str| var(key).filter(|value| !value.is_empty());
        let is_dev = Self::is_dev_mode(&var);

        let (config_dir, data_dir, log_dir) = if is_dev {
            let exe_dir = Self::exe_dir()?;
            (exe_dir.clone(), exe_dir.clone(), exe_dir.clone())
        } else {
            let dirs = ProjectDirs::from("", "", "nirvana")
                .ok_or_else(|| not_found("failed to determine platform directories"))?;
            (
                var("NIRVANA_CONFIG_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| dirs.config_dir().to_path_buf()),
                var("NIRVANA_DATA_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| dirs.data_dir().to_path_buf()),
                var("NIRVANA_LOG_DIR")
                    .map(PathBuf::from)
                    .map_or_else(|| Self::platform_log_dir(&dirs), Ok)?,
            )
        };

        let config_file = config_dir.join("config.toml");
        let db_file = data_dir.join("nirvana.db");
        let log_file = log_dir.join("nirvana.log");

        Ok(Self {
            config_dir,
            data_dir,
            log_dir,
            config_file,
            db_file,
            log_file,
            is_dev,
        })
    }

    /// When in debug mode or overwritten using NIRVANA_DEV env var.
    fn is_dev_mode(var: &impl Fn(&str) -> Option<String>) -> bool {
        match var("NIRVANA_DEV").as_deref() {
            Some("0") => false,
            Some("1") => true,
            _ => cfg!(debug_assertions),
        }
    }

    fn exe_dir() -> io::Result<PathBuf> {
        let exe = env::current_exe()?;
        let dir = exe
            .parent()
            .ok_or_else(|| not_found("exe has no parent directory"))?;
        Ok(dir.to_path_buf())
    }

    #[cfg(target_os = "macos")]
    fn platform_log_dir(dirs: &ProjectDirs) -> io::Result<PathBuf> {
        let base_dirs = directories::BaseDirs::new()
            .ok_or_else(|| not_found("failed to determine home directory"))?;
        Ok(base_dirs
            .home_dir()
            .join("Library/Logs")
            .join(dirs.project_path()))
    }

    #[cfg(not(target_os = "macos"))]
    fn platform_log_dir(dirs: &ProjectDirs) -> io::Result<PathBuf> {
        // Windows has no state dir and falls back to %LOCALAPPDATA%
        Ok(dirs
            .state_dir()
            .unwrap_or_else(|| dirs.data_local_dir())
            .to_path_buf())
    }
}

/// Returns `backup_dir/<file name>.<timestamp>.bak`, creating `backup_dir` if
/// it does not exist.
///
/// For example `nirvana.db` gives `backup/nirvana.db.20261019-203400.bak`.
pub(crate) fn backup_path(file: &Path, backup_dir: &Path) -> io::Result<PathBuf> {
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let mut name = file
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?
        .to_os_string();
    name.push(format!(".{stamp}.bak"));

    fs::create_dir_all(backup_dir)?;
    Ok(backup_dir.join(name))
}

fn not_found(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, message)
}

#[cfg(test)]
impl AppPaths {
    /// Places every directory and file in `dir`, in dev mode.
    pub(crate) fn in_dir(dir: &std::path::Path) -> Self {
        Self {
            config_dir: dir.to_path_buf(),
            data_dir: dir.to_path_buf(),
            log_dir: dir.to_path_buf(),
            config_file: dir.join("config.toml"),
            db_file: dir.join("nirvana.db"),
            log_file: dir.join("nirvana.log"),
            is_dev: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let vars: Vec<(String, String)> = vars
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        move |key| {
            vars.iter()
                .find(|(k, _)| k == key)
                .map(|(_, value)| value.clone())
        }
    }

    fn platform_dirs() -> ProjectDirs {
        ProjectDirs::from("", "", "nirvana").unwrap()
    }

    #[test]
    fn dev_mode_follows_nirvana_dev() {
        assert!(AppPaths::is_dev_mode(&env(&[("NIRVANA_DEV", "1")])));
        assert!(!AppPaths::is_dev_mode(&env(&[("NIRVANA_DEV", "0")])));
        assert_eq!(AppPaths::is_dev_mode(&env(&[])), cfg!(debug_assertions));
        assert_eq!(
            AppPaths::is_dev_mode(&env(&[("NIRVANA_DEV", "")])),
            cfg!(debug_assertions)
        );
        assert_eq!(
            AppPaths::is_dev_mode(&env(&[("NIRVANA_DEV", "yes")])),
            cfg!(debug_assertions)
        );
    }

    #[test]
    fn dev_mode_places_files_next_to_executable() {
        let paths = AppPaths::resolve_with(env(&[("NIRVANA_DEV", "1")])).unwrap();
        let exe_dir = AppPaths::exe_dir().unwrap();

        assert!(paths.is_dev);
        assert_eq!(paths.config_dir, exe_dir);
        assert_eq!(paths.data_dir, exe_dir);
        assert_eq!(paths.log_dir, exe_dir);
        assert_eq!(paths.config_file, exe_dir.join("config.toml"));
        assert_eq!(paths.db_file, exe_dir.join("nirvana.db"));
        assert_eq!(paths.log_file, exe_dir.join("nirvana.log"));
    }

    #[test]
    fn dev_mode_ignores_dir_overrides() {
        let paths = AppPaths::resolve_with(env(&[
            ("NIRVANA_DEV", "1"),
            ("NIRVANA_CONFIG_DIR", "/override/config"),
            ("NIRVANA_DATA_DIR", "/override/data"),
            ("NIRVANA_LOG_DIR", "/override/log"),
        ]))
        .unwrap();
        let exe_dir = AppPaths::exe_dir().unwrap();

        assert_eq!(paths.config_dir, exe_dir);
        assert_eq!(paths.data_dir, exe_dir);
        assert_eq!(paths.log_dir, exe_dir);
    }

    #[test]
    fn release_mode_uses_platform_dirs() {
        let paths = AppPaths::resolve_with(env(&[("NIRVANA_DEV", "0")])).unwrap();
        let dirs = platform_dirs();

        assert!(!paths.is_dev);
        assert_eq!(paths.config_dir, dirs.config_dir());
        assert_eq!(paths.data_dir, dirs.data_dir());
        assert_eq!(paths.config_file, dirs.config_dir().join("config.toml"));
        assert_eq!(paths.db_file, dirs.data_dir().join("nirvana.db"));
        assert_eq!(paths.log_file, paths.log_dir.join("nirvana.log"));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn release_mode_logs_to_library_logs_on_macos() {
        let paths = AppPaths::resolve_with(env(&[("NIRVANA_DEV", "0")])).unwrap();
        let home = PathBuf::from(env::var("HOME").unwrap());

        assert_eq!(paths.log_dir, home.join("Library/Logs/nirvana"));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn release_mode_logs_to_state_dir_on_linux() {
        let paths = AppPaths::resolve_with(env(&[("NIRVANA_DEV", "0")])).unwrap();
        let state_home = env::var("XDG_STATE_HOME")
            .ok()
            .filter(|dir| dir.starts_with('/'))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env::var("HOME").unwrap()).join(".local/state"));

        assert_eq!(paths.log_dir, state_home.join("nirvana"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn release_mode_logs_to_local_app_data_on_windows() {
        let paths = AppPaths::resolve_with(env(&[("NIRVANA_DEV", "0")])).unwrap();
        let local_app_data = PathBuf::from(env::var("LOCALAPPDATA").unwrap());

        assert_eq!(paths.log_dir, local_app_data.join("nirvana").join("data"));
    }

    #[test]
    fn release_mode_applies_dir_overrides() {
        let paths = AppPaths::resolve_with(env(&[
            ("NIRVANA_DEV", "0"),
            ("NIRVANA_CONFIG_DIR", "/override/config"),
            ("NIRVANA_DATA_DIR", "/override/data"),
            ("NIRVANA_LOG_DIR", "/override/log"),
        ]))
        .unwrap();

        assert_eq!(paths.config_dir, PathBuf::from("/override/config"));
        assert_eq!(paths.data_dir, PathBuf::from("/override/data"));
        assert_eq!(paths.log_dir, PathBuf::from("/override/log"));
        assert_eq!(
            paths.config_file,
            PathBuf::from("/override/config/config.toml")
        );
        assert_eq!(paths.db_file, PathBuf::from("/override/data/nirvana.db"));
        assert_eq!(paths.log_file, PathBuf::from("/override/log/nirvana.log"));
    }

    #[test]
    fn release_mode_ignores_empty_dir_overrides() {
        let paths = AppPaths::resolve_with(env(&[
            ("NIRVANA_DEV", "0"),
            ("NIRVANA_CONFIG_DIR", ""),
            ("NIRVANA_DATA_DIR", ""),
            ("NIRVANA_LOG_DIR", ""),
        ]))
        .unwrap();
        let defaults = AppPaths::resolve_with(env(&[("NIRVANA_DEV", "0")])).unwrap();

        assert_eq!(paths.config_dir, defaults.config_dir);
        assert_eq!(paths.data_dir, defaults.data_dir);
        assert_eq!(paths.log_dir, defaults.log_dir);
    }
}
