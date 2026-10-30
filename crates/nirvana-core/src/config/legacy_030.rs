use serde::Deserialize;

use super::{
    AppConfig, CURRENT_SCHEMA_VERSION, ConfigError, CoreConfig, GuiConfig, IdleConfig, LogConfig,
    Theme,
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Config {
    gui: Gui,
    idle: IdleConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct Gui {
    theme: String,
}

impl Default for Gui {
    fn default() -> Self {
        Self {
            theme: "high-contrast-dark".into(),
        }
    }
}

pub(super) fn parse(table: toml::Table) -> Result<AppConfig, ConfigError> {
    let config: Config = table.try_into()?;
    Ok(AppConfig {
        schema_version: CURRENT_SCHEMA_VERSION,
        core: CoreConfig::default(),
        gui: GuiConfig {
            theme: map_theme(&config.gui.theme),
        },
        idle: config.idle,
        log: LogConfig::default(),
    })
}

fn map_theme(theme: &str) -> Theme {
    match theme {
        "soft-light" => Theme::Light,
        "nirvana-dark" | "high-contrast-dark" => Theme::Dark,
        _ => Theme::System,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::tests::{FIXTURE_020, FIXTURE_030, parse};

    /// Parses the content of a fixture file into the current config.
    fn parse_fixture(content: &str) -> AppConfig {
        parse(content).unwrap()
    }

    #[test]
    fn migrates_020_fixture_without_idle_section() {
        let config = parse_fixture(FIXTURE_020);

        assert_eq!(
            config,
            AppConfig {
                gui: GuiConfig {
                    theme: Theme::Light,
                },
                ..AppConfig::default()
            }
        );
    }

    #[test]
    fn migrates_030_fixture() {
        let config = parse_fixture(FIXTURE_030);

        assert_eq!(
            config,
            AppConfig {
                gui: GuiConfig { theme: Theme::Dark },
                idle: IdleConfig {
                    enabled: true,
                    methods: vec!["lock".into()],
                    threshold_secs: 600,
                },
                ..AppConfig::default()
            }
        );
    }

    #[test]
    fn maps_themes() {
        assert_eq!(map_theme("soft-light"), Theme::Light);
        assert_eq!(map_theme("nirvana-dark"), Theme::Dark);
        assert_eq!(map_theme("high-contrast-dark"), Theme::Dark);
        assert_eq!(map_theme("unknown"), Theme::System);
    }

    #[test]
    fn wrong_type_fails() {
        let result = parse(
            r#"
schema_version = 1

[gui]
theme = "soft-light"

[idle]
enabled = true
methods = ["lock"]
threshold_secs = "600"
"#,
        );

        assert!(matches!(result, Err(ConfigError::Parse(_))));
    }
}
