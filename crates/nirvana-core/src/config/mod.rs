//! Machine config (`config.toml`).
//!
//! The file is read according to its `schema_version`:
//!
//! - missing: 0.1 flat format, replaced with the defaults
//! - `1`: 0.3 sectioned format, parsed by `legacy_030`
//! - `2`: the current format
//! - anything else: [`ConfigError::UnsupportedSchema`]
//!
//! Each legacy module only parses TOML and returns the current [`AppConfig`].

use std::fs;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::debug;

use crate::paths::AppPaths;

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
    fn parse(table: toml::Table) -> Result<Self, ConfigError> {
        match table.get("schema_version") {
            None => {
                debug!("replacing config without schema_version with defaults");
                Ok(Self::default())
            }
            Some(toml::Value::Integer(1)) => legacy_030::parse(table),
            Some(toml::Value::Integer(CURRENT_SCHEMA_VERSION)) => Ok(table.try_into()?),
            Some(version) => Err(ConfigError::UnsupportedSchema(version.clone())),
        }
    }

    /// Reads `config.toml` and returns it in the current format.
    ///
    /// A missing file or a file without `schema_version` gives the defaults.
    /// Older formats are migrated in memory and written on the next save.
    pub(crate) fn load(paths: &AppPaths) -> Result<Self, ConfigError> {
        if !paths.config_file.exists() {
            return Ok(Self::default());
        }
        Self::parse(fs::read_to_string(&paths.config_file)?.parse()?)
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
        fs::write(&paths.config_file, toml::to_string_pretty(&table)?)?;
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

    #[test]
    fn parses_current_format() {
        let config = AppConfig::parse(
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
"#
            .parse()
            .unwrap(),
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

        assert_eq!(AppConfig::parse(content.parse().unwrap()).unwrap(), config);
    }

    #[test]
    fn replaces_010_fixture_with_defaults() {
        let config = AppConfig::parse(
            include_str!("../../tests/fixtures/v0_3_0/config_010.toml")
                .parse()
                .unwrap(),
        )
        .unwrap();

        assert_eq!(config, AppConfig::default());
    }

    #[test]
    fn unsupported_schema_version_fails() {
        let result = AppConfig::parse("schema_version = 3".parse().unwrap());

        assert!(matches!(result, Err(ConfigError::UnsupportedSchema(_))));
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

    #[test]
    fn saves_030_fixture_without_dropped_keys() {
        let fixture = include_str!("../../tests/fixtures/v0_3_0/config_030.toml");
        let config = AppConfig::parse(fixture.parse().unwrap()).unwrap();

        let saved = save_and_read(&config, Some(fixture));

        assert_eq!(
            saved,
            r#"
schema_version = 2

[gui]
theme = "dark"

[idle]
enabled = true
methods = ["lock"]
threshold_secs = 600
"#
            .parse()
            .unwrap()
        );
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
