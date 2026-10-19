use console::style;
use nirvana_core::service::Nirvana;

pub(crate) struct DeleteArgs {
    pub slot_id: i64,
}

pub(crate) async fn run(args: DeleteArgs) -> anyhow::Result<()> {
    let api = Nirvana::new().await?;

    let (slot, ticket) = api.delete_slot(args.slot_id).await?;

    println!(
        "{} {} slot {}",
        style("Deleted").green().bold(),
        style(&ticket.ticket_key).bold(),
        style(slot.id),
    );

    Ok(())
}
