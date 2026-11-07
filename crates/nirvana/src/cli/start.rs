use crate::cli::StartArgs;

pub(crate) async fn run(args: StartArgs) -> anyhow::Result<()> {
    match args.ticket {
        Some(ticket) => println!("starting {ticket}"),
        None => println!("interactive start"),
    }
    Ok(())
}
