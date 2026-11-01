use chrono::{Local, TimeZone};
use clap::ValueEnum;
use console::{Alignment, Style, Term};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Password, Select};
use nirvana_core::domain::{Connection, ConnectionData};
use nirvana_core::secrets::{SecretStore, selectable_stores};
use nirvana_core::service::Nirvana;

use crate::cli::table::{self, Column};
use crate::cli::{AddArgs, ConnectionKind};

const COLUMNS: [Column; 7] = [
    Column::new("ID").align(Alignment::Right),
    Column::new("ACTIVE").align(Alignment::Center),
    Column::new("NAME").max_width(15),
    Column::new("KIND"),
    Column::new("HOST").max_width(30),
    Column::new("IDENTITY").max_width(25),
    Column::new("UPDATED"),
];

pub(crate) async fn list() -> anyhow::Result<()> {
    let api = Nirvana::new().await?;
    let connections = api.list_connections().await?;
    let active_profile = api.active_profile().await.ok();

    let term = Term::stdout();
    if connections.is_empty() {
        term.write_line("No connections found.")?;
        term.write_line("Add one with: nirvana connection add")?;
        return Ok(());
    }

    let rows: Vec<[String; COLUMNS.len()]> = connections
        .iter()
        .map(|c| {
            let updated = Local
                .timestamp_opt(c.model.updated_at, 0)
                .single()
                .unwrap()
                .date_naive()
                .to_string();
            let active = if active_profile == Some(c.model.profile_id) {
                "*"
            } else {
                ""
            };
            [
                c.model.id.to_string(),
                active.to_string(),
                c.model.name.clone(),
                c.kind.as_str().to_string(),
                c.model.host.clone(),
                c.model.identity.clone(),
                updated,
            ]
        })
        .collect();

    table::print(&term, &COLUMNS, &rows)?;

    Ok(())
}

pub(crate) async fn activate(query: Option<&str>) -> anyhow::Result<()> {
    let mut api = Nirvana::new().await?;
    let connections = api.list_connections().await?;

    if connections.is_empty() {
        let term = Term::stdout();
        term.write_line("No connections found.")?;
        term.write_line("Add one with: nirvana connection add")?;
        return Ok(());
    }

    let idx = match query {
        Some(q) => resolve_query(q, &connections)?,
        None => pick_interactive(&connections)?,
    };
    let chosen = &connections[idx];

    let profile = switch_to_profile(&mut api, chosen.model.profile_id).await?;
    let term = Term::stdout();
    term.write_line(&format!("Active profile set to '{profile}'"))?;
    Ok(())
}

fn resolve_query(query: &str, connections: &[Connection]) -> anyhow::Result<usize> {
    if let Ok(id) = query.parse::<i64>()
        && let Some(idx) = connections.iter().position(|c| c.model.id == id)
    {
        return Ok(idx);
    }
    if let Some(idx) = connections.iter().position(|c| c.model.name == query) {
        return Ok(idx);
    }
    anyhow::bail!("Connection '{query}' not found")
}

fn pick_interactive(connections: &[Connection]) -> anyhow::Result<usize> {
    let dim = Style::new().dim();
    let name_width = connections
        .iter()
        .map(|c| c.model.name.len())
        .max()
        .unwrap_or(0);

    let items: Vec<String> = connections
        .iter()
        .map(|c| {
            format!(
                "{:name_w$}  {} {}",
                c.model.name,
                dim.apply_to(&c.model.identity),
                dim.apply_to(format!("({})", c.kind.as_str())),
                name_w = name_width,
            )
        })
        .collect();

    let theme = ColorfulTheme {
        active_item_prefix: Style::new().green().apply_to(String::from("❯")),
        ..ColorfulTheme::default()
    };

    let idx = Select::with_theme(&theme)
        .with_prompt("Select a connection")
        .items(&items)
        .default(0)
        .interact_opt()?;

    match idx {
        Some(i) => Ok(i),
        None => std::process::exit(0),
    }
}

