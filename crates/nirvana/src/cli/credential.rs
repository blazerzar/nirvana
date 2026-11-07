use console::{Term, style};
use dialoguer::{Password, Select};
use nirvana_core::domain::{Credential, SecretLocation};
use nirvana_core::secrets::{SecretStore, selectable_stores};
use nirvana_core::service::Nirvana;
use secrecy::SecretString;
use tracing::info;

use crate::cli::table::{self, Column};
use crate::cli::theme;

const COLUMNS: [Column; 3] = [
    Column::new("NAME").max_width(30),
    Column::new("STORAGE"),
    Column::new("ACCOUNT"),
];

pub(crate) async fn list() -> anyhow::Result<()> {
    let service = Nirvana::new().await?;
    let credentials = service.list_credentials().await?;
    let term = Term::stdout();
    if credentials.is_empty() {
        print_no_credentials(&term)?;
        return Ok(());
    }

    let rows: Vec<[String; 3]> = credentials
        .into_iter()
        .map(|credential| {
            let store = credential.location.store().to_string();
            let account = match credential.location {
                SecretLocation::Plaintext => String::new(),
                SecretLocation::Keyring { account } => account,
            };
            [credential.connection, store, account]
        })
        .collect();

    table::print(&term, &COLUMNS, &rows)
}

pub(crate) async fn update(name: Option<String>, token: Option<String>) -> anyhow::Result<()> {
    let service = Nirvana::new().await?;

    let Some(credential) =
        select_credential(&service, name, "Select a connection to update").await?
    else {
        return Ok(());
    };
    let name = credential.connection;
    let token = match token {
        Some(token) => token,
        None => Password::with_theme(&theme())
            .with_prompt("New token")
            .interact()?,
    };

    service
        .update_credential(&name, SecretString::from(token))
        .await?;
    info!(%name, "credential updated");

    Term::stdout().write_line(&format!(
        "{} credential of {}",
        style("Updated").green(),
        style(&name).bold(),
    ))?;
    Ok(())
}

pub(crate) async fn move_to(name: Option<String>, to: Option<SecretStore>) -> anyhow::Result<()> {
    let service = Nirvana::new().await?;

    let (name, to) = match (name, to) {
        (Some(name), Some(to)) => (name, to),
        (name, _) => match move_interactive(&service, name).await? {
            Some(choice) => choice,
            None => return Ok(()),
        },
    };

    service.move_credential(&name, to).await?;
    info!(%name, %to, "credential moved");

    Term::stdout().write_line(&format!(
        "{} credential of {} to {}",
        style("Moved").green(),
        style(&name).bold(),
        style(to).bold(),
    ))?;
    Ok(())
}

async fn move_interactive(
    service: &Nirvana,
    name: Option<String>,
) -> anyhow::Result<Option<(String, SecretStore)>> {
    let Some(credential) = select_credential(service, name, "Select a connection to move").await?
    else {
        return Ok(None);
    };
    let current = credential.location.store();

    let keyring_available = service.keyring_status().await.is_ok();
    let stores: Vec<SecretStore> = selectable_stores()
        .iter()
        .copied()
        .filter(|store| *store != current && (keyring_available || *store != SecretStore::Keyring))
        .collect();
    if stores.is_empty() {
        Term::stdout().write_line(&format!("{}", style("No other stores available").yellow()))?;
        return Ok(None);
    }

    let Some(idx) = Select::with_theme(&theme())
        .with_prompt(format!("Move the token from {current} to"))
        .items(&stores)
        .default(0)
        .interact_opt()?
    else {
        Term::stdout().write_line(&format!("{}", style("No secret store selected").yellow()))?;
        return Ok(None);
    };

    Ok(Some((credential.connection, stores[idx])))
}

/// Returns the credential of the connection named `name`, or lets the user
/// select one when `name` is `None`.
async fn select_credential(
    service: &Nirvana,
    name: Option<String>,
    prompt: &str,
) -> anyhow::Result<Option<Credential>> {
    if let Some(name) = name {
        return Ok(Some(service.get_credential(&name).await?));
    }

    let mut credentials = service.list_credentials().await?;

    let term = Term::stdout();
    if credentials.is_empty() {
        print_no_credentials(&term)?;
        return Ok(None);
    }

    let items: Vec<&str> = credentials
        .iter()
        .map(|credential| credential.connection.as_str())
        .collect();
    let idx = Select::with_theme(&theme())
        .with_prompt(prompt)
        .items(&items)
        .default(0)
        .interact_opt()?;

    match idx {
        Some(idx) => Ok(Some(credentials.swap_remove(idx))),
        None => {
            term.write_line(&format!("{}", style("No connection selected").yellow()))?;
            Ok(None)
        }
    }
}

fn print_no_credentials(term: &Term) -> std::io::Result<()> {
    term.write_line(&format!("{}", style("No credentials found").yellow()))?;
    term.write_line(&format!(
        "Credentials are added with connections: {}",
        style("nirvana connection add").bold()
    ))
}
