use console::style;
use nirvana_core::service::Nirvana;

pub(crate) struct DeleteArgs {
    pub slot_id: i64,
}

pub(crate) fn run(args: DeleteArgs) -> anyhow::Result<()> {
    let api = Nirvana::new()?;

    let slot = api.delete_slot(args.slot_id)?;

    println!(
        "{} {} slot {}",
        style("Deleted").green().bold(),
        style(&slot.ticket_key).bold(),
        style(slot.id),
    );

    Ok(())
}
