//! Reading and changing `config.toml` by key.
//!
//! Keys are `section.key`, see [`KEYS`]. `core.profile` is changed by
//! switching the active profile and cannot be unset.

use crate::config::{AppConfig, ConfigError, KEYS};
use crate::domain::ConfigEntry;
use crate::errors::{DomainError, NirvanaError};
use crate::logging;
use crate::service::Nirvana;

const PROFILE_KEY: &str = "core.profile";
const LOG_LEVEL_KEY: &str = "log.level";

impl Nirvana {
    /// Returns the config entries selected by `key`: every key for `None`,
    /// the keys of a section for a section name, or one key for
    /// `section.key`. A key is set if [`AppConfig::save`] writes it.
    ///
    /// # Errors
    ///
    /// Returns an error if the key is unknown or the config is unavailable.
    pub fn get_config(&self, key: Option<&str>) -> Result<Vec<ConfigEntry>, NirvanaError> {
        let keys: Vec<&'static str> = KEYS
            .into_iter()
            .filter(|candidate| {
                key.is_none_or(|key| {
                    *candidate == key
                        || candidate
                            .split_once('.')
                            .is_some_and(|(section, _)| section == key)
                })
            })
            .collect();
        if keys.is_empty() {
            return Err(DomainError::UnknownConfigKey(key.unwrap_or_default().to_owned()).into());
        }

        let values = self.config()?.saved_table(&self.paths)?;
        let defaults = toml::Table::try_from(AppConfig::default()).map_err(ConfigError::from)?;
        Ok(keys
            .into_iter()
            .map(|key| ConfigEntry {
                key,
                value: lookup(&values, key).map(display),
                default: lookup(&defaults, key).map(display).or_else(|| {
                    (key == LOG_LEVEL_KEY).then(|| {
                        logging::default_level(self.paths.is_dev)
                            .as_str()
                            .to_owned()
                    })
                }),
            })
            .collect())
    }

    /// Sets the config key `key` (`section.key`) to `value` and saves the
    /// config. `value` is read as a TOML value, or as a string if it is not
    /// one.
    ///
    /// # Errors
    ///
    /// Returns an error if the key is unknown, the value is invalid for the
    /// key, the profile for `core.profile` does not exist, the config is
    /// unavailable or cannot be saved, or the database query fails.
    pub async fn set_config(&mut self, key: &str, value: &str) -> Result<(), NirvanaError> {
        let (section, name) = split_key(key)?;
        if key == PROFILE_KEY {
            self.switch_profile(value).await?;
            return Ok(());
        }

        let mut table = toml::Table::try_from(self.config()?).map_err(ConfigError::from)?;
        table
            .entry(section)
            .or_insert_with(|| toml::Table::new().into())
            .as_table_mut()
            .ok_or_else(|| DomainError::UnknownConfigKey(key.to_owned()))?
            .insert(name.to_owned(), parse_value(value));
        let config: AppConfig = table
            .try_into()
            .map_err(|e| DomainError::InvalidConfigValue(key.to_owned(), e))?;

        *self.config_mut()? = config;
        self.config()?.save(&self.paths)?;
        Ok(())
    }

    /// Removes the config key `key` (`section.key`) from `config.toml`,
    /// restoring its default.
    ///
    /// # Errors
    ///
    /// Returns an error if the key is unknown or is `core.profile`, or the
    /// config is unavailable or cannot be saved.
    pub fn unset_config(&mut self, key: &str) -> Result<(), NirvanaError> {
        let (section, name) = split_key(key)?;
        if key == PROFILE_KEY {
            return Err(DomainError::ConfigKeyRequired(key.to_owned()).into());
        }

        let mut table = toml::Table::try_from(self.config()?).map_err(ConfigError::from)?;
        if let Some(section) = table.get_mut(section).and_then(toml::Value::as_table_mut) {
            section.remove(name);
        }
        let config: AppConfig = table
            .try_into()
            .map_err(|e| DomainError::InvalidConfigValue(key.to_owned(), e))?;

        *self.config_mut()? = config;
        self.config()?.save_without(&self.paths, key)?;
        Ok(())
    }
}

fn split_key(key: &str) -> Result<(&str, &str), DomainError> {
    key.split_once('.')
        .filter(|_| KEYS.contains(&key))
        .ok_or_else(|| DomainError::UnknownConfigKey(key.to_owned()))
}

fn lookup<'a>(table: &'a toml::Table, key: &str) -> Option<&'a toml::Value> {
    let (section, name) = key.split_once('.')?;
    table.get(section)?.get(name)
}

fn display(value: &toml::Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned)
}

