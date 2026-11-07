mod connection;
mod credential;
mod delete;
mod edit;
mod info;
mod list;
mod profile;
mod publish;
mod start;
mod stop;
mod table;
mod time;

use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::{Args, Parser, Subcommand};
use console::Style;
use dialoguer::theme::ColorfulTheme;
use nirvana_core::domain::Kind;
use nirvana_core::secrets::{SecretStore, selectable_stores};
use tracing::debug;

#[derive(Parser)]
#[command(
    name = "nirvana",
    version,
    arg_required_else_help = true,
    about = "App that gets you to time tracking nirvana"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Show system and app diagnostics
    Info,
    /// Start tracking time on a ticket
    Start(StartArgs),
    /// Stop tracking time on active ticket
    Stop(StopArgs),
    /// Edit an existing slot
    Edit(EditArgs),
    /// Delete a slot
    Delete(DeleteArgs),
    /// List slots
    List(ListArgs),
    /// Publish slots to backend
    Publish(PublishArgs),
    /// Manage profiles
    Profile {
        #[command(subcommand)]
        command: Profile,
    },
    /// Manage connections
    Connection {
        #[command(subcommand)]
        command: Connection,
    },
    /// Manage credentials
    Credential {
        #[command(subcommand)]
        command: Credential,
    },
}

#[derive(Args, Debug)]
struct StartArgs {
    /// Ticket key (e.g. DES-1234). Omit for interactive mode.
    ticket: Option<String>,
    /// Start time (e.g. "14:30" or "2026-05-19 14:30"). Requires ticket.
    #[arg(long, requires = "ticket")]
    at: Option<String>,
}

#[derive(Args, Debug)]
struct StopArgs {
    /// Stop time (e.g. "14:30" or "2026-05-19 14:30")
    #[arg(long)]
    at: Option<String>,
}

#[derive(Args, Debug)]
struct EditArgs {
    /// Slot ID to edit
    slot_id: i64,
    /// Move the slot to a different ticket key
    #[arg(long)]
    ticket: Option<String>,
    /// Set the start time (e.g. "14:30" or "2026-05-19 14:30")
    #[arg(long)]
    start: Option<String>,
    /// Set or clear the stop time (use empty string to clear, reopening the slot)
    #[arg(long)]
    stop: Option<String>,
}

#[derive(Args, Debug)]
struct DeleteArgs {
    /// Slot ID to delete
    slot_id: i64,
}

#[derive(Args, Debug)]
struct ListArgs {
    /// Order by ticket instead of time
    #[arg(long)]
    by_ticket: bool,
    /// Range start for overlapping slots (e.g. "14:30" or "2026-05-19 14:30"). Defaults to today 00:00.
    #[arg(long)]
    start: Option<String>,
    /// Range stop for overlapping slots (e.g. "14:30" or "2026-05-19 14:30"). Unbounded if omitted.
    #[arg(long)]
    stop: Option<String>,
}

#[derive(Args, Debug)]
struct PublishArgs {
    /// Range start (e.g. "14:30" or "2026-05-19 14:30"). Defaults to today 00:00.
    #[arg(long)]
    start: Option<String>,
    /// Range stop (e.g. "14:30" or "2026-05-19 14:30"). Unbounded if omitted.
    #[arg(long)]
    stop: Option<String>,
}

