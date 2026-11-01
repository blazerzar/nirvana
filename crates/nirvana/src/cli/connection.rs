use crate::cli::AddArgs;

pub(crate) async fn add(args: AddArgs) -> anyhow::Result<()> {
    match args.name {
        Some(name) => println!("adding {name}"),
        None => println!("interactive add"),
    }
    Ok(())
}

pub(crate) async fn list() -> anyhow::Result<()> {
    println!("listing all connections");
    Ok(())
}

pub(crate) async fn activate(query: Option<&str>) -> anyhow::Result<()> {
    match query {
        Some(query) => println!("using {query}"),
        None => println!("interactive use"),
    }
    Ok(())
}

pub(crate) async fn test() -> anyhow::Result<()> {
    println!("testing active connection");
    Ok(())
}
