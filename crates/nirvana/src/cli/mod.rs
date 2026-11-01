mod connection;
mod delete;
mod edit;
mod info;
mod list;
mod profile;
mod publish;
mod start;
mod stop;
mod table;

use std::fmt;
use std::fmt::Display;

use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::{Args, Parser, Subcommand, ValueEnum};
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
    /// Switch profile
    Switch {
        /// Profile name (omit for interactive selection)
        name: Option<String>,
    },
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
    /// Delete a profile
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
    /// Add a new connection
    Add(AddArgs),
    /// List all connections
    List,
    /// Change active connection
    Use {
        /// Connection ID or name (omit for interactive selection)
        query: Option<String>,
    },
    /// Test active connection
    Test,
}

#[derive(Args, Debug)]
struct AddArgs {
    #[arg(long, requires_all = ["kind", "host", "identity", "storage", "token"])]
    name: Option<String>,
    #[arg(long, requires_all = ["name", "host", "identity", "storage", "token"])]
    kind: Option<ConnectionKind>,
    #[arg(long, requires_all = ["name", "kind", "identity", "storage", "token"])]
    host: Option<String>,
    #[arg(long, requires_all = ["name", "kind", "host", "storage", "token"])]
    identity: Option<String>,
    #[arg(long, value_parser = storage_parser(), requires_all = ["name", "kind", "host", "identity", "token"])]
    storage: Option<SecretStore>,
    #[arg(long, requires_all = ["name", "kind", "host", "identity", "storage"])]
    token: Option<String>,
}

#[derive(Clone, ValueEnum, Debug)]
pub(crate) enum ConnectionKind {
    JiraCloud,
    JiraDc,
}

impl Display for ConnectionKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let kind = match self {
            Self::JiraCloud => "jira-cloud",
            Self::JiraDc => "jira-dc",
        };
        write!(f, "{}", kind,)
    }
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

pub(crate) async fn run() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
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
            Profile::Switch { name } => profile::switch(name).await,
            Profile::List => profile::list().await,
            Profile::Create { name, switch } => profile::create(name, switch).await,
            Profile::Delete { name, yes } => profile::delete(name, yes).await,
        },
        Some(Command::Connection { command }) => match command {
            Connection::Add(args) => connection::add(args).await,
            Connection::List => connection::list().await,
            Connection::Use { query } => connection::activate(query.as_deref()).await,
            Connection::Test => connection::test().await,
        },
        None => Ok(()),
    }
}
