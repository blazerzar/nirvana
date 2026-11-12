use chrono::Local;
use console::Term;
use nirvana_core::Nirvana;
use tracing::info;

use crate::cli::SwitchArgs;
use crate::cli::start::write_start_result;
use crate::cli::time::parse_time;

pub(crate) async fn run(args: SwitchArgs) -> anyhow::Result<()> {
    let service = Nirvana::new().await?;
    let at = args
        .at
        .map(|at| parse_time(&at, Local::now().date_naive()))
        .transpose()?;

    let result = service.switch_slot(at).await?;
    let slot = &result.started;
    info!(key = %slot.ticket.ticket_key, slot_id = slot.model.id, started_at = slot.model.started_at, "slot switched");

    write_start_result(&Term::stdout(), result)
}
