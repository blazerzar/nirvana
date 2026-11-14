use nirvana_core::{
    ConnectionEdit, DomainError, Kind, Nirvana, NirvanaError, SecretLocation, SecretStore,
    SlotTarget, TicketConnection, TicketSort,
};
use tempfile::TempDir;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/v0_3_0");

/// Copies the 0.3.0-alpha fixture DB and config into a fresh data dir.
fn data_dir_030() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (fixture, file) in [
        ("nirvana_030.db", "nirvana.db"),
        ("config_030.toml", "config.toml"),
    ] {
        std::fs::copy(format!("{FIXTURES}/{fixture}"), dir.path().join(file)).unwrap();
    }
    dir
}

async fn ticket_keys(service: &Nirvana, connection: TicketConnection) -> Vec<String> {
    service
        .list_tickets(connection, TicketSort::Key)
        .await
        .unwrap()
        .into_iter()
        .map(|(ticket, _)| ticket.ticket_key)
        .collect()
}

#[tokio::test]
async fn opens_030_data_dir() {
    let dir = data_dir_030();
    let service = Nirvana::open_in(dir.path()).await.unwrap();

    let active = service.active_profile().await.unwrap();
    assert_eq!(active.profile.name, "default");
    let connections: Vec<_> = active
        .connections
        .iter()
        .map(|c| (c.model.name.as_str(), c.kind, c.secret_store))
        .collect();
    assert_eq!(
        connections,
        [
            ("work", Kind::JiraCloud, SecretStore::Plaintext),
            ("local", Kind::JiraDc, SecretStore::Plaintext),
        ]
    );
    let credential = service.get_credential("local").await.unwrap();
    assert_eq!(credential.location, SecretLocation::Plaintext);

    assert_eq!(ticket_keys(&service, TicketConnection::Any).await.len(), 17);
    assert_eq!(
        ticket_keys(&service, TicketConnection::Named("local".to_string())).await,
        [
            "NIR-12", "PROD-23", "PROD-7", "PROD-88", "QA-14", "QA-41", "QA-62"
        ]
    );

    assert_eq!(service.list_slots(0, None, false).await.unwrap().len(), 27);
    let (_, published) = service.get_slot(SlotTarget::Id(1)).await.unwrap();
    assert!(published);
    let (_, published) = service.get_slot(SlotTarget::Id(9)).await.unwrap();
    assert!(!published);
    let running = service.running_slot().await.unwrap().unwrap();
    assert_eq!(running.model.id, 27);
    assert_eq!(running.ticket.ticket_key, "PROD-88");
}

#[tokio::test]
async fn tracks_on_030_data_dir() {
    let dir = data_dir_030();
    let service = Nirvana::open_in(dir.path()).await.unwrap();

    let err = service.start_slot("NIR-12", None, None).await.unwrap_err();
    assert!(matches!(
        err,
        NirvanaError::Domain(DomainError::AmbiguousTicket(key, _)) if key == "NIR-12"
    ));

    let work = service.get_connection("work").await.unwrap();
    let result = service
        .start_slot("NIR-12", None, Some("work"))
        .await
        .unwrap();
    assert_eq!(result.stopped.unwrap().model.id, 27);
    assert_eq!(result.started.ticket.connection_id, Some(work.model.id));
    let started = result.started.model.id;
    drop(service);

    let service = Nirvana::open_in(dir.path()).await.unwrap();
    assert_eq!(
        service.active_profile().await.unwrap().profile.name,
        "default"
    );
    let running = service.running_slot().await.unwrap().unwrap();
    assert_eq!(running.model.id, started);
    let (stopped, _) = service.get_slot(SlotTarget::Id(27)).await.unwrap();
    assert_eq!(stopped.model.stopped_at, Some(running.model.started_at));
}

#[tokio::test]
async fn renames_and_moves_credential_on_030_data_dir() {
    let dir = data_dir_030();
    let mut service = Nirvana::open_in(dir.path()).await.unwrap();

    service
        .rename_profile("default", "home".to_string())
        .await
        .unwrap();
    service
        .edit_connection(
            "work",
            ConnectionEdit {
                name: Some("cloud".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    drop(service);

    let service = Nirvana::open_in(dir.path()).await.unwrap();
    let active = service.active_profile().await.unwrap();
    assert_eq!(active.profile.name, "home");
    let connections: Vec<_> = active
        .connections
        .iter()
        .map(|c| c.model.name.as_str())
        .collect();
    assert_eq!(connections, ["cloud", "local"]);

    service
        .move_credential("cloud", SecretStore::Keyring)
        .await
        .unwrap();
    let SecretLocation::Keyring { account } =
        service.get_credential("cloud").await.unwrap().location
    else {
        panic!("expected a keyring credential");
    };
    assert!(account.starts_with("nirvana_home_cloud_"));
    let cloud = service.get_connection("cloud").await.unwrap();
    assert_eq!(cloud.secret_store, SecretStore::Keyring);

    service
        .move_credential("cloud", SecretStore::Plaintext)
        .await
        .unwrap();
    let credential = service.get_credential("cloud").await.unwrap();
    assert_eq!(credential.location, SecretLocation::Plaintext);
}
