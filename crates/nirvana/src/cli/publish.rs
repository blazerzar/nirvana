use crate::cli::PublishArgs;

pub(crate) async fn run(_args: PublishArgs) -> anyhow::Result<()> {
    println!("publishing slots");
    Ok(())
}
