use console::{Term, style};
use nirvana_core::Nirvana;
use tracing::info;

use crate::cli::table::{self, Column};

const COLUMNS: [Column; 3] = [
    Column::new("KEY"),
    Column::new("VALUE"),
    Column::new("DEFAULT"),
];

pub(crate) async fn get(key: Option<String>, show_unset: bool) -> anyhow::Result<()> {
    let service = Nirvana::new().await?;
    let entries = service.get_config(key.as_deref())?;
    let term = Term::stdout();

    if key.is_some_and(|key| key.contains('.')) {
        let value = entries.into_iter().find_map(|entry| entry.value);
        term.write_line(&value.unwrap_or_default())?;
        return Ok(());
    }

    let rows: Vec<[String; 3]> = entries
        .into_iter()
        .filter(|entry| show_unset || entry.value.is_some())
        .map(|entry| {
            [
                entry.key.to_string(),
                entry.value.unwrap_or_default(),
                entry.default.unwrap_or_default(),
            ]
        })
        .collect();

    if rows.is_empty() {
        term.write_line(&format!(
            "{}",
            style("No config values set, use --show-unset to show all").yellow()
        ))?;
        return Ok(());
    }

    table::print(&term, &COLUMNS, &rows)
}

pub(crate) async fn set(key: String, value: String) -> anyhow::Result<()> {
    let mut service = Nirvana::new().await?;
    service.set_config(&key, &value).await?;
    info!(%key, %value, "config changed");

    Term::stdout().write_line(&format!(
        "{} {}",
        style("Updated").green(),
        style(&key).bold(),
    ))?;
    Ok(())
}

pub(crate) async fn unset(key: String) -> anyhow::Result<()> {
    let mut service = Nirvana::new().await?;
    service.unset_config(&key)?;
    info!(%key, "config unset");

    Term::stdout().write_line(&format!(
        "{} {}",
        style("Reset").green(),
        style(&key).bold(),
    ))?;
    Ok(())
}
