use nirvana_core::secrets::SecretStore;

use crate::cli::ConnectionKind;

pub(crate) async fn add(
    name: Option<String>,
    kind: Option<ConnectionKind>,
    host: Option<String>,
    credential: Option<String>,
    storage: Option<SecretStore>,
) -> anyhow::Result<()> {
    match (name, kind, host, credential, storage) {
        (Some(name), Some(kind), Some(host), Some(_), Some(storage)) => {
            println!("adding {name} ({kind}, {host}, {storage})")
        }
        _ => println!("interactive add"),
    }
    Ok(())
}

pub(crate) async fn list() -> anyhow::Result<()> {
    println!("listing all connections");
    Ok(())
}

pub(crate) async fn rename(name: Option<String>, new_name: Option<String>) -> anyhow::Result<()> {
    match (name, new_name) {
        (Some(name), Some(new_name)) => println!("renaming {name} to {new_name}"),
        _ => println!("interactive rename"),
    }
    Ok(())
}

pub(crate) async fn delete(
    name: Option<String>,
    with_slots: bool,
    yes: bool,
) -> anyhow::Result<()> {
    match name {
        Some(name) => println!("deleting {name} (with slots: {with_slots}, yes: {yes})"),
        None => println!("interactive delete"),
    }
    Ok(())
}

pub(crate) async fn test(name: Option<String>, all: bool) -> anyhow::Result<()> {
    match (name, all) {
        (_, true) => println!("testing all connections"),
        (Some(name), false) => println!("testing {name}"),
        (None, false) => println!("interactive test"),
    }
    Ok(())
}
