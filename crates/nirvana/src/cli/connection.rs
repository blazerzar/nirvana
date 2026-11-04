use std::time::{Duration, Instant};

use anyhow::bail;
use console::{Term, style};
use dialoguer::{Confirm, Input, Password, Select};
use indicatif::ProgressBar;
use nirvana_core::domain::{ConnectionData, Kind};
use nirvana_core::errors::{DomainError, NirvanaError};
use nirvana_core::secrets::{SecretStore, selectable_stores};
use nirvana_core::service::Nirvana;
use secrecy::SecretString;

use crate::cli::table::{self, Column};
use crate::cli::theme;
use crate::cli::time::format_timestamp;

const COLUMNS: [Column; 7] = [
    Column::new("NAME").max_width(30),
    Column::new("KIND"),
    Column::new("HOST").max_width(40),
    Column::new("IDENTITY").max_width(30),
    Column::new("STORAGE"),
    Column::new("CREATED"),
    Column::new("UPDATED"),
];

pub(crate) async fn add(
    name: Option<String>,
    kind: Option<Kind>,
    host: Option<String>,
    credential: Option<String>,
    storage: Option<SecretStore>,
) -> anyhow::Result<()> {
    let service = Nirvana::new().await?;

    let connection = match (name, kind, host, credential, storage) {
        (Some(name), Some(kind), Some(host), Some(credential), Some(secret_store)) => {
            let (identity, token) = split_credential(kind, credential)?;
            ConnectionData {
                name,
                kind,
                host,
                identity,
                secret_store,
                token: SecretString::from(token),
            }
        }
        _ => add_interactive(&service).await?,
    };

    let added = service.add_connection(connection).await?;

    let term = Term::stdout();
    term.write_line(&format!(
        "{} connection {}",
        style("Added").green(),
        style(&added.model.name).bold(),
    ))?;

    Ok(())
}

fn split_credential(kind: Kind, credential: String) -> anyhow::Result<(Option<String>, String)> {
    match kind {
        Kind::JiraCloud => match credential.split_once(':') {
            Some((email, token)) => Ok((Some(email.to_owned()), token.to_owned())),
            None => bail!("{kind} credential must be \"<email>:<token>\""),
        },
        Kind::JiraDc | Kind::Linear => Ok((None, credential)),
    }
}

async fn add_interactive(service: &Nirvana) -> anyhow::Result<ConnectionData> {
    service.active_profile().await?;

    let name = Input::<String>::with_theme(&theme())
        .with_prompt("Connection name")
        .interact_text()?;

    let kind = Kind::ALL[Select::with_theme(&theme())
        .with_prompt("Connection kind")
        .items(Kind::ALL)
        .default(0)
        .interact()?];

    let host = Input::<String>::with_theme(&theme())
        .with_prompt("Host URL")
        .interact_text()?;

    let identity = match kind {
        Kind::JiraCloud => Some(
            Input::<String>::with_theme(&theme())
                .with_prompt("Email")
                .interact_text()?,
        ),
        Kind::JiraDc | Kind::Linear => None,
    };

    let token = Password::with_theme(&theme())
        .with_prompt("Token")
        .interact()?;

    let keyring_available = service.keyring_status().await.is_ok();
    let stores: Vec<SecretStore> = selectable_stores()
        .iter()
        .copied()
        .filter(|store| keyring_available || *store != SecretStore::Keyring)
        .collect();
    let secret_store = stores[Select::with_theme(&theme())
        .with_prompt("Where to store the token")
        .items(&stores)
        .default(0)
        .interact()?];

    Ok(ConnectionData {
        name,
        kind,
        host,
        identity,
        secret_store,
        token: SecretString::from(token),
    })
}

