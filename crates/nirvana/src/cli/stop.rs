use chrono::Local;
use console::{Term, style};
use nirvana_core::{Nirvana, Slot};
use tracing::info;

use crate::cli::StopArgs;
use crate::cli::time::{format_duration, parse_time};

pub(crate) async fn run(args: StopArgs) -> anyhow::Result<()> {
    let service = Nirvana::new().await?;
    let term = Term::stdout();
    let at = args
        .at
        .map(|at| parse_time(&at, Local::now().date_naive()))
        .transpose()?;

    let Some(Slot {
        model: slot,
        ticket,
    }) = service.stop_slot(at).await?
    else {
        term.write_line(&format!("{}", style("Not tracking").yellow()))?;
        return Ok(());
    };
    let stopped_at = slot.stopped_at.unwrap_or_default();
    info!(key = %ticket.ticket_key, slot_id = slot.id, stopped_at, "slot stopped");

    term.write_line(&format!(
        "{} ticket {} {} {}",
        style("Stopped").yellow(),
        style(&ticket.ticket_key).bold(),
        style(ticket.title.unwrap_or_default()).dim(),
        format_duration(stopped_at - slot.started_at),
    ))?;

    Ok(())
}
