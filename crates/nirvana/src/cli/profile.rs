use console::{Alignment, Style, Term, style};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, Select};
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
        None => {
            let active = service.active_profile().await.ok().map(|p| p.profile.name);
            match select_profile(&service, "Select a profile", active.as_deref()).await? {
                Some(name) => name,
                None => return Ok(()),
            }
        }
    };

    let term = Term::stdout();
    if service.switch_profile(&name).await? {
        print_switched(&term, &name)?;
    } else {
        term.write_line(&format!(
            "{}",
            style(format!("Profile {name} is already active")).yellow()
        ))?;
    }

    Ok(())
}

fn print_switched(term: &Term, name: &str) -> std::io::Result<()> {
    term.write_line(&format!(
        "{} to profile {}",
        style("Switched").green(),
        style(name).bold(),
    ))
}

fn print_no_profiles(term: &Term) -> std::io::Result<()> {
    term.write_line(&format!("{}", style("No profiles found").yellow()))?;
    term.write_line(&format!(
        "Create one with: {}",
        style("nirvana profile create").bold()
    ))
}

async fn select_profile(
    service: &Nirvana,
    prompt: &str,
    default: Option<&str>,
) -> anyhow::Result<Option<String>> {
    let profiles = service.list_profiles().await?;

    let term = Term::stdout();
    if profiles.is_empty() {
        print_no_profiles(&term)?;
        return Ok(None);
    }

    let mut items: Vec<String> = profiles.into_iter().map(|c| c.name).collect();
    let default_idx = default
        .and_then(|name| items.iter().position(|item| item == name))
        .unwrap_or(0);

    let idx = Select::with_theme(&theme())
        .with_prompt(prompt)
        .items(&items)
        .default(default_idx)
        .interact_opt()?;

    if idx.is_none() {
        term.write_line(&format!("{}", style("No profile selected").yellow()))?;
    }
    Ok(idx.map(|i| items.swap_remove(i)))
}

fn theme() -> ColorfulTheme {
    ColorfulTheme {
        active_item_prefix: Style::new().green().apply_to(String::from("❯")),
        ..ColorfulTheme::default()
    }
}

pub(crate) async fn list() -> anyhow::Result<()> {
    let service = Nirvana::new().await?;
    let profiles = service.list_profiles().await?;
    if profiles.is_empty() {
        return Ok(print_no_profiles(&Term::stdout())?);
    }
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

pub(crate) async fn create(name: Option<String>, switch: bool) -> anyhow::Result<()> {
    let (name, switch) = match name {
        Some(name) => (name, switch),
        None => create_interactive(switch).await?,
    };

    let mut service = Nirvana::new().await?;
    let created = service.create_profile(name).await?;

    let term = Term::stdout();
    term.write_line(&format!(
        "{} profile {}",
        style("Created").green(),
        style(&created.name).bold(),
    ))?;

    if switch {
        service.switch_profile(&created.name).await?;
        print_switched(&term, &created.name)?;
    }

    Ok(())
}

async fn create_interactive(switch: bool) -> anyhow::Result<(String, bool)> {
    let name = Input::<String>::with_theme(&theme())
        .with_prompt("Profile name")
        .interact_text()?;
    let switch = switch
        || Confirm::with_theme(&theme())
            .with_prompt("Do you want to switch to the new profile?")
            .default(true)
            .interact()?;

    Ok((name, switch))
}

pub(crate) async fn delete(name: Option<String>, yes: bool) -> anyhow::Result<()> {
    let mut service = Nirvana::new().await?;

    let name = match name {
        Some(name) => name,
        None => match delete_interactive(&service).await? {
            Some(name) => name,
            None => return Ok(()),
        },
    };

    let term = Term::stdout();
    if yes
        || Confirm::with_theme(&theme())
            .with_prompt(format!("Are you sure you want to delete profile {name}?"))
            .default(false)
            .interact()?
    {
        let active = service.delete_profile(&name).await?;
        term.write_line(&format!(
            "{} profile {}",
            style("Deleted").green(),
            style(&name).bold(),
        ))?;
        if let Some(active) = active {
            print_switched(&term, &active)?;
        }
    } else {
        term.write_line(&format!(
            "{}",
            style(format!("Profile {name} was not deleted")).yellow()
        ))?;
    }

    Ok(())
}

async fn delete_interactive(service: &Nirvana) -> anyhow::Result<Option<String>> {
    let Some(name) = select_profile(service, "Select a profile to delete", None).await? else {
        return Ok(None);
    };
    Ok(Some(name))
}

pub(crate) async fn rename(name: Option<String>, new_name: Option<String>) -> anyhow::Result<()> {
    let mut service = Nirvana::new().await?;

    let (name, new_name) = match name.zip(new_name) {
        Some(names) => names,
        None => match rename_interactive(&service).await? {
            Some(names) => names,
            None => return Ok(()),
        },
    };

    service.rename_profile(&name, new_name.clone()).await?;

    let term = Term::stdout();
    term.write_line(&format!(
        "{} profile {} to {}",
        style("Renamed").green(),
        style(&name).dim(),
        style(&new_name).bold(),
    ))?;

    Ok(())
}

async fn rename_interactive(service: &Nirvana) -> anyhow::Result<Option<(String, String)>> {
    let Some(name) = select_profile(service, "Select a profile to rename", None).await? else {
        return Ok(None);
    };

    let new_name = Input::<String>::with_theme(&theme())
        .with_prompt("New name")
        .interact_text()?;

    Ok(Some((name, new_name)))
}
