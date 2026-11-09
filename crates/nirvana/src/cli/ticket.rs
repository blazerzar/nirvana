use console::{Term, style};
use nirvana_core::{Nirvana, TicketConnection, TicketSort};

use crate::cli::table::{self, Column};
use crate::cli::time::format_timestamp;

const COLUMNS: [Column; 5] = [
    Column::new("KEY"),
    Column::new("TITLE").max_width(50),
    Column::new("CONNECTION").max_width(20),
    Column::new("LAST WORKED"),
    Column::new("SYNCED"),
];

pub(crate) async fn list(
    connection: Option<String>,
    no_connection: bool,
    sort: TicketSort,
) -> anyhow::Result<()> {
    let connection = match connection {
        Some(name) => TicketConnection::Named(name),
        None if no_connection => TicketConnection::Disconnected,
        None => TicketConnection::Any,
    };
    let service = Nirvana::new().await?;
    let tickets = service.list_tickets(connection, sort).await?;
    let term = Term::stdout();
    if tickets.is_empty() {
        print_no_tickets(&term)?;
        return Ok(());
    }

    let rows: Vec<[String; 5]> = tickets
        .into_iter()
        .map(|(ticket, connection)| {
            [
                ticket.ticket_key,
                ticket.title.unwrap_or_default(),
                connection.map_or_else(|| "".to_string(), |c| c.name),
                Some(ticket.last_worked_at)
                    .filter(|&timestamp| timestamp != 0)
                    .map(format_timestamp)
                    .unwrap_or_default(),
                format_timestamp(ticket.synced_at),
            ]
        })
        .collect();

    table::print(&term, &COLUMNS, &rows)
}

fn print_no_tickets(term: &Term) -> std::io::Result<()> {
    term.write_line(&format!("{}", style("No tickets found").yellow()))?;
    term.write_line(&format!(
        "Add one with: {}",
        style("nirvana ticket load").bold()
    ))
}
