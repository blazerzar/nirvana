use chrono::Utc;
use nirvana_core::{
    ConnectionData, ConnectionEdit, DomainError, JiraSettings, JiraSettingsEdit, Kind, Nirvana,
    NirvanaError, SecretLocation, SecretStore, Settings, SettingsEdit, TicketConnection,
    TicketSort,
};
use tempfile::TempDir;

struct Setup {
    service: Nirvana,
    _dir: TempDir,
}

async fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let mut service = Nirvana::open_in(dir.path()).await.unwrap();
    service.create_profile("work".to_string()).await.unwrap();
    service.switch_profile("work").await.unwrap();
    Setup { service, _dir: dir }
}

fn jira_cloud(secret_store: SecretStore) -> ConnectionData {
    ConnectionData {
        name: "Jira".to_string(),
        kind: Kind::JiraCloud,
        host: " https://nirvana.atlassian.net/ ".to_string(),
        identity: Some("me@example.com".to_string()),
        secret_store,
        token: "token".into(),
    }
}

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
async fn add_edit_and_move_credential() {
    let Setup { service, .. } = setup().await;
    let added = service
        .add_connection(jira_cloud(SecretStore::Keyring))
        .await
        .unwrap();
    assert_eq!(added.model.host, "nirvana.atlassian.net");
    assert_eq!(added.secret_store, SecretStore::Keyring);
    let SecretLocation::Keyring { account } =
        service.get_credential("Jira").await.unwrap().location
    else {
        panic!("expected a keyring credential");
    };
    assert!(account.starts_with("nirvana_work_Jira_"));

    service
        .edit_connection(
            "Jira",
            ConnectionEdit {
                name: Some("Work Jira".to_string()),
                host: Some("http://work.atlassian.net/".to_string()),
                identity: Some("work@example.com".to_string()),
            },
        )
        .await
        .unwrap();
    service
        .edit_connection_settings(
            "Work Jira",
            SettingsEdit::Jira(JiraSettingsEdit {
                publish: Some(false),
                squash: Some(true),
            }),
        )
        .await
        .unwrap();
    let edited = service.get_connection("Work Jira").await.unwrap();
    assert_eq!(edited.model.id, added.model.id);
    assert_eq!(edited.model.host, "work.atlassian.net");
    assert_eq!(edited.model.identity.as_deref(), Some("work@example.com"));
    assert!(matches!(
        edited.settings,
        Settings::Jira(JiraSettings {
            publish: false,
            squash: true
        })
    ));
    let renamed = service.get_credential("Work Jira").await.unwrap();
    assert_eq!(renamed.location, SecretLocation::Keyring { account });

    service
        .update_credential("Work Jira", "new token".into())
        .await
        .unwrap();
    service
        .move_credential("Work Jira", SecretStore::Plaintext)
        .await
        .unwrap();
    let moved = service.get_credential("Work Jira").await.unwrap();
    assert_eq!(moved.location, SecretLocation::Plaintext);
    service
        .move_credential("Work Jira", SecretStore::Keyring)
        .await
        .unwrap();
    let credentials = service.list_credentials().await.unwrap();
    assert_eq!(credentials.len(), 1);
    assert_eq!(credentials[0].connection, "Work Jira");
    assert_eq!(credentials[0].location.store(), SecretStore::Keyring);
}

#[tokio::test]
async fn connections_belong_to_their_profile() {
    let Setup { mut service, .. } = setup().await;
    service.add_connection(linear("Linear")).await.unwrap();
    service
        .store_test_ticket("Linear", "NIR-1", "Fix login")
        .await
        .unwrap();

    service
        .create_profile("personal".to_string())
        .await
        .unwrap();
    service.switch_profile("personal").await.unwrap();
    assert!(service.list_connections().await.unwrap().is_empty());
    let err = service.get_connection("Linear").await.unwrap_err();
    assert!(matches!(
        err,
        NirvanaError::Domain(DomainError::ConnectionNotFound(name)) if name == "Linear"
    ));
    service.add_connection(linear("Linear")).await.unwrap();
    service
        .store_test_ticket("Linear", "KAR-1", "Add search")
        .await
        .unwrap();
    assert_eq!(
        ticket_keys(&service, TicketConnection::Any).await,
        ["KAR-1"]
    );

    service.delete_connection("Linear", true).await.unwrap();
    assert!(service.list_connections().await.unwrap().is_empty());

    service.switch_profile("work").await.unwrap();
    let connections = service.list_connections().await.unwrap();
    assert_eq!(connections.len(), 1);
    assert_eq!(connections[0].model.name, "Linear");
    assert_eq!(
        ticket_keys(&service, TicketConnection::Any).await,
        ["NIR-1"]
    );
}

#[tokio::test]
async fn delete_connection_keeps_or_deletes_tickets() {
    let Setup { service, .. } = setup().await;
    let now = Utc::now().timestamp();
    service.add_connection(linear("Linear")).await.unwrap();
    service
        .add_connection(jira_cloud(SecretStore::Plaintext))
        .await
        .unwrap();
    for (connection, key, started_at) in [
        ("Linear", "NIR-1", now - 3600),
        ("Jira", "KAR-1", now - 1800),
    ] {
        service
            .store_test_ticket(connection, key, "Title")
            .await
            .unwrap();
        service
            .add_slot(key, started_at, started_at + 600, Some(connection))
            .await
            .unwrap();
    }

    service.delete_connection("Linear", false).await.unwrap();
    assert_eq!(
        ticket_keys(&service, TicketConnection::Disconnected).await,
        ["NIR-1"]
    );
    assert_eq!(
        ticket_keys(&service, TicketConnection::Connected).await,
        ["KAR-1"]
    );
    assert_eq!(
        service
            .list_slots(now - 7200, None, false)
            .await
            .unwrap()
            .len(),
        2
    );

    service.delete_connection("Jira", true).await.unwrap();
    assert_eq!(
        ticket_keys(&service, TicketConnection::Any).await,
        ["NIR-1"]
    );
    let slots = service.list_slots(now - 7200, None, false).await.unwrap();
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].ticket.ticket_key, "NIR-1");

    let err = service.delete_connection("Jira", true).await.unwrap_err();
    assert!(matches!(
        err,
        NirvanaError::Domain(DomainError::ConnectionNotFound(name)) if name == "Jira"
    ));
}

#[tokio::test]
async fn connection_names_are_unique_per_profile() {
    let Setup { service, .. } = setup().await;
    for name in ["Linear", "Other"] {
        service.add_connection(linear(name)).await.unwrap();
    }

    let err = service.add_connection(linear("Linear")).await.unwrap_err();
    assert!(matches!(
        err,
        NirvanaError::Domain(DomainError::ConnectionExists(name)) if name == "Linear"
    ));
    let err = service
        .edit_connection(
            "Other",
            ConnectionEdit {
                name: Some("Linear".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        NirvanaError::Domain(DomainError::ConnectionExists(name)) if name == "Linear"
    ));
    let names: Vec<_> = service
        .list_connections()
        .await
        .unwrap()
        .into_iter()
        .map(|connection| connection.model.name)
        .collect();
    assert_eq!(names, ["Linear", "Other"]);
}
