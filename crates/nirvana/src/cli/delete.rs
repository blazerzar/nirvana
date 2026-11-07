use crate::cli::DeleteArgs;

pub(crate) async fn run(args: DeleteArgs) -> anyhow::Result<()> {
    println!("deleting slot {}", args.slot_id);
    Ok(())
}
