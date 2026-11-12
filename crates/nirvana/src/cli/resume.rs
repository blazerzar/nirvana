use chrono::Local;
use console::{Term, style};
use nirvana_core::{Nirvana, Slot};
use tracing::info;

use crate::cli::ResumeArgs;
use crate::cli::time::parse_time;

pub(crate) async fn run(args: ResumeArgs) -> anyhow::Result<()> {
    let service = Nirvana::new().await?;
    let term = Term::stdout();
    let at = args
        .at
        .map(|at| parse_time(&at, Local::now().date_naive()))
        .transpose()?;

    let Slot {
        model: slot,
        ticket,
    } = service.resume_slot(at).await?;
    info!(key = %ticket.ticket_key, slot_id = slot.id, started_at = slot.started_at, "slot resumed");

    term.write_line(&format!(
        "{} ticket {} {}",
        style("Resumed").green(),
        style(&ticket.ticket_key).bold(),
        style(ticket.title.unwrap_or_default()).dim(),
    ))?;

    Ok(())
}
