use anyhow::anyhow;
use console::{Term, style};
use nirvana_core::{JiraSettings, JiraSettingsEdit, Nirvana, Settings, SettingsEdit};
use tracing::info;

use crate::cli::table::{self, Column};

const COLUMNS: [Column; 2] = [Column::new("KEY"), Column::new("VALUE")];

pub(crate) async fn get(connection: String, key: Option<String>) -> anyhow::Result<()> {
    let service = Nirvana::new().await?;
    let settings = service.get_connection(&connection).await?.settings;
    let values = values(&settings);
    let term = Term::stdout();

    if let Some(key) = key {
        let (_, value) = values
            .iter()
            .find(|(name, _)| *name == key)
            .ok_or_else(|| unknown_setting(&connection, &settings, &key))?;
        term.write_line(&value.to_string())?;
        return Ok(());
    }

    if values.is_empty() {
        term.write_line(&format!(
            "{}",
            style(format!("Connection {connection} has no settings")).yellow()
        ))?;
        return Ok(());
    }

    let rows: Vec<[String; 2]> = values
        .into_iter()
        .map(|(key, value)| [key.to_string(), value.to_string()])
        .collect();
    table::print(&term, &COLUMNS, &rows)
}

pub(crate) async fn set(connection: String, key: String, value: String) -> anyhow::Result<()> {
    let service = Nirvana::new().await?;
    let settings = service.get_connection(&connection).await?.settings;
    let edit = settings_edit(&connection, &settings, &key, &value)?;

    service.edit_connection_settings(&connection, edit).await?;
    info!(%connection, %key, %value, "connection setting changed");

    Term::stdout().write_line(&format!(
        "{} {} of connection {}",
        style("Updated").green(),
        style(&key).bold(),
        style(&connection).bold(),
    ))?;
    Ok(())
}

struct JiraSetting {
    key: &'static str,
    value: fn(&JiraSettings) -> bool,
    field: fn(&mut JiraSettingsEdit) -> &mut Option<bool>,
}

const JIRA_SETTINGS: [JiraSetting; 2] = [
    JiraSetting {
        key: "publish",
        value: |settings| settings.publish,
        field: |edit| &mut edit.publish,
    },
    JiraSetting {
        key: "squash",
        value: |settings| settings.squash,
        field: |edit| &mut edit.squash,
    },
];

fn values(settings: &Settings) -> Vec<(&'static str, bool)> {
    match settings {
        Settings::Jira(settings) => JIRA_SETTINGS
            .iter()
            .map(|setting| (setting.key, (setting.value)(settings)))
            .collect(),
        Settings::Linear => Vec::new(),
    }
}

fn settings_edit(
    connection: &str,
    settings: &Settings,
    key: &str,
    value: &str,
) -> anyhow::Result<SettingsEdit> {
    let setting = match settings {
        Settings::Jira(_) => JIRA_SETTINGS.iter().find(|setting| setting.key == key),
        Settings::Linear => None,
    }
    .ok_or_else(|| unknown_setting(connection, settings, key))?;
    let mut edit = JiraSettingsEdit::default();
    *(setting.field)(&mut edit) = Some(
        value
            .parse()
            .map_err(|_| anyhow!("setting '{key}' must be true or false"))?,
    );
    Ok(SettingsEdit::Jira(edit))
}

fn unknown_setting(connection: &str, settings: &Settings, key: &str) -> anyhow::Error {
    let keys: Vec<&str> = values(settings).into_iter().map(|(key, _)| key).collect();
    if keys.is_empty() {
        return anyhow!("connection '{connection}' has no settings");
    }
    anyhow!(
        "unknown setting '{key}' for connection '{connection}', expected one of: {}",
        keys.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jira() -> Settings {
        Settings::Jira(JiraSettings {
            publish: true,
            squash: false,
        })
    }

    #[test]
    fn settings_edit_sets_only_the_given_jira_setting() {
        assert!(matches!(
            settings_edit("jira", &jira(), "publish", "false").unwrap(),
            SettingsEdit::Jira(JiraSettingsEdit {
                publish: Some(false),
                squash: None,
            })
        ));
        assert!(matches!(
            settings_edit("jira", &jira(), "squash", "true").unwrap(),
            SettingsEdit::Jira(JiraSettingsEdit {
                publish: None,
                squash: Some(true),
            })
        ));
    }

    #[test]
    fn settings_edit_rejects_unknown_setting() {
        let error = settings_edit("jira", &jira(), "foo", "true").unwrap_err();

        assert_eq!(
            error.to_string(),
            "unknown setting 'foo' for connection 'jira', expected one of: publish, squash"
        );
    }

    #[test]
    fn settings_edit_rejects_non_bool_value() {
        let error = settings_edit("jira", &jira(), "publish", "yes").unwrap_err();

        assert_eq!(error.to_string(), "setting 'publish' must be true or false");
    }

    #[test]
    fn settings_edit_rejects_connection_without_settings() {
        let error = settings_edit("linear", &Settings::Linear, "publish", "true").unwrap_err();

        assert_eq!(error.to_string(), "connection 'linear' has no settings");
    }
}
