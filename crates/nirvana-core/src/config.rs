use crate::paths::AppPaths;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use thiserror::Error;

const CURRENT_SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("config parse error: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("config save error: {0}")]
    Save(#[from] toml::ser::Error),
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct CoreConfig {
    #[serde(default)]
    pub active_connection: Option<i64>,
    #[serde(default = "default_publish_squashed_worklogs")]
    pub publish_squashed_worklogs: bool,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct GuiConfig {
    #[serde(default = "default_font_scale")]
    pub font_scale: f64,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_show_tray_icon")]
    pub show_tray_icon: bool,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct IdleConfig {
    #[serde(default = "default_idle_enabled")]
    pub enabled: bool,
    #[serde(default = "default_idle_methods")]
    pub methods: Vec<String>,
    #[serde(default = "default_idle_threshold_secs")]
    pub threshold_secs: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(default)]
    pub schema_version: Option<i64>,
    #[serde(default)]
    pub(crate) core: CoreConfig,
    #[serde(default)]
    pub gui: GuiConfig,
    #[serde(default)]
    pub idle: IdleConfig,
}

#[derive(Debug, Deserialize)]
struct LegacyAppConfig {
    #[serde(default)]
    pub active_connection: Option<i64>,
    #[serde(default = "default_publish_squashed_worklogs")]
    pub publish_squashed_worklogs: bool,
    #[serde(default = "default_font_scale")]
    pub font_scale: f64,
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl From<LegacyAppConfig> for AppConfig {
    fn from(legacy: LegacyAppConfig) -> Self {
        Self {
            schema_version: Some(CURRENT_SCHEMA_VERSION),
            core: CoreConfig {
                active_connection: legacy.active_connection,
                publish_squashed_worklogs: legacy.publish_squashed_worklogs,
            },
            gui: GuiConfig {
                font_scale: legacy.font_scale,
                theme: legacy.theme,
                show_tray_icon: default_show_tray_icon(),
            },
            idle: IdleConfig {
                enabled: default_idle_enabled(),
                methods: default_idle_methods(),
                threshold_secs: default_idle_threshold_secs(),
            },
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: Some(CURRENT_SCHEMA_VERSION),
            core: CoreConfig {
                active_connection: None,
                publish_squashed_worklogs: default_publish_squashed_worklogs(),
            },
            gui: GuiConfig {
                font_scale: default_font_scale(),
                theme: default_theme(),
                show_tray_icon: default_show_tray_icon(),
            },
            idle: IdleConfig {
                enabled: default_idle_enabled(),
                methods: default_idle_methods(),
                threshold_secs: default_idle_threshold_secs(),
            },
        }
    }
}

fn default_publish_squashed_worklogs() -> bool {
    true
}

fn default_font_scale() -> f64 {
    1.0
}

pub(crate) fn normalize_font_scale(font_scale: f64) -> f64 {
    if font_scale.is_finite() {
        font_scale.clamp(0.9, 1.25)
    } else {
        default_font_scale()
    }
}

pub(crate) fn default_theme() -> String {
    "high-contrast-dark".to_string()
}

pub(crate) fn normalize_theme(theme: &str) -> String {
    match theme {
        "nirvana-dark" | "high-contrast-dark" | "soft-light" => theme.to_string(),
        _ => default_theme(),
    }
}

fn default_show_tray_icon() -> bool {
    false
}

fn default_idle_methods() -> Vec<String> {
    vec!["lock".into(), "sleep".into(), "input".into()]
}

fn default_idle_enabled() -> bool {
    true
}

fn default_idle_threshold_secs() -> u64 {
    300
}

pub(crate) fn normalize_idle_methods(methods: &[String]) -> Vec<String> {
    let valid = ["lock", "sleep", "input"];
    let normalized: Vec<String> = methods
        .iter()
        .filter(|m| valid.contains(&m.as_str()))
        .cloned()
        .collect();
    if normalized.is_empty() {
        default_idle_methods()
    } else {
        normalized
    }
}

pub(crate) fn normalize_idle_threshold_secs(secs: u64) -> u64 {
    if secs >= 10 { secs } else { default_idle_threshold_secs() }
}

impl AppConfig {
    pub(crate) fn parse(content: &str) -> Result<Self, ConfigError> {
        // Try the versioned sectioned format first
        if let Ok(config) = toml::from_str::<Self>(content)
            && config.schema_version.is_some()
        {
            return Ok(config);
        }

        // Fall back to legacy flat format
        let legacy: LegacyAppConfig = toml::from_str(content)?;
        Ok(legacy.into())
    }

    pub(crate) fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)?;
        Self::parse(&content)
    }

    pub(crate) fn save(&self, paths: &AppPaths) -> Result<(), ConfigError> {
        let mut config = self.clone();
        config.schema_version = Some(CURRENT_SCHEMA_VERSION);
        let content = toml::to_string_pretty(&config)?;
        std::fs::create_dir_all(&paths.config_dir)?;
        fs::write(&paths.config_file, content)?;
        Ok(())
    }

    pub fn load_from_default_path() -> Self {
        let paths = AppPaths::resolve();
        Self::load(&paths.config_file).unwrap_or_default()
    }
}

impl Clone for AppConfig {
    fn clone(&self) -> Self {
        Self {
            schema_version: self.schema_version,
            core: CoreConfig {
                active_connection: self.core.active_connection,
                publish_squashed_worklogs: self.core.publish_squashed_worklogs,
            },
            gui: GuiConfig {
                font_scale: self.gui.font_scale,
                theme: self.gui.theme.clone(),
                show_tray_icon: self.gui.show_tray_icon,
            },
            idle: IdleConfig {
                enabled: self.idle.enabled,
                methods: self.idle.methods.clone(),
                threshold_secs: self.idle.threshold_secs,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_publish_squashed_worklogs_to_enabled() {
        let config = AppConfig::parse("[core]\nactive_connection = 1\n").unwrap();

        assert!(config.core.publish_squashed_worklogs);
    }

    #[test]
    fn defaults_show_tray_icon_to_false() {
        let config = AppConfig::parse("").unwrap();

        assert!(!config.gui.show_tray_icon);
    }

    #[test]
    fn parses_sectioned_format() {
        let config = AppConfig::parse(
            r#"
schema_version = 1

[core]
active_connection = 42
publish_squashed_worklogs = false

[gui]
font_scale = 1.1
theme = "soft-light"
show_tray_icon = true
"#,
        )
        .unwrap();

        assert_eq!(config.schema_version, Some(1));
        assert_eq!(config.core.active_connection, Some(42));
        assert!(!config.core.publish_squashed_worklogs);
        assert!((config.gui.font_scale - 1.1).abs() < f64::EPSILON);
        assert_eq!(config.gui.theme, "soft-light");
        assert!(config.gui.show_tray_icon);
    }

    #[test]
    fn migrates_legacy_flat_format() {
        let config = AppConfig::parse(
            r#"
active_connection = 1
publish_squashed_worklogs = false
font_scale = 1.2
theme = "nirvana-dark"
"#,
        )
        .unwrap();

        assert_eq!(config.schema_version, Some(CURRENT_SCHEMA_VERSION));
        assert_eq!(config.core.active_connection, Some(1));
        assert!(!config.core.publish_squashed_worklogs);
        assert!((config.gui.font_scale - 1.2).abs() < f64::EPSILON);
        assert_eq!(config.gui.theme, "nirvana-dark");
        assert!(!config.gui.show_tray_icon);
        assert!(config.idle.enabled);
    }

    #[test]
    fn defaults_idle_enabled_with_all_methods() {
        let config = AppConfig::default();

        assert!(config.idle.enabled);
        assert_eq!(config.idle.methods, vec!["lock", "sleep", "input"]);
        assert_eq!(config.idle.threshold_secs, 300);
    }

    #[test]
    fn parses_idle_section() {
        let config = AppConfig::parse(
            r#"
schema_version = 1

[idle]
enabled = true
methods = ["lock", "input"]
threshold_secs = 600
"#,
        )
        .unwrap();

        assert!(config.idle.enabled);
        assert_eq!(config.idle.methods, vec!["lock", "input"]);
        assert_eq!(config.idle.threshold_secs, 600);
    }

    #[test]
    fn normalizes_empty_methods_to_defaults() {
        let methods = normalize_idle_methods(&[]);
        assert_eq!(methods, vec!["lock", "sleep", "input"]);
    }

    #[test]
    fn normalizes_invalid_methods() {
        let methods = normalize_idle_methods(&["foo".into(), "lock".into()]);
        assert_eq!(methods, vec!["lock"]);
    }

    #[test]
    fn normalizes_threshold_below_minimum() {
        assert_eq!(normalize_idle_threshold_secs(5), 300);
        assert_eq!(normalize_idle_threshold_secs(10), 10);
    }
}
