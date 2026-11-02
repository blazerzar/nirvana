use console::{Term, style};
use dialoguer::{Input, Select};
use nirvana_core::secrets::SecretStore;
use nirvana_core::service::Nirvana;

use crate::cli::{ConnectionKind, theme};

pub(crate) async fn add(
    name: Option<String>,
    kind: Option<ConnectionKind>,
    host: Option<String>,
    credential: Option<String>,
    storage: Option<SecretStore>,
) -> anyhow::Result<()> {
    match (name, kind, host, credential, storage) {
        (Some(name), Some(kind), Some(host), Some(_), Some(storage)) => {
            println!("adding {name} ({kind}, {host}, {storage})")
        }
        _ => println!("interactive add"),
    }
    Ok(())
}

pub(crate) async fn list() -> anyhow::Result<()> {
    println!("listing all connections");
    Ok(())
}

fn print_no_connections(term: &Term) -> std::io::Result<()> {
    term.write_line(&format!("{}", style("No connections found").yellow()))?;
    term.write_line(&format!(
        "Add one with: {}",
        style("nirvana connection add").bold()
    ))
}

async fn select_connection(service: &Nirvana, prompt: &str) -> anyhow::Result<Option<String>> {
    let connections = service.active_profile().await?.connections;

    let term = Term::stdout();
    if connections.is_empty() {
        print_no_connections(&term)?;
        return Ok(None);
    }

    let mut items: Vec<String> = connections.into_iter().map(|c| c.model.name).collect();
    let idx = Select::with_theme(&theme())
        .with_prompt(prompt)
        .items(&items)
        .default(0)
        .interact_opt()?;

    if idx.is_none() {
        term.write_line(&format!("{}", style("No connection selected").yellow()))?;
    }
    Ok(idx.map(|i| items.swap_remove(i)))
}

pub(crate) async fn rename(name: Option<String>, new_name: Option<String>) -> anyhow::Result<()> {
    let service = Nirvana::new().await?;

    let (name, new_name) = match name.zip(new_name) {
        Some(names) => names,
        None => match rename_interactive(&service).await? {
            Some(names) => names,
            None => return Ok(()),
        },
    };

    service.rename_connection(&name, new_name.clone()).await?;

    let term = Term::stdout();
    term.write_line(&format!(
        "{} connection {} to {}",
        style("Renamed").green(),
        style(&name).dim(),
        style(&new_name).bold(),
    ))?;

    Ok(())
}

async fn rename_interactive(service: &Nirvana) -> anyhow::Result<Option<(String, String)>> {
    let Some(name) = select_connection(service, "Select a connection to rename").await? else {
        return Ok(None);
    };

    let new_name = Input::<String>::with_theme(&theme())
        .with_prompt("New name")
        .interact_text()?;

    Ok(Some((name, new_name)))
}

pub(crate) async fn delete(
    name: Option<String>,
    with_slots: bool,
    yes: bool,
) -> anyhow::Result<()> {
    match name {
        Some(name) => println!("deleting {name} (with slots: {with_slots}, yes: {yes})"),
        None => println!("interactive delete"),
    }
    Ok(())
}

pub(crate) async fn test(name: Option<String>, all: bool) -> anyhow::Result<()> {
    match (name, all) {
        (_, true) => println!("testing all connections"),
        (Some(name), false) => println!("testing {name}"),
        (None, false) => println!("interactive test"),
    }
    Ok(())
}
