use anyhow::bail;
use console::{Alignment, Term, style};
use dialoguer::Confirm;
use nirvana_core::{
    DomainError, Nirvana, NirvanaError, TicketConnection, TicketSort, TicketTarget,
};
use tracing::info;

use crate::cli::table::{self, Column};
use crate::cli::theme;
use crate::cli::time::format_timestamp;

const COLUMNS: [Column; 5] = [
    Column::new("KEY"),
    Column::new("TITLE").max_width(50),
    Column::new("CONNECTION").max_width(20),
    Column::new("LAST WORKED"),
    Column::new("SYNCED"),
];

const COLUMNS_WITH_ID: [Column; 6] = [
    Column::new("ID").align(Alignment::Right),
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
    show_ids: bool,
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

    let rows: Vec<(String, [String; 5])> = tickets
        .into_iter()
        .map(|(ticket, connection)| {
            let row = [
                ticket.ticket_key,
                ticket.title.unwrap_or_default(),
                connection.map_or_else(|| "".to_string(), |c| c.name),
                Some(ticket.last_worked_at)
                    .filter(|&timestamp| timestamp != 0)
                    .map(format_timestamp)
                    .unwrap_or_default(),
                format_timestamp(ticket.synced_at),
            ];
            (ticket.id.to_string(), row)
        })
        .collect();

    if show_ids {
        let rows: Vec<[String; 6]> = rows
            .into_iter()
            .map(|(id, [key, title, connection, last_worked, synced])| {
                [id, key, title, connection, last_worked, synced]
            })
            .collect();
        table::print(&term, &COLUMNS_WITH_ID, &rows)
    } else {
        let rows: Vec<[String; 5]> = rows.into_iter().map(|(_, row)| row).collect();
        table::print(&term, &COLUMNS, &rows)
    }
}

pub(crate) async fn load(key: String, connection: Option<String>) -> anyhow::Result<()> {
    let service = Nirvana::new().await?;
    let ticket = service.load_ticket(&key, connection.as_deref()).await?;
    info!(key = %ticket.ticket_key, connection_id = ?ticket.connection_id, "ticket loaded");

    Term::stdout().write_line(&format!(
        "{} ticket {} {}",
        style("Loaded").green(),
        style(&ticket.ticket_key).bold(),
        ticket.title.unwrap_or_default(),
    ))?;

    Ok(())
}

pub(crate) async fn remove(
    key: Option<String>,
    connection: Option<String>,
    id: Option<i64>,
    with_slots: bool,
) -> anyhow::Result<()> {
    let target = match (key, id) {
        (_, Some(id)) => TicketTarget::Id(id),
        (Some(key), None) => TicketTarget::Key { key, connection },
        (None, None) => bail!("a ticket key or --id is required"),
    };

    let service = Nirvana::new().await?;
    let term = Term::stdout();
    let ticket = match service.remove_ticket(target.clone(), with_slots).await {
        Err(NirvanaError::Domain(DomainError::TicketHasSlots(key, slots))) => {
            let slots = if slots == 1 {
                "1 slot".to_string()
            } else {
                format!("{slots} slots")
            };
            if !Confirm::with_theme(&theme())
                .with_prompt(format!(
                    "Ticket {key} has {slots}. Remove it with its slots?"
                ))
                .default(false)
                .interact()?
            {
                term.write_line(&format!(
                    "{}",
                    style(format!("Ticket {key} was not removed")).yellow()
                ))?;
                return Ok(());
            }
            service.remove_ticket(target, true).await?
        }
        result => result?,
    };
    info!(key = %ticket.ticket_key, id = ticket.id, "ticket removed");
    term.write_line(&format!(
        "{} ticket {}",
        style("Removed").green(),
        style(&ticket.ticket_key).bold(),
    ))?;

    Ok(())
}

fn print_no_tickets(term: &Term) -> std::io::Result<()> {
    term.write_line(&format!("{}", style("No tickets found").yellow()))?;
    term.write_line(&format!(
        "Add one with: {}",
        style("nirvana ticket load").bold()
    ))
}
