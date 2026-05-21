use console::style;
use nirvana_core::api::NirvanaApi;
use nirvana_core::api::domain::{Change, SlotEdit};

pub(crate) struct EditArgs {
    pub slot_id: i64,
    pub note: Change<String>,
    pub started_at: Option<i64>,
    pub stopped_at: Change<i64>,
}

pub(crate) fn run(args: EditArgs) -> anyhow::Result<()> {
    let api = NirvanaApi::new()?;

    let edit = SlotEdit {
        note: args.note,
        started_at: args.started_at,
        stopped_at: args.stopped_at,
    };

    let slot = api.edit_slot(args.slot_id, edit)?;

    println!(
        "{} {} slot {}",
        style("Edited").green().bold(),
        style(&slot.ticket_key).bold(),
        style(slot.id),
    );

    Ok(())
}
