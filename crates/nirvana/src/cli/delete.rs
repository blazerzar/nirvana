use anyhow::bail;
use chrono::Local;
use console::{Term, style};
use dialoguer::Confirm;
use nirvana_core::{Nirvana, Slot, SlotTarget};
use tracing::info;

use crate::cli::time::{format_duration, format_timestamp, parse_time};
use crate::cli::{DeleteArgs, theme};

pub(crate) async fn run(args: DeleteArgs) -> anyhow::Result<()> {
    let target = match (args.id, args.at) {
        (Some(id), _) => SlotTarget::Id(id),
        (None, Some(at)) => SlotTarget::At(parse_time(&at, Local::now().date_naive())?),
        (None, None) => bail!("a slot ID or --at is required"),
    };

    let service = Nirvana::new().await?;
    let term = Term::stdout();
    let (slot, published) = service.get_slot(target).await?;
    let description = describe(&slot);
    let prompt = match published {
        true => format!("Slot {description} is published. Delete it anyway?"),
        false => format!("Are you sure you want to delete slot {description}?"),
    };
    if !args.yes
        && !Confirm::with_theme(&theme())
            .with_prompt(prompt)
            .default(false)
            .interact()?
    {
        term.write_line(&format!(
            "{}",
            style(format!("Slot {description} was not deleted")).yellow()
        ))?;
        return Ok(());
    }

    service.delete_slot(slot.model.id).await?;
    info!(key = %slot.ticket.ticket_key, slot_id = slot.model.id, published, "slot deleted");
    term.write_line(&format!(
        "{} slot {} {} {}",
        style("Deleted").green(),
        style(&slot.ticket.ticket_key).bold(),
        style(slot.ticket.title.as_deref().unwrap_or_default()).dim(),
        format_duration(slot.elapsed()),
    ))?;

    Ok(())
}

/// Describes `slot` by its ticket key and time range, e.g.
/// `DES-1 2026-05-19 14:30 - 2026-05-19 15:45 (1h 15m)`.
fn describe(slot: &Slot) -> String {
    let stop = slot
        .model
        .stopped_at
        .map_or_else(|| "now".to_string(), format_timestamp);
    format!(
        "{} {} - {} ({})",
        slot.ticket.ticket_key,
        format_timestamp(slot.model.started_at),
        stop,
        format_duration(slot.elapsed()),
    )
}
