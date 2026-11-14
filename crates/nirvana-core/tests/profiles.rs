use nirvana_core::{
    ConnectionData, DomainError, Kind, Nirvana, NirvanaError, SecretStore, TicketConnection,
    TicketSort,
};
use tempfile::TempDir;

fn linear(name: &str) -> ConnectionData {
    ConnectionData {
        name: name.to_string(),
        kind: Kind::Linear,
        host: "linear.app/nirvana".to_string(),
        identity: None,
        secret_store: SecretStore::Plaintext,
        token: "token".into(),
    }
}

async fn reopen(dir: &TempDir) -> Nirvana {
    Nirvana::open_in(dir.path()).await.unwrap()
}

async fn active_name(service: &Nirvana) -> String {
    service.active_profile().await.unwrap().profile.name
}

async fn profile_names(service: &Nirvana) -> Vec<String> {
    service
        .list_profiles()
        .await
        .unwrap()
        .into_iter()
        .map(|profile| profile.name)
        .collect()
}

async fn connection_names(service: &Nirvana) -> Vec<String> {
    service
        .list_connections()
        .await
        .unwrap()
        .into_iter()
        .map(|connection| connection.model.name)
        .collect()
}

#[tokio::test]
async fn fresh_data_dir_needs_a_profile() {
    let dir = tempfile::tempdir().unwrap();
    let mut service = reopen(&dir).await;
    assert!(service.list_profiles().await.unwrap().is_empty());

    let err = &service.active_profile().await.unwrap_err();
    matches!(err, NirvanaError::Domain(DomainError::NoActiveProfile));
    let err = &service.add_connection(linear("Linear")).await.unwrap_err();
    matches!(err, NirvanaError::Domain(DomainError::NoActiveProfile));

    service.create_profile("work".to_string()).await.unwrap();
    let err = &service.active_profile().await.unwrap_err();
    matches!(err, NirvanaError::Domain(DomainError::NoActiveProfile));
    assert!(service.switch_profile("work").await.unwrap());
    service.add_connection(linear("Linear")).await.unwrap();
    drop(service);

    let service = reopen(&dir).await;
    let active = service.active_profile().await.unwrap();
    assert_eq!(active.profile.name, "work");
    let connections: Vec<_> = active
        .connections
        .iter()
        .map(|connection| connection.model.name.as_str())
        .collect();
    assert_eq!(connections, ["Linear"]);
}

#[tokio::test]
async fn switch_rename_and_delete_persist() {
    let dir = tempfile::tempdir().unwrap();
    let mut service = reopen(&dir).await;
    for name in ["work", "personal"] {
        service.create_profile(name.to_string()).await.unwrap();
    }
    service.switch_profile("work").await.unwrap();
    service.add_connection(linear("Linear")).await.unwrap();

    assert!(service.switch_profile("personal").await.unwrap());
    assert!(!service.switch_profile("personal").await.unwrap());
    assert!(connection_names(&service).await.is_empty());

    service
        .rename_profile("personal", "home".to_string())
        .await
        .unwrap();
    drop(service);

    let mut service = reopen(&dir).await;
    assert_eq!(active_name(&service).await, "home");
    assert_eq!(profile_names(&service).await, ["work", "home"]);

    service.switch_profile("work").await.unwrap();
    assert_eq!(connection_names(&service).await, ["Linear"]);
    let active = service.delete_profile("work").await.unwrap();
    assert_eq!(active.as_deref(), Some("home"));
    drop(service);

    let mut service = reopen(&dir).await;
    assert_eq!(active_name(&service).await, "home");
    assert_eq!(profile_names(&service).await, ["home"]);

    assert_eq!(service.delete_profile("home").await.unwrap(), None);
    drop(service);

    let service = reopen(&dir).await;
    assert!(service.list_profiles().await.unwrap().is_empty());
    let err = &service.active_profile().await.unwrap_err();
    matches!(err, NirvanaError::Domain(DomainError::NoActiveProfile));
}

#[tokio::test]
async fn delete_profile_removes_its_data() {
    let dir = tempfile::tempdir().unwrap();
    let mut service = reopen(&dir).await;
    for name in ["work", "personal"] {
        service.create_profile(name.to_string()).await.unwrap();
        service.switch_profile(name).await.unwrap();
        service.add_connection(linear("Linear")).await.unwrap();
        service
            .store_test_ticket("Linear", "NIR-1", "Fix login")
            .await
            .unwrap();
    }

    assert_eq!(service.delete_profile("work").await.unwrap(), None);
    assert_eq!(active_name(&service).await, "personal");
    assert_eq!(connection_names(&service).await, ["Linear"]);
    let tickets = service
        .list_tickets(TicketConnection::Any, TicketSort::Key)
        .await
        .unwrap();
    assert_eq!(tickets.len(), 1);

    service.create_profile("work".to_string()).await.unwrap();
    service.switch_profile("work").await.unwrap();
    assert!(connection_names(&service).await.is_empty());
    let tickets = service
        .list_tickets(TicketConnection::Any, TicketSort::Key)
        .await
        .unwrap();
    assert!(tickets.is_empty());
}

#[tokio::test]
async fn profile_names_are_validated() {
    let dir = tempfile::tempdir().unwrap();
    let mut service = reopen(&dir).await;
    for name in ["work", "personal"] {
        service.create_profile(name.to_string()).await.unwrap();
    }

    let err = service
        .create_profile("work".to_string())
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        NirvanaError::Domain(DomainError::ProfileExists(name)) if name == "work"
    ));
    let err = service.create_profile(" ".to_string()).await.unwrap_err();
    assert!(matches!(
        err,
        NirvanaError::Domain(DomainError::EmptyProfileName)
    ));
    let err = service
        .rename_profile("personal", "work".to_string())
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        NirvanaError::Domain(DomainError::ProfileExists(name)) if name == "work"
    ));
    for err in [
        service.switch_profile("missing").await.unwrap_err(),
        service
            .rename_profile("missing", "other".to_string())
            .await
            .unwrap_err(),
        service.delete_profile("missing").await.unwrap_err(),
    ] {
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::ProfileNotFound(name)) if name == "missing"
        ));
    }
    assert_eq!(profile_names(&service).await, ["work", "personal"]);
}
