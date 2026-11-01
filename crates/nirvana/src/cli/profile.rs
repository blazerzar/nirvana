pub(crate) async fn switch(name: Option<String>) -> anyhow::Result<()> {
    match name {
        Some(name) => println!("switching to {name}"),
        None => println!("interactive switch"),
    }
    Ok(())
}

pub(crate) async fn list() -> anyhow::Result<()> {
    println!("listing all profiles");
    Ok(())
}

pub(crate) async fn create(name: Option<String>, _switch: bool) -> anyhow::Result<()> {
    match name {
        Some(name) => println!("creating {name}"),
        None => println!("interactive create"),
    }
    Ok(())
}

pub(crate) async fn delete(name: Option<String>, _yes: bool) -> anyhow::Result<()> {
    match name {
        Some(name) => println!("deleting {name}"),
        None => println!("interactive delete"),
    }
    Ok(())
}
