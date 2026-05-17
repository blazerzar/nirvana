mod cli;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "nirvana",
    version,
    about = "App to get you to time-tracking nirvana"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Show system and app diagnostics
    Info,
    Connection {
        #[command(subcommand)]
        command: Connection,
    },
}

#[derive(Subcommand)]
enum Connection {
    /// List all connections
    List,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Info) => cli::info::run(),
        Some(Command::Connection { command }) => match command {
            Connection::List => cli::connection::list(),
        },
        None => Ok(()),
    }
}
