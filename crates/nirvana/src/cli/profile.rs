use console::{Alignment, Style, Term, style};
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use nirvana_core::service::Nirvana;

use crate::cli::table::{self, Column};
use crate::cli::time::format_timestamp;

const COLUMNS: [Column; 4] = [
    Column::new("ACTIVE").align(Alignment::Center),
    Column::new("NAME").max_width(30),
    Column::new("CREATED"),
    Column::new("UPDATED"),
];

pub(crate) async fn switch(name: Option<String>) -> anyhow::Result<()> {
    let mut service = Nirvana::new().await?;

    let name = match name {
        Some(name) => name,
        None => match switch_interactive(&service).await? {
            Some(name) => name,
            None => return Ok(()),
        },
    };

    service.switch_profile(&name).await?;

    let term = Term::stdout();
    term.write_line(&format!(
        "{} to profile {}",
        style("Switched").green(),
        style(&name).bold(),
    ))?;

    Ok(())
}

async fn switch_interactive(service: &Nirvana) -> anyhow::Result<Option<String>> {
    let profiles = service.list_profiles().await?;

    if profiles.is_empty() {
        let term = Term::stdout();
        term.write_line("No profiles found.")?;
        term.write_line("Create one with: nirvana profile create")?;
        return Ok(None);
    }

    let mut items: Vec<String> = profiles.into_iter().map(|c| c.name).collect();

    let theme = ColorfulTheme {
        active_item_prefix: Style::new().green().apply_to(String::from("❯")),
        ..ColorfulTheme::default()
    };

    let idx = Select::with_theme(&theme)
        .with_prompt("Select a profile")
        .items(&items)
        .default(0)
        .report(false)
        .interact_opt()?;

    Ok(idx.map(|i| items.swap_remove(i)))
}

pub(crate) async fn list() -> anyhow::Result<()> {
    let service = Nirvana::new().await?;
    let profiles = service.list_profiles().await?;
    let active_profile = service.active_profile().await?;

    let rows: Vec<[String; 4]> = profiles
        .into_iter()
        .map(|profile| {
            let active = if profile.id == active_profile.profile.id {
                "*"
            } else {
                ""
            };
            [
                active.to_string(),
                profile.name,
                format_timestamp(profile.created_at),
                format_timestamp(profile.updated_at),
            ]
        })
        .collect();

    table::print(&Term::stdout(), &COLUMNS, &rows)
}

pub(crate) async fn create(name: Option<String>, _switch: bool) -> anyhow::Result<()> {
    match name {
        Some(name) => println!("creating {name}"),
        None => println!("interactive create"),
    }
    Ok(())
}

pub(crate) async fn delete(name: Option<String>, _yes: bool) -> anyhow::Result<()> {
    match name {
        Some(name) => println!("deleting {name}"),
        None => println!("interactive delete"),
    }
    Ok(())
}
