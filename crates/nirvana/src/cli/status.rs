use chrono::Local;
use console::{Term, style};
use nirvana_core::{Nirvana, Slot};

use crate::cli::time::format_duration;

pub(crate) async fn run() -> anyhow::Result<()> {
    let service = Nirvana::new().await?;
    let term = Term::stdout();

    let Some(Slot {
        model: slot,
        ticket,
    }) = service.running_slot().await?
    else {
        term.write_line(&format!("{}", style("Not tracking").yellow()))?;
        return Ok(());
    };

    term.write_line(&format!(
        "{} ticket {} {} {}",
        style("Tracking").green(),
        style(&ticket.ticket_key).bold(),
        style(ticket.title.unwrap_or_default()).dim(),
        format_duration(Local::now().timestamp() - slot.started_at),
    ))?;

    Ok(())
}
