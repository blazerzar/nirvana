use chrono::Local;
use console::Term;
use nirvana_core::Nirvana;
use tracing::info;

use crate::cli::ResumeArgs;
use crate::cli::start::write_started;
use crate::cli::time::parse_time;

pub(crate) async fn run(args: ResumeArgs) -> anyhow::Result<()> {
    let service = Nirvana::new().await?;
    let at = args
        .at
        .map(|at| parse_time(&at, Local::now().date_naive()))
        .transpose()?;

    let slot = service.resume_slot(at).await?;
    info!(key = %slot.ticket.ticket_key, slot_id = slot.model.id, started_at = slot.model.started_at, "slot resumed");

    write_started(&Term::stdout(), "Resumed", slot)
}
