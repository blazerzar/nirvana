use console::style;
use nirvana_core::domain::{Change, SlotEdit};
use nirvana_core::service::Nirvana;

pub(crate) struct EditArgs {
    pub slot_id: i64,
    pub ticket_key: Option<String>,
    pub note: Change<String>,
    pub started_at: Option<i64>,
    pub stopped_at: Change<i64>,
}

pub(crate) async fn run(args: EditArgs) -> anyhow::Result<()> {
    let api = Nirvana::new()?;

    let edit = SlotEdit {
        ticket_key: args.ticket_key,
        note: args.note,
        started_at: args.started_at,
        stopped_at: args.stopped_at,
    };

    let slot = api.edit_slot(args.slot_id, edit).await?;

    println!(
        "{} {} slot {}",
        style("Edited").green().bold(),
        style(&slot.ticket_key).bold(),
        style(slot.id),
    );

    Ok(())
}
