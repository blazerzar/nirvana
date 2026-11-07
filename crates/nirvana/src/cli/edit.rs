use crate::cli::EditArgs;

pub(crate) async fn run(args: EditArgs) -> anyhow::Result<()> {
    println!("editing slot {}", args.slot_id);
    Ok(())
}