#[derive(Subcommand)]
enum Profile {
    /// List all profiles
    List,
    /// Create a new profile
    Create {
        /// Profile name (omit for interactive creation)
        name: Option<String>,
        /// Switch to the new profile
        #[arg(short, long)]
        switch: bool,
    },
    /// Switch the active profile
    Switch {
        /// Profile name (omit for interactive selection)
        name: Option<String>,
    },
    /// Rename a profile
    Rename {
        /// Current profile name (omit for interactive selection)
        name: Option<String>,
        /// New profile name
        new_name: Option<String>,
    },
    /// Delete a profile with its connections, tickets and slots
    Delete {
        /// Profile name (omit for interactive selection)
        name: Option<String>,
        /// Delete without asking
        #[arg(short, long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum Connection {
    /// List all connections
    List,
    /// Add a new connection
    Add {
        /// Connection name (omit for interactive creation)
        #[arg(requires_all = ["kind", "host", "storage"])]
        name: Option<String>,
        /// Connection kind
        #[arg(long, value_parser = kind_parser(), requires = "name")]
        kind: Option<Kind>,
        /// Host URL
        #[arg(long, requires = "name")]
        host: Option<String>,
        /// Token, or "<email>:<token>" for jira-cloud (omit to enter it interactively)
        #[arg(long, requires = "name")]
        credential: Option<String>,
        /// Where to store the token
        #[arg(long, value_parser = storage_parser(), requires = "name")]
        storage: Option<SecretStore>,
    },
    /// Edit a connection's name, host or identity
    Edit {
        /// Connection name (omit for interactive selection)
        name: Option<String>,
        /// New connection name
        #[arg(long = "name", value_name = "NAME", requires = "name")]
        new_name: Option<String>,
        /// New host URL
        #[arg(long, requires = "name")]
        host: Option<String>,
        /// New identity (jira-cloud email)
        #[arg(long, requires = "name")]
        identity: Option<String>,
    },
    /// Delete a connection
    Delete {
        /// Connection name (omit for interactive selection)
        name: Option<String>,
        /// Also delete the connection's tickets and their slots
        #[arg(long)]
        with_tickets: bool,
        /// Delete without asking
        #[arg(short, long)]
        yes: bool,
    },
    /// Test a connection
    Test {
        /// Connection name (omit for interactive selection)
        name: Option<String>,
        /// Test all connections
        #[arg(short, long, conflicts_with = "name")]
        all: bool,
    },
}

#[derive(Subcommand)]
enum Credential {
    /// List credentials for all connections
    List,
    /// Update credential secret
    Update {
        /// Connection name (omit for interactive selection)
        name: Option<String>,
        /// Token (omit to enter it interactively)
        #[arg(long, requires = "name")]
        token: Option<String>,
    },
    /// Move credential secret to a different store
    Move {
        /// Connection name (omit for interactive selection)
        name: Option<String>,
        /// Secret store to move the token to
        #[arg(long, value_parser = storage_parser(), requires = "name")]
        to: Option<SecretStore>,
    },
}

fn kind_parser() -> impl TypedValueParser<Value = Kind> {
    PossibleValuesParser::new(Kind::ALL.map(|kind| PossibleValue::new(kind.as_str())))
        .try_map(|s| s.parse::<Kind>())
}

fn storage_parser() -> impl TypedValueParser<Value = SecretStore> {
    let hidden = PossibleValue::new(SecretStore::Env.to_string()).hide(true);
    PossibleValuesParser::new(
        selectable_stores()
            .iter()
            .map(|store| PossibleValue::new(store.to_string()))
            .chain([hidden]),
    )
    .try_map(|s| s.parse::<SecretStore>())
}

pub(crate) fn theme() -> ColorfulTheme {
    ColorfulTheme {
        active_item_prefix: Style::new().green().apply_to(String::from("❯")),
        ..ColorfulTheme::default()
    }
}

pub(crate) async fn run() -> anyhow::Result<()> {
    let args = mask_secrets(
        std::env::args_os()
            .skip(1)
            .map(|arg| arg.to_string_lossy().into_owned()),
    );
    debug!(?args, "command ran");

    let cli = Cli::parse();

    match cli.command {
        Some(Command::Info) => info::run().await,
        Some(Command::Start(args)) => start::run(args).await,
        Some(Command::Stop(args)) => stop::run(args).await,
        Some(Command::Edit(args)) => edit::run(args).await,
        Some(Command::Delete(args)) => delete::run(args).await,
        Some(Command::List(args)) => list::run(args).await,
        Some(Command::Publish(args)) => publish::run(args).await,
        Some(Command::Profile { command }) => match command {
            Profile::List => profile::list().await,
            Profile::Create { name, switch } => profile::create(name, switch).await,
            Profile::Switch { name } => profile::switch(name).await,
            Profile::Rename { name, new_name } => profile::rename(name, new_name).await,
            Profile::Delete { name, yes } => profile::delete(name, yes).await,
        },
        Some(Command::Connection { command }) => match command {
            Connection::List => connection::list().await,
            Connection::Add {
                name,
                kind,
                host,
                credential,
                storage,
            } => connection::add(name, kind, host, credential, storage).await,
            Connection::Edit {
                name,
                new_name,
                host,
                identity,
            } => connection::edit(name, new_name, host, identity).await,
            Connection::Delete {
                name,
                with_tickets,
                yes,
            } => connection::delete(name, with_tickets, yes).await,
            Connection::Test { name, all } => connection::test(name, all).await,
        },
        Some(Command::Credential { command }) => match command {
            Credential::List => credential::list().await,
            Credential::Update { name, token } => credential::update(name, token).await,
            Credential::Move { name, to } => credential::move_to(name, to).await,
        },
        None => Ok(()),
    }
}

const SECRET_FLAGS: [&str; 2] = ["--credential", "--token"];
const MASK: &str = "***";

/// Replaces the values of [`SECRET_FLAGS`], given as `--flag value` or
/// `--flag=value`, with [`MASK`].
fn mask_secrets(args: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut mask_next = false;
    args.into_iter()
        .map(|arg| {
            if std::mem::take(&mut mask_next) {
                return MASK.to_string();
            }
            if let Some((flag, _)) = arg.split_once('=')
                && SECRET_FLAGS.contains(&flag)
            {
                return format!("{flag}={MASK}");
            }
            mask_next = SECRET_FLAGS.contains(&arg.as_str());
            arg
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn mask_secrets_masks_separate_values() {
        assert_eq!(
            mask_secrets(args(&[
                "connection",
                "add",
                "jira",
                "--credential",
                "me@example.com:secret",
                "--storage",
                "keyring",
            ])),
            args(&[
                "connection",
                "add",
                "jira",
                "--credential",
                MASK,
                "--storage",
                "keyring",
            ])
        );
    }

    #[test]
    fn mask_secrets_masks_inline_values() {
        assert_eq!(
            mask_secrets(args(&["credential", "update", "jira", "--token=secret"])),
            args(&["credential", "update", "jira", "--token=***"])
        );
    }

    #[test]
    fn mask_secrets_keeps_other_args() {
        let other = args(&["connection", "edit", "jira", "--host=example.com"]);
        assert_eq!(mask_secrets(other.clone()), other);
    }
}
