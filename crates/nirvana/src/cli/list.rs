use crate::cli::ListArgs;

pub(crate) async fn run(_args: ListArgs) -> anyhow::Result<()> {
    println!("listing slots");
    Ok(())
}
