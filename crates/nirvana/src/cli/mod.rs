mod config;
mod connection;
mod credential;
mod delete;
mod edit;
mod info;
mod list;
mod logs;
mod profile;
mod publish;
mod resume;
mod settings;
mod start;
mod stop;
mod table;
mod ticket;
mod time;

use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::{Args, Parser, Subcommand};
use console::Style;
use dialoguer::theme::ColorfulTheme;
use nirvana_core::{Kind, SecretStore, TicketSort, selectable_stores};
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
    /// Resume tracking time on the previous ticket
    Resume(ResumeArgs),
    /// Edit an existing slot
    Edit(EditArgs),
    /// Delete a slot
    Delete(DeleteArgs),
    /// List slots
    List(ListArgs),
    /// Publish slots to backend
    Publish(PublishArgs),
    /// Manage tickets
    Ticket {
        #[command(subcommand)]
        command: Ticket,
    },
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
    /// View and clear logs
    Logs {
        #[command(subcommand)]
        command: Logs,
    },
    /// View and change settings
    Settings {
        #[command(subcommand)]
        command: Settings,
    },
    /// View and change config
    Config {
        #[command(subcommand)]
        command: Config,
    },
}

#[derive(Args, Debug)]
struct StartArgs {
    /// Ticket key (e.g. DES-1234). Omit for interactive mode.
    ticket: Option<String>,
    /// Start time (e.g. "14:30" or "2026-05-19 14:30"). Requires ticket.
    #[arg(long, requires = "ticket")]
    at: Option<String>,
    /// Use the ticket of this connection (omit to search all connections)
    #[arg(long, requires = "ticket")]
    connection: Option<String>,
}

#[derive(Args, Debug)]
struct StopArgs {
    /// Stop time (e.g. "14:30" or "2026-05-19 14:30")
    #[arg(long)]
    at: Option<String>,
}

#[derive(Args, Debug)]
struct ResumeArgs {
    /// Start time (e.g. "14:30" or "2026-05-19 14:30")
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
    #[arg(long, alias = "start")]
    from: Option<String>,
    /// Range end for overlapping slots (e.g. "14:30" or "2026-05-19 14:30"). Unbounded if omitted.
    #[arg(long, alias = "stop", requires = "from")]
    to: Option<String>,
    /// Show slot IDs
    #[arg(long)]
    show_ids: bool,
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

#[derive(Subcommand)]
enum Ticket {
    /// List tickets
    List {
        /// Show only tickets of this connection
        #[arg(long)]
        connection: Option<String>,
        /// Show only tickets without a connection
        #[arg(long, conflicts_with = "connection")]
        no_connection: bool,
        /// Column to sort by
        #[arg(long, value_parser = ticket_sort_parser(), default_value = TicketSort::default().as_str())]
        sort: TicketSort,
        /// Show ticket IDs
        #[arg(long)]
        show_ids: bool,
    },
    /// Fetch a ticket with its title from your connections
    Load {
        /// Ticket key (e.g. DES-1234)
        key: String,
        /// Fetch only from this connection (omit to search all connections)
        #[arg(long)]
        connection: Option<String>,
    },
    /// Remove a ticket
    Remove {
        /// Ticket key (e.g. DES-1234)
        #[arg(required_unless_present = "id")]
        key: Option<String>,
        /// Remove only the ticket of this connection
        #[arg(long)]
        connection: Option<String>,
        /// Ticket ID, shown by `ticket list --show-ids`
        #[arg(long, conflicts_with_all = ["key", "connection"])]
        id: Option<i64>,
        /// Also remove the ticket's slots without asking
        #[arg(long)]
        with_slots: bool,
    },
    /// Update a ticket's title from its connection
    Sync {
        /// Ticket key (e.g. DES-1234)
        key: String,
        /// Sync only the ticket of this connection
        #[arg(long)]
        connection: Option<String>,
    },
}

#[derive(Subcommand)]
enum Logs {
    /// Show the log file
    View {
        /// Show only the last N lines
        #[arg(short = 'n', long)]
        lines: Option<usize>,
        /// Keep printing new lines as they are written
        #[arg(short, long)]
        follow: bool,
    },
    /// Clear the log file
    Clear {
        /// Clear without asking
        #[arg(short, long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum Settings {
    /// Show connection settings
    Get {
        /// Connection name
        #[arg(long)]
        connection: String,
        /// Setting to show (omit to show all)
        key: Option<String>,
    },
    /// Change a connection setting
    Set {
        /// Connection name
        #[arg(long)]
        connection: String,
        /// Setting to change
        key: String,
        /// New value
        value: String,
    },
}

#[derive(Subcommand)]
enum Config {
    /// Show config values
    Get {
        /// Section or key (section.key) to show (omit to show all)
        key: Option<String>,
        /// Also show keys that are not set
        #[arg(long)]
        show_unset: bool,
        /// Show the default instead of the value of a single key
        #[arg(long)]
        show_default: bool,
    },
    /// Change a config value
    Set {
        /// Key to change (section.key)
        key: String,
        /// New value
        value: String,
    },
    /// Reset a config value to its default
    Unset {
        /// Key to reset (section.key)
        key: String,
    },
}

fn kind_parser() -> impl TypedValueParser<Value = Kind> {
    PossibleValuesParser::new(Kind::ALL.map(|kind| PossibleValue::new(kind.as_str())))
        .try_map(|s| s.parse::<Kind>())
}

fn ticket_sort_parser() -> impl TypedValueParser<Value = TicketSort> {
    PossibleValuesParser::new(TicketSort::ALL.map(|sort| PossibleValue::new(sort.as_str())))
        .try_map(|s| s.parse::<TicketSort>())
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
        Some(Command::Resume(args)) => resume::run(args).await,
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
        Some(Command::Ticket { command }) => match command {
            Ticket::List {
                connection,
                no_connection,
                sort,
                show_ids,
            } => ticket::list(connection, no_connection, sort, show_ids).await,
            Ticket::Load { key, connection } => ticket::load(key, connection).await,
            Ticket::Remove {
                key,
                connection,
                id,
                with_slots,
            } => ticket::remove(key, connection, id, with_slots).await,
            Ticket::Sync { key, connection } => ticket::sync(key, connection).await,
        },
        Some(Command::Credential { command }) => match command {
            Credential::List => credential::list().await,
            Credential::Update { name, token } => credential::update(name, token).await,
            Credential::Move { name, to } => credential::move_to(name, to).await,
        },
        Some(Command::Logs { command }) => match command {
            Logs::View { lines, follow } => logs::view(lines, follow).await,
            Logs::Clear { yes } => logs::clear(yes).await,
        },
        Some(Command::Settings { command }) => match command {
            Settings::Get { connection, key } => settings::get(connection, key).await,
            Settings::Set {
                connection,
                key,
                value,
            } => settings::set(connection, key, value).await,
        },
        Some(Command::Config { command }) => match command {
            Config::Get {
                key,
                show_unset,
                show_default,
            } => config::get(key, show_unset, show_default).await,
            Config::Set { key, value } => config::set(key, value).await,
            Config::Unset { key } => config::unset(key).await,
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
