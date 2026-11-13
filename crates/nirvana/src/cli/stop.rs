use chrono::Local;
use console::{Term, style};
use nirvana_core::Nirvana;
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

    let Some(slot) = service.stop_slot(at).await? else {
        term.write_line(&format!("{}", style("Not tracking").yellow()))?;
        return Ok(());
    };
    info!(key = %slot.ticket.ticket_key, slot_id = slot.model.id, stopped_at = slot.model.stopped_at, "slot stopped");

    term.write_line(&format!(
        "{} ticket {} {} {}",
        style("Stopped").yellow(),
        style(&slot.ticket.ticket_key).bold(),
        style(slot.ticket.title.as_deref().unwrap_or_default()).dim(),
        format_duration(slot.elapsed()),
    ))?;

    Ok(())
}
