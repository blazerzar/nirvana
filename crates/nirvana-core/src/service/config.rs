//! Reading and changing `config.toml` by key.
//!
//! Keys are `section.key`, see [`KEYS`]. `core.profile` is changed by
//! switching the active profile and cannot be unset.

use crate::config::{self, AppConfig, ConfigError, IdleConfig, KEYS, Theme};
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
    pub async fn get_config(&self, key: Option<&str>) -> Result<Vec<ConfigEntry>, NirvanaError> {
        let keys: Vec<&'static str> = KEYS
            .iter()
            .copied()
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

        let defaults = config::defaults_table()?;
        let values = self
            .config()
            .await?
            .saved_table(&self.paths, &defaults)
            .await?;
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
    pub async fn set_config(&self, key: &str, value: &str) -> Result<(), NirvanaError> {
        if key == PROFILE_KEY {
            self.switch_profile(value).await?;
            return Ok(());
        }

        let mut config = self.config_mut().await?;
        let edited = edited_config(&config, key, |section, name| {
            section.insert(name.to_owned(), parse_value(value));
        })?;
        edited.save(&self.paths).await?;
        *config = edited;
        Ok(())
    }

    /// Removes the config key `key` (`section.key`) from `config.toml`,
    /// restoring its default.
    ///
    /// # Errors
    ///
    /// Returns an error if the key is unknown or is `core.profile`, or the
    /// config is unavailable or cannot be saved.
    pub async fn unset_config(&self, key: &str) -> Result<(), NirvanaError> {
        if key == PROFILE_KEY {
            return Err(DomainError::ConfigKeyRequired(key.to_owned()).into());
        }

        let mut config = self.config_mut().await?;
        let edited = edited_config(&config, key, |section, name| {
            section.remove(name);
        })?;
        edited.save_without(&self.paths, key).await?;
        *config = edited;
        Ok(())
    }

    /// Returns `gui.theme`.
    ///
    /// # Errors
    ///
    /// Returns an error if the config is unavailable.
    pub async fn theme(&self) -> Result<Theme, NirvanaError> {
        Ok(self.config().await?.gui.theme)
    }

    /// Returns `core.port`.
    ///
    /// # Errors
    ///
    /// Returns an error if the config is unavailable.
    pub async fn port(&self) -> Result<u16, NirvanaError> {
        Ok(self.config().await?.core.port)
    }

    /// Returns `gui.server`.
    ///
    /// # Errors
    ///
    /// Returns an error if the config is unavailable.
    pub async fn server(&self) -> Result<bool, NirvanaError> {
        Ok(self.config().await?.gui.server)
    }

    /// Sets `gui.server` and saves the config.
    ///
    /// # Errors
    ///
    /// Returns an error if the config is unavailable or cannot be saved.
    pub async fn set_server(&self, server: bool) -> Result<(), NirvanaError> {
        let mut config = self.config_mut().await?;
        let mut edited = config.clone();
        edited.gui.server = server;
        edited.save(&self.paths).await?;
        *config = edited;
        Ok(())
    }

    /// Returns the `idle` section.
    ///
    /// # Errors
    ///
    /// Returns an error if the config is unavailable.
    pub async fn idle(&self) -> Result<IdleConfig, NirvanaError> {
        Ok(self.config().await?.idle.clone())
    }

    /// Replaces the `idle` section and saves the config.
    ///
    /// # Errors
    ///
    /// Returns an error if the config is unavailable or cannot be saved.
    pub async fn set_idle(&self, idle: IdleConfig) -> Result<(), NirvanaError> {
        let mut config = self.config_mut().await?;
        let mut edited = config.clone();
        edited.idle = idle;
        edited.save(&self.paths).await?;
        *config = edited;
        Ok(())
    }
}

