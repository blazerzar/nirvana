use super::{AppConfig, ConfigError};

/// Returns whether `table` is in the 0.1 flat format, with keys but no
/// sections.
pub(super) fn is_flat(table: &toml::Table) -> bool {
    !table.values().any(toml::Value::is_table)
}

/// Returns the default values, all 0.1 settings are reset.
pub(super) fn parse(_table: toml::Table) -> Result<AppConfig, ConfigError> {
    Ok(AppConfig::default())
}
