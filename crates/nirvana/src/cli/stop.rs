use crate::cli::StopArgs;

pub(crate) async fn run(_args: StopArgs) -> anyhow::Result<()> {
    println!("stopping active slot");
    Ok(())
}
