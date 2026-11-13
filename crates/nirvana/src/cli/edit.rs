use console::{Term, style};
use nirvana_core::{Change, Nirvana};
use tracing::info;

use crate::cli::time::{format_duration, local_date, parse_duration, parse_time};
use crate::cli::{EditArgs, slot_target};

pub(crate) async fn run(args: EditArgs) -> anyhow::Result<()> {
    let target = slot_target(args.id, args.at.as_deref())?;
    let service = Nirvana::new().await?;
    let (slot, _) = service.get_slot(target).await?;

    let started_at = match args.start {
        Some(start) => Some(parse_time(&start, local_date(slot.model.started_at)?)?),
        None => None,
    };
    let start = started_at.unwrap_or(slot.model.started_at);
    let stopped_at = match (args.stop.as_deref(), args.duration) {
        (Some(""), _) => Change::Clear,
        (Some(stop), _) => {
            let stop_date = local_date(slot.model.stopped_at.unwrap_or(start))?;
            Change::Set(parse_time(stop, stop_date)?)
        }
        (None, Some(duration)) => Change::Set(start + parse_duration(&duration)?),
        (None, None) => Change::Skip,
    };

    let slot = service
        .edit_slot(
            slot.model.id,
            args.ticket.as_deref(),
            args.connection.as_deref(),
            started_at,
            stopped_at,
        )
        .await?;
    info!(
        key = %slot.ticket.ticket_key,
        slot_id = slot.model.id,
        started_at = slot.model.started_at,
        stopped_at = slot.model.stopped_at,
        "slot edited"
    );

    Term::stdout().write_line(&format!(
        "{} slot {} {} {}",
        style("Edited").green(),
        style(&slot.ticket.ticket_key).bold(),
        style(slot.ticket.title.as_deref().unwrap_or_default()).dim(),
        format_duration(slot.elapsed()),
    ))?;

    Ok(())
}
