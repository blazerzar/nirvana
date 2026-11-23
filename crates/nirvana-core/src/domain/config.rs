use crate::config::{IdleConfig, Theme};

/// A config key with its value in `config.toml` and its default.
#[derive(Debug, PartialEq)]
pub struct ConfigEntry {
    /// `section.key`
    pub key: &'static str,
    /// `None` if the key is not set.
    pub value: Option<String>,
    /// `None` if the key has no default.
    pub default: Option<String>,
}

/// The config values shown in the desktop settings.
#[derive(Clone, Debug, PartialEq)]
pub struct DesktopSettings {
    /// `gui.theme`
    pub theme: Theme,
    /// `gui.server`
    pub server: bool,
    /// `core.port`
    pub port: u16,
    /// The `idle` section.
    pub idle: IdleConfig,
}
