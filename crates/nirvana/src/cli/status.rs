use console::{Term, style};
use nirvana_core::Nirvana;

use crate::cli::time::format_duration;

pub(crate) async fn run() -> anyhow::Result<()> {
    let service = Nirvana::new().await?;
    let term = Term::stdout();

    let Some(slot) = service.running_slot().await? else {
        term.write_line(&format!("{}", style("Not tracking").yellow()))?;
        return Ok(());
    };

    term.write_line(&format!(
        "{} ticket {} {} {}",
        style("Tracking").green(),
        style(&slot.ticket.ticket_key).bold(),
        style(slot.ticket.title.as_deref().unwrap_or_default()).dim(),
        format_duration(slot.elapsed()),
    ))?;

    Ok(())
}
