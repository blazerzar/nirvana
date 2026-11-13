use anyhow::{Context, bail};
use chrono::{DateTime, Local};
use console::{Term, style};
use nirvana_core::Nirvana;
use tracing::info;

use crate::cli::AddArgs;
use crate::cli::time::{format_duration, parse_duration, parse_time};

pub(crate) async fn run(args: AddArgs) -> anyhow::Result<()> {
    let started_at = parse_time(&args.start, Local::now().date_naive())?;
    let stopped_at = match (args.stop, args.duration) {
        (Some(stop), _) => {
            let start_date = DateTime::from_timestamp(started_at, 0)
                .context("start time is out of range")?
                .with_timezone(&Local)
                .date_naive();
            parse_time(&stop, start_date)?
        }
        (None, Some(duration)) => started_at + parse_duration(&duration)?,
        (None, None) => bail!("--stop or --duration is required"),
    };

    let service = Nirvana::new().await?;
    let slot = service
        .add_slot(
            &args.ticket,
            started_at,
            stopped_at,
            args.connection.as_deref(),
        )
        .await?;
    info!(key = %slot.ticket.ticket_key, slot_id = slot.model.id, started_at, stopped_at, "slot added");

    Term::stdout().write_line(&format!(
        "{} slot {} {} {}",
        style("Added").green(),
        style(&slot.ticket.ticket_key).bold(),
        style(slot.ticket.title.as_deref().unwrap_or_default()).dim(),
        format_duration(slot.elapsed()),
    ))?;

    Ok(())
}
