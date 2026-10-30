use std::cmp::max;

use chrono::{Local, TimeZone};
use clap::ValueEnum;
use console::{Style, Term};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Password, Select};
use nirvana_core::domain::{Connection, ConnectionData};
use nirvana_core::secrets::{SecretStore, selectable_stores};
use nirvana_core::service::Nirvana;

use crate::cli::{AddArgs, ConnectionKind};

enum Column {
    Unlimited(&'static str),
    Limited(&'static str, usize),
    Static(&'static str, usize),
}

impl Column {
    fn header(&self) -> &'static str {
        match self {
            Column::Unlimited(h) | Column::Limited(h, _) | Column::Static(h, _) => h,
        }
    }

    fn initial_width(&self) -> usize {
        match self {
            Column::Unlimited(h) => h.len(),
            Column::Limited(h, _) => h.len(),
            Column::Static(h, n) => max(h.len(), *n),
        }
    }

    fn fit(&self, current: usize, cell: &str) -> usize {
        match self {
            Column::Unlimited(_) => max(current, cell.len()),
            Column::Limited(_, cap) => max(current, cell.len().min(*cap)),
            Column::Static(_, _) => current,
        }
    }

    fn render(&self, cell: &str) -> String {
        match self {
            Column::Limited(_, cap) if cell.len() > *cap => {
                let mut s = cell[..cap.saturating_sub(1)].to_string();
                s.push('…');
                s
            }
            Column::Static(_, n) if cell.len() > *n => cell[..*n].to_string(),
            _ => cell.to_string(),
        }
    }
}

const COLUMNS: &[Column] = &[
    Column::Unlimited("ID"),
    Column::Static("ACTIVE", "ACTIVE".len()),
    Column::Limited("NAME", 15),
    Column::Unlimited("KIND"),
    Column::Limited("HOST", 30),
    Column::Limited("IDENTITY", 25),
    Column::Static("UPDATED", "2026-05-17".len()),
];

pub(crate) async fn list() -> anyhow::Result<()> {
    let api = Nirvana::new().await?;
    let connections = api.list_connections().await?;

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
            let active = if is_active(c, &api) { "*" } else { "" };
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

    let mut widths: Vec<usize> = COLUMNS.iter().map(|c| c.initial_width()).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = COLUMNS[i].fit(widths[i], cell);
        }
    }

    let bold = Style::new().bold();
    let header: Vec<&str> = COLUMNS.iter().map(|c| c.header()).collect();
    print_row(&term, &header, &widths, Some(&bold))?;

    for row in &rows {
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| COLUMNS[i].render(cell))
            .collect();
        let cell_refs: Vec<&str> = cells.iter().map(|s| s.as_str()).collect();
        print_row(&term, &cell_refs, &widths, None)?;
    }

    Ok(())
}

fn is_active(conn: &Connection, api: &Nirvana) -> bool {
    api.active_profile()
        .is_ok_and(|id| id == conn.model.profile_id)
}

fn print_row(
    term: &Term,
    cells: &[&str],
    widths: &[usize],
    style: Option<&Style>,
) -> anyhow::Result<()> {
    debug_assert_eq!(cells.len(), widths.len());
    debug_assert_eq!(cells.len(), COLUMNS.len());

    let parts: Vec<String> = cells
        .iter()
        .zip(widths)
        .enumerate()
        .map(|(i, (cell, w))| {
            let padded = match i {
                0 => format!("{:>w$}", cell, w = *w),
                1 => format!("{:^w$}", cell, w = *w),
                _ => format!("{:<w$}", cell, w = *w),
            };
            match style {
                Some(s) => s.apply_to(padded).to_string(),
                None => padded,
            }
        })
        .collect();

    term.write_line(&parts.join("  "))?;
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

    api.set_active_profile(chosen.model.profile_id).await?;
    let term = Term::stdout();
    term.write_line(&format!(
        "Active profile set to id {}",
        chosen.model.profile_id
    ))?;
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
        api.set_active_profile(connection.model.profile_id).await?;
        term.write_line(&format!(
            "Active profile set to id {}",
            connection.model.profile_id
        ))?;
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
