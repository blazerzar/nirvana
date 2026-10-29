//! Machine config (`config.toml`).
//!
//! The file is read according to its `schema_version`:
//!
//! - missing in an empty file: the defaults
//! - missing in a flat file without sections: 0.1 format, replaced with the
//!   defaults by `legacy_010`
//! - missing in any other file: [`ConfigError::MissingSchema`]
//! - `1`: 0.3 sectioned format, parsed by `legacy_030`
//! - `2`: the current format
//! - anything else: [`ConfigError::UnsupportedSchema`]
//!
//! Each legacy module converts its TOML table to the current [`AppConfig`].
//! A legacy format without `schema_version` is detected by its module.
//! A file in an older format is backed up and rewritten on load.

use std::fs;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;

use crate::paths::{AppPaths, backup_path};

mod legacy_010;
mod legacy_030;

const CURRENT_SCHEMA_VERSION: i64 = 2;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("config parse error: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("config save error: {0}")]
    Save(#[from] toml::ser::Error),
    #[error("unsupported config schema_version: {0}")]
    UnsupportedSchema(toml::Value),
    #[error("config has sections but no schema_version")]
    MissingSchema,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Schema {
    Empty,
    Legacy010,
    Legacy030,
    Current,
}

impl Schema {
    fn of(table: &toml::Table) -> Result<Self, ConfigError> {
        match table.get("schema_version") {
            None if table.is_empty() => Ok(Self::Empty),
            None if legacy_010::is_flat(table) => Ok(Self::Legacy010),
            None => Err(ConfigError::MissingSchema),
            Some(toml::Value::Integer(1)) => Ok(Self::Legacy030),
            Some(toml::Value::Integer(CURRENT_SCHEMA_VERSION)) => Ok(Self::Current),
            Some(version) => Err(ConfigError::UnsupportedSchema(version.clone())),
        }
    }

    fn is_legacy(self) -> bool {
        matches!(self, Self::Legacy010 | Self::Legacy030)
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub(crate) struct AppConfig {
    schema_version: i64,
    #[serde(default)]
    pub(crate) core: CoreConfig,
    #[serde(default)]
    pub(crate) gui: GuiConfig,
    #[serde(default)]
    pub(crate) idle: IdleConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            core: CoreConfig::default(),
            gui: GuiConfig::default(),
            idle: IdleConfig::default(),
        }
    }
}

impl AppConfig {
    fn parse(schema: Schema, table: toml::Table) -> Result<Self, ConfigError> {
        match schema {
            Schema::Empty => Ok(Self::default()),
            Schema::Legacy010 => legacy_010::parse(table),
            Schema::Legacy030 => legacy_030::parse(table),
            Schema::Current => Ok(table.try_into()?),
        }
    }

    /// Reads `config.toml` and returns it in the current format.
    ///
    /// A missing file gives the defaults and nothing is written. A file in an
    /// older format is copied to `data_dir/backup` and rewritten in the
    /// current format with only non-default values.
    pub(crate) fn load(paths: &AppPaths) -> Result<Self, ConfigError> {
        if !paths.config_file.exists() {
            return Ok(Self::default());
        }
        let table: toml::Table = fs::read_to_string(&paths.config_file)?.parse()?;
        let schema = Schema::of(&table)?;
        let config = Self::parse(schema, table)?;
        if schema.is_legacy() {
            let backup_path = backup_path(&paths.config_file, &paths.data_dir.join("backup"))?;
            fs::copy(&paths.config_file, &backup_path)?;
            info!("config backed up to {}", backup_path.display());
            config.write(paths, &toml::Table::new())?;
            info!("config migrated from {schema:?} to schema version {CURRENT_SCHEMA_VERSION}");
        }
        Ok(config)
    }

    /// Writes the config to `config.toml` in the current format, creating the
    /// config directory if it does not exist.
    ///
    /// A key is written only if its value differs from the default or the key
    /// is already present in the file. `schema_version` is always written. A
    /// file that cannot be read is overwritten.
    pub(crate) fn save(&self, paths: &AppPaths) -> Result<(), ConfigError> {
        let existing: toml::Table = fs::read_to_string(&paths.config_file)
            .ok()
            .and_then(|content| content.parse().ok())
            .unwrap_or_default();
        self.write(paths, &existing)
    }

    /// Writes the config like [`save`](Self::save), keeping default values
    /// only for keys present in `existing`.
    fn write(&self, paths: &AppPaths, existing: &toml::Table) -> Result<(), ConfigError> {
        let defaults = toml::Table::try_from(Self::default())?;
        let mut table = toml::Table::try_from(self)?;
        table.retain(|key, value| {
            if key == "schema_version" {
                return true;
            }
            let existing = existing.get(key);
            let default = defaults.get(key);
            match value {
                toml::Value::Table(keys) => {
                    keys.retain(|key, value| {
                        existing.and_then(|s| s.get(key)).is_some()
                            || default.and_then(|s| s.get(key)) != Some(value)
                    });
                    !keys.is_empty()
                }
                value => existing.is_some() || default != Some(value),
            }
        });
        fs::create_dir_all(&paths.config_dir)?;
        let temp_file = paths.config_file.with_extension("toml.tmp");
        fs::write(&temp_file, toml::to_string_pretty(&table)?)?;
        fs::rename(&temp_file, &paths.config_file)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub(crate) struct CoreConfig {
    #[serde(default)]
    pub(crate) profile: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub(crate) struct GuiConfig {
    #[serde(default)]
    pub(crate) theme: Theme,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub(crate) struct IdleConfig {
    #[serde(default = "default_idle_enabled")]
    pub(crate) enabled: bool,
    #[serde(default = "default_idle_methods")]
    pub(crate) methods: Vec<String>,
    #[serde(default = "default_idle_threshold_secs")]
    pub(crate) threshold_secs: u64,
}

impl Default for IdleConfig {
    fn default() -> Self {
        Self {
            enabled: default_idle_enabled(),
            methods: default_idle_methods(),
            threshold_secs: default_idle_threshold_secs(),
        }
    }
}

fn default_idle_enabled() -> bool {
    true
}

fn default_idle_methods() -> Vec<String> {
    vec!["lock".into(), "sleep".into(), "input".into()]
}

fn default_idle_threshold_secs() -> u64 {
    300
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_010: &str = include_str!("../../tests/fixtures/v0_3_0/config_010.toml");
    pub(super) const FIXTURE_020: &str =
        include_str!("../../tests/fixtures/v0_3_0/config_020.toml");
    pub(super) const FIXTURE_030: &str =
        include_str!("../../tests/fixtures/v0_3_0/config_030.toml");

    /// Parses the content of a config file into the current config.
    pub(super) fn parse(content: &str) -> Result<AppConfig, ConfigError> {
        let table: toml::Table = content.parse()?;
        AppConfig::parse(Schema::of(&table)?, table)
    }

    #[test]
    fn parses_current_format() {
        let config = parse(
            r#"
schema_version = 2

[core]
profile = 3

[gui]
theme = "dark"

[idle]
enabled = false
methods = ["lock", "input"]
threshold_secs = 600
"#,
        )
        .unwrap();

        assert_eq!(
            config,
            AppConfig {
                schema_version: 2,
                core: CoreConfig { profile: Some(3) },
                gui: GuiConfig { theme: Theme::Dark },
                idle: IdleConfig {
                    enabled: false,
                    methods: vec!["lock".into(), "input".into()],
                    threshold_secs: 600,
                },
            }
        );
    }

    #[test]
    fn saved_config_parses_back() {
        let config = AppConfig {
            core: CoreConfig { profile: Some(1) },
            ..AppConfig::default()
        };

        let content = toml::to_string_pretty(&config).unwrap();

        assert_eq!(parse(&content).unwrap(), config);
    }

    #[test]
    fn sections_without_schema_version_fail() {
        let result = parse("[gui]\ntheme = \"dark\"");

        assert!(matches!(result, Err(ConfigError::MissingSchema)));
    }

    #[test]
    fn defaults_idle_enabled_with_all_methods() {
        let config = AppConfig::default();

        assert!(config.idle.enabled);
        assert_eq!(config.idle.methods, vec!["lock", "sleep", "input"]);
        assert_eq!(config.idle.threshold_secs, 300);
    }

    /// Saves `config` in a temporary directory and returns the saved
    /// `config.toml`.
    ///
    /// `file_before_save` is the content of `config.toml` before saving, `None`
    /// means the file does not exist yet.
    fn save_and_read(config: &AppConfig, file_before_save: Option<&str>) -> toml::Table {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::in_dir(dir.path());
        if let Some(content) = file_before_save {
            fs::write(&paths.config_file, content).unwrap();
        }
        config.save(&paths).unwrap();
        fs::read_to_string(&paths.config_file)
            .unwrap()
            .parse()
            .unwrap()
    }

    #[test]
    fn saves_only_schema_version_for_defaults() {
        let saved = save_and_read(&AppConfig::default(), None);

        assert_eq!(saved, "schema_version = 2".parse().unwrap());
    }

    #[test]
    fn saves_non_default_values() {
        let config = AppConfig {
            core: CoreConfig { profile: Some(1) },
            idle: IdleConfig {
                enabled: default_idle_enabled(),
                methods: default_idle_methods(),
                threshold_secs: 450,
            },
            ..AppConfig::default()
        };

        let saved = save_and_read(&config, None);

        assert_eq!(
            saved,
            r#"
schema_version = 2

[core]
profile = 1

[idle]
threshold_secs = 450
"#
            .parse()
            .unwrap()
        );
    }

    #[test]
    fn keeps_default_values_already_in_file() {
        let saved = save_and_read(
            &AppConfig::default(),
            Some(
                r#"
schema_version = 2

[idle]
enabled = true
"#,
            ),
        );

        assert_eq!(
            saved,
            r#"
schema_version = 2

[idle]
enabled = true
"#
            .parse()
            .unwrap()
        );
    }

    #[test]
    fn writes_default_values_when_key_already_in_file() {
        let saved = save_and_read(
            &AppConfig::default(),
            Some(
                r#"
schema_version = 2

[idle]
enabled = false
"#,
            ),
        );

        assert_eq!(
            saved,
            r#"
schema_version = 2

[idle]
enabled = true
"#
            .parse()
            .unwrap()
        );
    }

    /// Returns the files in `data_dir/backup`, empty if the directory does not
    /// exist.
    fn backups(paths: &AppPaths) -> Vec<std::path::PathBuf> {
        fs::read_dir(paths.data_dir.join("backup"))
            .map(|entries| entries.map(|entry| entry.unwrap().path()).collect())
            .unwrap_or_default()
    }

    /// Writes `content` to `config.toml` in `dir` and loads it.
    fn load_file(
        dir: &std::path::Path,
        content: &str,
    ) -> (AppPaths, Result<AppConfig, ConfigError>) {
        let paths = AppPaths::in_dir(dir);
        fs::write(&paths.config_file, content).unwrap();
        let result = AppConfig::load(&paths);
        (paths, result)
    }

    #[test]
    fn loading_030_fixture_backs_it_up() {
        let dir = tempfile::tempdir().unwrap();
        let (paths, result) = load_file(dir.path(), FIXTURE_030);
        result.unwrap();

        let backups = backups(&paths);
        assert_eq!(backups.len(), 1);
        let name = backups[0].file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("config.toml.") && name.ends_with(".bak"));
        assert_eq!(fs::read(&backups[0]).unwrap(), FIXTURE_030.as_bytes());
    }

    #[test]
    fn loading_030_fixture_rewrites_only_non_default_values() {
        let dir = tempfile::tempdir().unwrap();
        let (paths, result) = load_file(dir.path(), FIXTURE_030);
        result.unwrap();

        let saved: toml::Table = fs::read_to_string(&paths.config_file)
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            saved,
            r#"
schema_version = 2

[gui]
theme = "dark"

[idle]
methods = ["lock"]
threshold_secs = 600
"#
            .parse()
            .unwrap()
        );
    }

    #[test]
    fn reloading_migrated_config_skips_backup_and_rewrite() {
        let dir = tempfile::tempdir().unwrap();
        let (paths, result) = load_file(dir.path(), FIXTURE_030);
        let migrated = result.unwrap();
        let content = fs::read_to_string(&paths.config_file).unwrap();

        assert_eq!(backups(&paths).len(), 1);
        assert_eq!(AppConfig::load(&paths).unwrap(), migrated);
        assert_eq!(backups(&paths).len(), 1);
        assert_eq!(fs::read_to_string(&paths.config_file).unwrap(), content);
    }

    #[test]
    fn loading_010_fixture_backs_it_up_and_writes_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let (paths, result) = load_file(dir.path(), FIXTURE_010);

        assert_eq!(result.unwrap(), AppConfig::default());
        assert_eq!(backups(&paths).len(), 1);
        assert_eq!(
            fs::read_to_string(&paths.config_file)
                .unwrap()
                .parse::<toml::Table>()
                .unwrap(),
            "schema_version = 2".parse().unwrap()
        );
    }

    #[test]
    fn parse_error_leaves_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let (paths, result) = load_file(dir.path(), "not = [valid");

        assert!(matches!(result, Err(ConfigError::Parse(_))));
        assert_eq!(
            fs::read_to_string(&paths.config_file).unwrap(),
            "not = [valid"
        );
        assert!(backups(&paths).is_empty());
    }

    #[test]
    fn unsupported_schema_version_fails_and_leaves_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let (paths, result) = load_file(dir.path(), "schema_version = 3");

        assert!(matches!(result, Err(ConfigError::UnsupportedSchema(_))));
        assert_eq!(
            fs::read_to_string(&paths.config_file).unwrap(),
            "schema_version = 3"
        );
        assert!(backups(&paths).is_empty());
    }

    #[test]
    fn comment_only_file_gives_defaults_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (paths, result) = load_file(dir.path(), "# theme = \"dark\"\n");

        assert_eq!(result.unwrap(), AppConfig::default());
        assert_eq!(
            fs::read_to_string(&paths.config_file).unwrap(),
            "# theme = \"dark\"\n"
        );
        assert!(backups(&paths).is_empty());
    }

    #[test]
    fn missing_file_gives_defaults_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::in_dir(dir.path());

        assert_eq!(AppConfig::load(&paths).unwrap(), AppConfig::default());
        assert!(!paths.config_file.exists());
        assert!(!paths.data_dir.join("backup").exists());
    }

    #[test]
    fn overwrites_unreadable_file() {
        let config = AppConfig {
            core: CoreConfig { profile: Some(1) },
            ..AppConfig::default()
        };

        let saved = save_and_read(&config, Some("not = [valid"));

        assert_eq!(
            saved,
            r#"
schema_version = 2

[core]
profile = 1
"#
            .parse()
            .unwrap()
        );
    }
}
