use nirvana_core::secrets::SecretStore;

pub(crate) async fn list() -> anyhow::Result<()> {
    println!("listing credentials");
    Ok(())
}

pub(crate) async fn update(name: Option<String>, token: Option<String>) -> anyhow::Result<()> {
    match (name, token) {
        (Some(name), Some(_)) => println!("updating credential of {name}"),
        (Some(name), None) => println!("updating credential of {name} with prompted token"),
        (None, _) => println!("interactive credential update"),
    }
    Ok(())
}

pub(crate) async fn move_to(name: Option<String>, to: Option<SecretStore>) -> anyhow::Result<()> {
    match (name, to) {
        (Some(name), Some(to)) => println!("moving credential of {name} to {to}"),
        _ => println!("interactive credential move"),
    }
    Ok(())
}