pub(crate) async fn list() -> anyhow::Result<()> {
    let service = Nirvana::new().await?;
    let connections = service.list_connections().await?;
    let term = Term::stdout();
    if connections.is_empty() {
        print_no_connections(&term)?;
        return Ok(());
    }

    let rows: Vec<[String; 7]> = connections
        .into_iter()
        .map(|connection| {
            [
                connection.model.name,
                connection.kind.to_string(),
                connection.model.host,
                connection.model.identity.unwrap_or_default(),
                connection.secret_store.to_string(),
                format_timestamp(connection.model.created_at),
                format_timestamp(connection.model.updated_at),
            ]
        })
        .collect();

    table::print(&term, &COLUMNS, &rows)
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
    with_tickets: bool,
    yes: bool,
) -> anyhow::Result<()> {
    let service = Nirvana::new().await?;

    let (name, with_tickets) = match name {
        Some(name) => {
            let connections = service.active_profile().await?.connections;
            if !connections.iter().any(|c| c.model.name == name) {
                bail!(DomainError::ConnectionNotFound(name));
            }
            (name, with_tickets)
        }
        None => match delete_interactive(&service, with_tickets).await? {
            Some(choice) => choice,
            None => return Ok(()),
        },
    };

    let term = Term::stdout();
    let prompt = if with_tickets {
        format!("Are you sure you want to delete connection {name} with its tickets and slots?")
    } else {
        format!("Are you sure you want to delete connection {name}? Its tickets are kept.")
    };
    if yes
        || Confirm::with_theme(&theme())
            .with_prompt(prompt)
            .default(false)
            .interact()?
    {
        service.delete_connection(&name, with_tickets).await?;
        term.write_line(&format!(
            "{} connection {}",
            style("Deleted").green(),
            style(&name).bold(),
        ))?;
    } else {
        term.write_line(&format!(
            "{}",
            style(format!("Connection {name} was not deleted")).yellow()
        ))?;
    }

    Ok(())
}

async fn delete_interactive(
    service: &Nirvana,
    with_tickets: bool,
) -> anyhow::Result<Option<(String, bool)>> {
    let Some(name) = select_connection(service, "Select a connection to delete").await? else {
        return Ok(None);
    };
    let with_tickets = with_tickets
        || Confirm::with_theme(&theme())
            .with_prompt("Also delete its tickets and slots?")
            .default(false)
            .interact()?;
    Ok(Some((name, with_tickets)))
}

pub(crate) async fn test(name: Option<String>, all: bool) -> anyhow::Result<()> {
    let service = Nirvana::new().await?;
    let term = Term::stdout();

    if !all {
        let name = match name {
            Some(name) => name,
            None => match select_connection(&service, "Select a connection to test").await? {
                Some(name) => name,
                None => return Ok(()),
            },
        };
        return Ok(test_one(&service, &term, &name).await?);
    }

    let connections = service.list_connections().await?;
    if connections.is_empty() {
        print_no_connections(&term)?;
        return Ok(());
    }

    let mut failed = 0;
    for connection in &connections {
        let name = &connection.model.name;
        if let Err(e) = test_one(&service, &term, name).await {
            failed += 1;
            term.write_line(&format!(
                "{} {} {:#}",
                style("✗").red(),
                style(name).bold(),
                anyhow::Error::from(e),
            ))?;
        }
    }

    if failed > 0 {
        bail!("{failed} of {} connection tests failed", connections.len());
    }
    Ok(())
}

async fn test_one(service: &Nirvana, term: &Term, name: &str) -> Result<(), NirvanaError> {
    let spinner = ProgressBar::new_spinner().with_message(format!("Testing {name}"));
    spinner.enable_steady_tick(Duration::from_millis(50));
    let started = Instant::now();
    let result = service.test_connection(name).await;
    spinner.finish_and_clear();
    result?;

    term.write_line(&format!(
        "{} {} {}",
        style("✓").green(),
        style(name).bold(),
        style(format!("{} ms", started.elapsed().as_millis())).dim(),
    ))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jira_cloud_credential_splits_into_email_and_token() {
        let (identity, token) =
            split_credential(Kind::JiraCloud, "me@example.com:to:ken".into()).unwrap();

        assert_eq!(identity.as_deref(), Some("me@example.com"));
        assert_eq!(token, "to:ken");
    }

    #[test]
    fn jira_cloud_credential_without_email_is_rejected() {
        let error = split_credential(Kind::JiraCloud, "token".into()).unwrap_err();

        assert_eq!(
            error.to_string(),
            "jira-cloud credential must be \"<email>:<token>\""
        );
    }

    #[test]
    fn credential_without_identity_is_the_token() {
        for kind in [Kind::JiraDc, Kind::Linear] {
            let (identity, token) = split_credential(kind, "to:ken".into()).unwrap();

            assert_eq!(identity, None);
            assert_eq!(token, "to:ken");
        }
    }
}