pub(crate) async fn add(args: AddArgs) -> anyhow::Result<()> {
    let mut api = Nirvana::new().await?;
    let data = if args.name.is_none() {
        add_interactive(&api).await?
    } else {
        add_command(args)?
    };

    let connection = api.add_connection(data).await?;

    let term = Term::stdout();
    term.write_line(&format!(
        "Added connection '{}' (id {})",
        connection.model.name, connection.model.id
    ))?;

    let use_it = dialoguer::Confirm::new()
        .with_prompt("Use this connection?")
        .default(true)
        .interact()?;

    if use_it {
        let profile = switch_to_profile(&mut api, connection.model.profile_id).await?;
        term.write_line(&format!("Active profile set to '{profile}'"))?;
    }

    Ok(())
}

fn add_command(args: AddArgs) -> anyhow::Result<ConnectionData> {
    Ok(ConnectionData {
        name: args.name.unwrap(),
        kind: args.kind.unwrap().to_string(),
        host: args.host.unwrap(),
        identity: args.identity.unwrap(),
        secret_store: args.storage.unwrap(),
        token: args.token.unwrap().into(),
    })
}

async fn switch_to_profile(api: &mut Nirvana, profile_id: i64) -> anyhow::Result<String> {
    let profile = api
        .list_profiles()
        .await?
        .into_iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| anyhow::anyhow!("profile {profile_id} not found"))?;
    api.switch_profile(profile.name.clone()).await?;
    Ok(profile.name)
}

async fn add_interactive(api: &Nirvana) -> anyhow::Result<ConnectionData> {
    let theme = ColorfulTheme {
        active_item_prefix: Style::new().green().apply_to(String::from("❯")),
        ..ColorfulTheme::default()
    };

    let name: String = Input::with_theme(&theme)
        .with_prompt("Connection name")
        .interact_text()?;

    let kinds = ConnectionKind::value_variants()
        .iter()
        .map(|k| match k {
            ConnectionKind::JiraCloud => "Jira Cloud",
            ConnectionKind::JiraDc => "Jira DC",
        })
        .collect::<Vec<_>>();

    let kind_idx = Select::with_theme(&theme)
        .with_prompt("Connection type")
        .items(&kinds)
        .default(0)
        .interact_opt()?;

    let kind = match kind_idx {
        Some(i) => ConnectionKind::value_variants()[i].clone(),
        None => std::process::exit(0),
    };

    let host: String = Input::with_theme(&theme)
        .with_prompt("Host")
        .interact_text()?;

    let identity_label = match &kind {
        ConnectionKind::JiraCloud => "Email",
        ConnectionKind::JiraDc => "Username",
    };

    let identity: String = Input::with_theme(&theme)
        .with_prompt(identity_label)
        .interact_text()?;

    let dim = Style::new().dim();
    let storage_items = selectable_stores();
    let storage_labels: Vec<String> = storage_items
        .iter()
        .map(|s| match s {
            SecretStore::Keyring => format!("Keyring {}", dim.apply_to("(recommended)")),
            SecretStore::Plaintext => format!("Plaintext {}", dim.apply_to("(stored in database)")),
            SecretStore::Env => format!("Env {}", dim.apply_to("(environment variable)")),
        })
        .collect();

    let preferred_store = if api.keyring_status().await.is_ok() {
        SecretStore::Keyring
    } else {
        SecretStore::Plaintext
    };
    let storage_default = storage_items
        .iter()
        .position(|s| *s == preferred_store)
        .unwrap_or(0);

    let storage_idx = Select::with_theme(&theme)
        .with_prompt("Secret storage")
        .items(&storage_labels)
        .default(storage_default)
        .interact_opt()?;

    let storage = match storage_idx {
        Some(i) => storage_items[i],
        None => std::process::exit(0),
    };

    let token_label = match &kind {
        ConnectionKind::JiraCloud => "API token",
        ConnectionKind::JiraDc => "Personal access token",
    };

    let token = Password::with_theme(&theme)
        .with_prompt(token_label)
        .interact()?;

    Ok(ConnectionData {
        name,
        kind: kind.to_string(),
        host,
        identity,
        secret_store: storage,
        token: token.into(),
    })
}

pub(crate) async fn test() -> anyhow::Result<()> {
    let api = Nirvana::new().await?;
    let term = Term::stdout();

    match api.test_connection().await {
        Ok(()) => term.write_line("Connection OK")?,
        Err(e) => return Err(e.into()),
    }
    Ok(())
}