/// Returns `config` with `edit` applied to the section table and key name of
/// `key` (`section.key`).
fn edited_config(
    config: &AppConfig,
    key: &str,
    edit: impl FnOnce(&mut toml::Table, &str),
) -> Result<AppConfig, NirvanaError> {
    let (section, name) = split_key(key)?;
    let mut table = toml::Table::try_from(config).map_err(ConfigError::from)?;
    let section = table
        .get_mut(section)
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| DomainError::UnknownConfigKey(key.to_owned()))?;
    edit(section, name);
    table
        .try_into()
        .map_err(|e| DomainError::InvalidConfigValue(key.to_owned(), e).into())
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
    use std::io::Write;

    use super::*;
    use crate::config::{IdleMethod, LogLevel};
    use crate::service::test_service::TestService;

    fn append_default_theme(service: &TestService) {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&service.paths.config_file)
            .unwrap();
        writeln!(file, "\n[gui]\ntheme = \"system\"").unwrap();
    }

    fn entry(key: &'static str, value: Option<&str>, default: Option<&str>) -> ConfigEntry {
        ConfigEntry {
            key,
            value: value.map(str::to_owned),
            default: default.map(str::to_owned),
        }
    }

    #[tokio::test]
    async fn get_config_returns_all_keys_with_values_and_defaults() {
        let service = TestService::new(&[]).await;
        service.set_config("idle.enabled", "false").await.unwrap();

        assert_eq!(
            service.get_config(None).await.unwrap(),
            [
                entry("core.profile", Some("test"), None),
                entry("core.port", None, Some("5167")),
                entry("gui.theme", None, Some("system")),
                entry("gui.server", None, Some("true")),
                entry("idle.enabled", Some("false"), Some("true")),
                entry("idle.methods", None, Some(r#"["lock", "sleep", "input"]"#)),
                entry("idle.threshold_secs", None, Some("300")),
                entry("log.level", None, Some("trace")),
            ]
        );
    }

    #[tokio::test]
    async fn get_config_selects_section_or_key() {
        let service = TestService::new(&[]).await;

        let keys = async |key| -> Vec<&str> {
            service
                .get_config(Some(key))
                .await
                .unwrap()
                .into_iter()
                .map(|entry| entry.key)
                .collect()
        };

        assert_eq!(
            keys("idle").await,
            ["idle.enabled", "idle.methods", "idle.threshold_secs"]
        );
        assert_eq!(keys("idle.methods").await, ["idle.methods"]);
    }

    #[tokio::test]
    async fn get_config_rejects_unknown_key() {
        let service = TestService::new(&[]).await;

        for key in ["foo", "idle.foo", "schema_version", ""] {
            let err = service.get_config(Some(key)).await.unwrap_err();
            assert!(
                matches!(&err, NirvanaError::Domain(DomainError::UnknownConfigKey(unknown)) if unknown == key),
                "{key}: {err}"
            );
        }
    }

    #[tokio::test]
    async fn get_config_shows_default_value_present_in_file() {
        let service = TestService::new(&[]).await;
        append_default_theme(&service);

        assert_eq!(
            service.get_config(Some("gui.theme")).await.unwrap(),
            [entry("gui.theme", Some("system"), Some("system"))]
        );
    }

    #[tokio::test]
    async fn theme_returns_saved_theme() {
        let service = TestService::new(&[]).await;
        assert_eq!(service.theme().await.unwrap(), Theme::System);

        service.set_config("gui.theme", "dark").await.unwrap();

        assert_eq!(service.theme().await.unwrap(), Theme::Dark);
    }

    #[tokio::test]
    async fn port_returns_saved_port() {
        let service = TestService::new(&[]).await;
        assert_eq!(service.port().await.unwrap(), 5167);

        service.set_config("core.port", "8080").await.unwrap();

        assert_eq!(service.port().await.unwrap(), 8080);
    }

    #[tokio::test]
    async fn set_server_saves_server() {
        let service = TestService::new(&[]).await;
        assert!(service.server().await.unwrap());

        service.set_server(false).await.unwrap();

        assert!(!service.server().await.unwrap());
        let saved = AppConfig::load(&service.paths).await.unwrap();
        assert!(!saved.gui.server);
    }

    #[tokio::test]
    async fn set_idle_saves_idle_section() {
        let service = TestService::new(&[]).await;
        let idle = IdleConfig {
            enabled: false,
            methods: vec![IdleMethod::Sleep],
            threshold_secs: 600,
        };

        service.set_idle(idle.clone()).await.unwrap();

        assert_eq!(service.idle().await.unwrap(), idle);
        let saved = AppConfig::load(&service.paths).await.unwrap();
        assert_eq!(saved.idle, idle);
    }

    #[tokio::test]
    async fn concurrent_config_changes_are_all_saved() {
        let service = TestService::new(&[]).await;
        let idle = IdleConfig {
            enabled: false,
            ..IdleConfig::default()
        };

        let (theme, idle_set) = tokio::join!(
            service.set_config("gui.theme", "dark"),
            service.set_idle(idle.clone())
        );
        theme.unwrap();
        idle_set.unwrap();

        let saved = AppConfig::load(&service.paths).await.unwrap();
        assert_eq!(saved.gui.theme, Theme::Dark);
        assert_eq!(saved.idle, idle);
    }

    #[tokio::test]
    async fn set_config_parses_value_for_key_and_saves() {
        let service = TestService::new(&[]).await;

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

        let saved = AppConfig::load(&service.paths).await.unwrap();
        assert_eq!(saved, *service.config().await.unwrap());
        assert_eq!(saved.gui.theme, Theme::Dark);
        assert_eq!(saved.idle.threshold_secs, 600);
        assert_eq!(saved.idle.methods, [IdleMethod::Lock]);
        assert_eq!(saved.log.level, Some(LogLevel::Debug));
    }

    #[tokio::test]
    async fn set_config_rejects_invalid_value() {
        let service = TestService::new(&[]).await;
        let content = std::fs::read_to_string(&service.paths.config_file).unwrap();

        for (key, value) in [
            ("core.port", "70000"),
            ("core.port", "x"),
            ("gui.theme", "blue"),
            ("idle.enabled", "yes"),
            ("idle.threshold_secs", "-1"),
            ("idle.methods", r#"["typing"]"#),
        ] {
            let err = service.set_config(key, value).await.unwrap_err();
            assert!(
                matches!(&err, NirvanaError::Domain(DomainError::InvalidConfigValue(invalid, _)) if invalid == key),
                "{key}: {err}"
            );
        }
        assert_eq!(
            std::fs::read_to_string(&service.paths.config_file).unwrap(),
            content
        );
    }

    #[tokio::test]
    async fn set_config_rejects_unknown_key() {
        let service = TestService::new(&[]).await;

        for key in ["idle", "idle.foo", "schema_version"] {
            let err = service.set_config(key, "1").await.unwrap_err();
            assert!(
                matches!(&err, NirvanaError::Domain(DomainError::UnknownConfigKey(unknown)) if unknown == key),
                "{key}: {err}"
            );
        }
    }

    #[tokio::test]
    async fn set_config_keeps_config_when_save_fails() {
        let service = TestService::new(&[]).await;
        std::fs::remove_file(&service.paths.config_file).unwrap();
        std::fs::create_dir(&service.paths.config_file).unwrap();

        service.set_config("gui.theme", "dark").await.unwrap_err();

        assert_eq!(service.config().await.unwrap().gui.theme, Theme::default());
    }

    #[tokio::test]
    async fn unset_config_removes_key_and_restores_default() {
        let service = TestService::new(&[]).await;
        service.set_config("gui.theme", "dark").await.unwrap();
        service.set_config("log.level", "debug").await.unwrap();

        service.unset_config("gui.theme").await.unwrap();
        service.unset_config("log.level").await.unwrap();

        let saved = AppConfig::load(&service.paths).await.unwrap();
        assert_eq!(saved, *service.config().await.unwrap());
        assert_eq!(saved.gui.theme, Theme::default());
        assert_eq!(saved.log.level, None);
        let content = std::fs::read_to_string(&service.paths.config_file).unwrap();
        assert!(!content.contains("[gui]"), "{content}");
        assert!(!content.contains("[log]"), "{content}");
    }

    #[tokio::test]
    async fn unset_config_removes_default_value_present_in_file() {
        let service = TestService::new(&[]).await;
        append_default_theme(&service);

        service.unset_config("gui.theme").await.unwrap();

        assert_eq!(
            service.get_config(Some("gui.theme")).await.unwrap(),
            [entry("gui.theme", None, Some("system"))]
        );
    }

    #[tokio::test]
    async fn unset_config_rejects_unknown_and_profile_key() {
        let service = TestService::new(&[]).await;

        for key in ["idle", "idle.foo", "schema_version"] {
            let err = service.unset_config(key).await.unwrap_err();
            assert!(
                matches!(&err, NirvanaError::Domain(DomainError::UnknownConfigKey(unknown)) if unknown == key),
                "{key}: {err}"
            );
        }
        let err = service.unset_config(PROFILE_KEY).await.unwrap_err();
        assert!(
            matches!(&err, NirvanaError::Domain(DomainError::ConfigKeyRequired(key)) if key == PROFILE_KEY),
            "{err}"
        );
    }

    #[tokio::test]
    async fn set_config_profile_switches_to_existing_profile() {
        let service = TestService::new(&[]).await;
        service.create_profile("work".to_string()).await.unwrap();

        service.set_config(PROFILE_KEY, "work").await.unwrap();
        let err = service
            .set_config(PROFILE_KEY, "missing")
            .await
            .unwrap_err();

        assert_eq!(
            AppConfig::load(&service.paths)
                .await
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