fn parse_value(value: &str) -> toml::Value {
    format!("value = {value}")
        .parse::<toml::Table>()
        .ok()
        .and_then(|mut table| table.remove("value"))
        .unwrap_or_else(|| value.to_owned().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LogLevel, Theme};
    use crate::service::test_support::test_service;

    fn entry(key: &'static str, value: Option<&str>, default: Option<&str>) -> ConfigEntry {
        ConfigEntry {
            key,
            value: value.map(str::to_owned),
            default: default.map(str::to_owned),
        }
    }

    #[tokio::test]
    async fn get_config_returns_all_keys_with_values_and_defaults() {
        let service = test_service().await;

        assert_eq!(
            service.get_config(None).unwrap(),
            [
                entry("core.profile", Some("test"), None),
                entry("gui.theme", None, Some("system")),
                entry("idle.enabled", Some("false"), Some("true")),
                entry("idle.methods", None, Some(r#"["lock", "sleep", "input"]"#)),
                entry("idle.threshold_secs", None, Some("300")),
                entry("log.level", None, Some("trace")),
            ]
        );
    }

    #[tokio::test]
    async fn get_config_selects_section_or_key() {
        let service = test_service().await;

        let keys = |key| -> Vec<&str> {
            service
                .get_config(Some(key))
                .unwrap()
                .into_iter()
                .map(|entry| entry.key)
                .collect()
        };

        assert_eq!(
            keys("idle"),
            ["idle.enabled", "idle.methods", "idle.threshold_secs"]
        );
        assert_eq!(keys("idle.methods"), ["idle.methods"]);
    }

    #[tokio::test]
    async fn get_config_rejects_unknown_key() {
        let service = test_service().await;

        for key in ["foo", "idle.foo", "schema_version", ""] {
            let err = service.get_config(Some(key)).unwrap_err();
            assert!(
                matches!(&err, NirvanaError::Domain(DomainError::UnknownConfigKey(unknown)) if unknown == key),
                "{key}: {err}"
            );
        }
    }

    #[tokio::test]
    async fn get_config_shows_default_value_present_in_file() {
        let service = test_service().await;
        std::fs::write(
            &service.paths.config_file,
            "schema_version = 2\n\n[gui]\ntheme = \"system\"",
        )
        .unwrap();

        assert_eq!(
            service.get_config(Some("gui.theme")).unwrap(),
            [entry("gui.theme", Some("system"), Some("system"))]
        );
    }

    #[tokio::test]
    async fn set_config_parses_value_for_key_and_saves() {
        let mut service = test_service().await;

        service.set_config("gui.theme", "dark").await.unwrap();
        service
            .set_config("idle.threshold_secs", "600")
            .await
            .unwrap();
        service
            .set_config("idle.methods", r#"["lock"]"#)
            .await
            .unwrap();
        service.set_config("log.level", "debug").await.unwrap();

        let saved = AppConfig::load(&service.paths).unwrap();
        assert_eq!(saved, *service.config().unwrap());
        assert_eq!(saved.gui.theme, Theme::Dark);
        assert_eq!(saved.idle.threshold_secs, 600);
        assert_eq!(saved.idle.methods, ["lock"]);
        assert_eq!(saved.log.level, Some(LogLevel::Debug));
    }

    #[tokio::test]
    async fn set_config_rejects_invalid_value() {
        let mut service = test_service().await;

        for (key, value) in [
            ("gui.theme", "blue"),
            ("idle.enabled", "yes"),
            ("idle.threshold_secs", "-1"),
        ] {
            let err = service.set_config(key, value).await.unwrap_err();
            assert!(
                matches!(&err, NirvanaError::Domain(DomainError::InvalidConfigValue(invalid, _)) if invalid == key),
                "{key}: {err}"
            );
        }
        assert!(!service.paths.config_file.exists());
    }

    #[tokio::test]
    async fn set_config_rejects_unknown_key() {
        let mut service = test_service().await;

        for key in ["idle", "idle.foo", "schema_version"] {
            let err = service.set_config(key, "1").await.unwrap_err();
            assert!(
                matches!(&err, NirvanaError::Domain(DomainError::UnknownConfigKey(unknown)) if unknown == key),
                "{key}: {err}"
            );
        }
    }

    #[tokio::test]
    async fn unset_config_removes_key_and_restores_default() {
        let mut service = test_service().await;
        service.set_config("gui.theme", "dark").await.unwrap();
        service.set_config("log.level", "debug").await.unwrap();

        service.unset_config("gui.theme").unwrap();
        service.unset_config("log.level").unwrap();

        let saved = AppConfig::load(&service.paths).unwrap();
        assert_eq!(saved, *service.config().unwrap());
        assert_eq!(saved.gui.theme, Theme::default());
        assert_eq!(saved.log.level, None);
        let content = std::fs::read_to_string(&service.paths.config_file).unwrap();
        assert!(!content.contains("[gui]"), "{content}");
        assert!(!content.contains("[log]"), "{content}");
    }

    #[tokio::test]
    async fn unset_config_removes_default_value_present_in_file() {
        let mut service = test_service().await;
        std::fs::write(
            &service.paths.config_file,
            "schema_version = 2\n\n[gui]\ntheme = \"system\"",
        )
        .unwrap();

        service.unset_config("gui.theme").unwrap();

        assert_eq!(
            service.get_config(Some("gui.theme")).unwrap(),
            [entry("gui.theme", None, Some("system"))]
        );
    }

    #[tokio::test]
    async fn unset_config_rejects_unknown_and_profile_key() {
        let mut service = test_service().await;

        for key in ["idle", "idle.foo", "schema_version"] {
            let err = service.unset_config(key).unwrap_err();
            assert!(
                matches!(&err, NirvanaError::Domain(DomainError::UnknownConfigKey(unknown)) if unknown == key),
                "{key}: {err}"
            );
        }
        let err = service.unset_config(PROFILE_KEY).unwrap_err();
        assert!(
            matches!(&err, NirvanaError::Domain(DomainError::ConfigKeyRequired(key)) if key == PROFILE_KEY),
            "{err}"
        );
    }

    #[tokio::test]
    async fn set_config_profile_switches_to_existing_profile() {
        let mut service = test_service().await;
        service.create_profile("work".to_string()).await.unwrap();

        service.set_config(PROFILE_KEY, "work").await.unwrap();
        let err = service
            .set_config(PROFILE_KEY, "missing")
            .await
            .unwrap_err();

        assert_eq!(
            AppConfig::load(&service.paths)
                .unwrap()
                .core
                .profile
                .as_deref(),
            Some("work")
        );
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::ProfileNotFound(name)) if name == "missing"
        ));
    }
}
