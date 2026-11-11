use std::collections::HashMap;

use chrono::Local;
use console::{Term, style};
use dialoguer::FuzzySelect;
use nirvana_core::{Nirvana, Slot, TicketConnection, TicketSort};
use tracing::info;

use crate::cli::time::{format_duration, parse_time};
use crate::cli::{StartArgs, theme};

pub(crate) async fn run(args: StartArgs) -> anyhow::Result<()> {
    let service = Nirvana::new().await?;
    let term = Term::stdout();
    let (key, connection) = match args.ticket {
        Some(key) => (key, args.connection),
        None => match select_ticket(&service, &term).await? {
            Some(selected) => selected,
            None => return Ok(()),
        },
    };
    let at = args
        .at
        .map(|at| parse_time(&at, Local::now().date_naive()))
        .transpose()?;

    let result = service.start_slot(&key, at, connection.as_deref()).await?;
    let Slot {
        model: slot,
        ticket,
    } = result.started;
    info!(key = %ticket.ticket_key, slot_id = slot.id, started_at = slot.started_at, "slot started");

    if let Some(Slot {
        model: stopped,
        ticket: stopped_ticket,
    }) = result.stopped
    {
        term.write_line(&format!(
            "{} ticket {} {} {}",
            style("Stopped").yellow(),
            style(&stopped_ticket.ticket_key).bold(),
            style(stopped_ticket.title.unwrap_or_default()).dim(),
            format_duration(stopped.stopped_at.unwrap_or_default() - stopped.started_at),
        ))?;
    }
    term.write_line(&format!(
        "{} ticket {} {}",
        style("Started").green(),
        style(&ticket.ticket_key).bold(),
        style(ticket.title.unwrap_or_default()).dim(),
    ))?;

    Ok(())
}

/// Lets the user select one of the recently worked on tickets with a
/// connection and returns its key and connection name.
async fn select_ticket(
    service: &Nirvana,
    term: &Term,
) -> anyhow::Result<Option<(String, Option<String>)>> {
    let mut tickets = service
        .list_tickets(TicketConnection::Connected, TicketSort::LastWorked)
        .await?;
    if tickets.is_empty() {
        term.write_line(&format!("{}", style("No tickets found").yellow()))?;
        term.write_line(&format!(
            "Start one with: {}",
            style("nirvana start <KEY>").bold()
        ))?;
        return Ok(None);
    }

    let mut key_counts: HashMap<&str, usize> = HashMap::new();
    for (ticket, _) in &tickets {
        *key_counts.entry(&ticket.ticket_key).or_default() += 1;
    }
    let items: Vec<String> = tickets
        .iter()
        .map(|(ticket, connection)| {
            let title = ticket.title.as_deref().unwrap_or_default();
            match connection {
                Some(connection) if key_counts[ticket.ticket_key.as_str()] > 1 => {
                    format!("{} {title} ({})", ticket.ticket_key, connection.name)
                }
                _ => format!("{} {title}", ticket.ticket_key),
            }
        })
        .collect();
    let running_ticket_id = service.running_slot().await?.map(|slot| slot.ticket.id);
    let default = tickets
        .iter()
        .position(|(ticket, _)| Some(ticket.id) != running_ticket_id)
        .unwrap_or(0);
    let Some(idx) = FuzzySelect::with_theme(&theme())
        .with_prompt("Start ticket")
        .items(&items)
        .default(default)
        .interact_opt()?
    else {
        term.write_line(&format!("{}", style("No ticket selected").yellow()))?;
        return Ok(None);
    };

    let (ticket, connection) = tickets.swap_remove(idx);
    Ok(Some((ticket.ticket_key, connection.map(|c| c.name))))
}
