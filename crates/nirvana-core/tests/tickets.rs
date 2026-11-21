use nirvana_core::mock::{mock_issue, mount_missing_issues};
use nirvana_core::{
    ConnectionData, DomainError, Kind, Nirvana, NirvanaError, SecretStore, TicketConnection,
    TicketSort,
};
use tempfile::TempDir;
use wiremock::MockServer;

const JIRA: &str = "Jira";
const LINEAR: &str = "Linear";
const CONNECTIONS: [(&str, Kind, &str); 2] = [
    (JIRA, Kind::JiraCloud, "KAR-1"),
    (LINEAR, Kind::Linear, "NIR-1"),
];

struct Setup {
    service: Nirvana,
    mock: MockServer,
    _dir: TempDir,
}

async fn setup() -> Setup {
    let mock = MockServer::start().await;
    mount_missing_issues(&mock).await;
    let dir = tempfile::tempdir().unwrap();
    let service = Nirvana::open_in(dir.path()).await.unwrap();
    service.create_profile("work".to_string()).await.unwrap();
    service.switch_profile("work").await.unwrap();
    for (name, kind, _) in CONNECTIONS {
        let identity = (kind != Kind::Linear).then(|| "me@example.com".to_string());
        service
            .add_connection(ConnectionData {
                name: name.to_string(),
                kind,
                host: mock.uri(),
                identity,
                secret_store: SecretStore::Plaintext,
                token: "token".into(),
            })
            .await
            .unwrap();
    }
    Setup {
        service,
        mock,
        _dir: dir,
    }
}

async fn tickets(service: &Nirvana) -> Vec<(String, String, String)> {
    service
        .list_tickets(TicketConnection::Any, TicketSort::Key)
        .await
        .unwrap()
        .into_iter()
        .map(|(ticket, connection)| (ticket.ticket_key, ticket.title, connection.unwrap().name))
        .collect()
}

fn ticket(key: &str, title: &str, connection: &str) -> (String, String, String) {
    (key.to_string(), title.to_string(), connection.to_string())
}

#[tokio::test]
async fn load_ticket_stores_ticket_of_named_connection() {
    let Setup {
        service,
        mock,
        _dir,
    } = setup().await;
    mock_issue(&mock, Kind::JiraCloud, "KAR-1", "Fix login").await;
    mock_issue(&mock, Kind::Linear, "NIR-1", "Add search").await;

    service.load_ticket("KAR-1", Some(JIRA)).await.unwrap();
    service.load_ticket("NIR-1", Some(LINEAR)).await.unwrap();

    assert_eq!(
        tickets(&service).await,
        [
            ticket("KAR-1", "Fix login", JIRA),
            ticket("NIR-1", "Add search", LINEAR),
        ]
    );
}

#[tokio::test]
async fn load_ticket_stores_ticket_found_in_one_connection() {
    for (name, kind, key) in CONNECTIONS {
        let Setup {
            service,
            mock,
            _dir,
        } = setup().await;
        mock_issue(&mock, kind, key, "Fix login").await;

        service.load_ticket(key, None).await.unwrap();

        assert_eq!(tickets(&service).await, [ticket(key, "Fix login", name)]);
    }
}

#[tokio::test]
async fn load_ticket_rejects_ticket_found_in_several_connections() {
    let Setup {
        service,
        mock,
        _dir,
    } = setup().await;
    mock_issue(&mock, Kind::JiraCloud, "NIR-1", "Fix login").await;
    mock_issue(&mock, Kind::Linear, "NIR-1", "Add search").await;

    let err = service.load_ticket("NIR-1", None).await.unwrap_err();

    assert!(matches!(
        err,
        NirvanaError::Domain(DomainError::AmbiguousTicket(key, connections))
            if key == "NIR-1" && connections == [JIRA, LINEAR]
    ));
    assert!(tickets(&service).await.is_empty());
}

#[tokio::test]
async fn load_ticket_of_missing_issue_stores_nothing() {
    let Setup {
        service,
        mock: _mock,
        _dir,
    } = setup().await;

    let err = service.load_ticket("KAR-9", None).await.unwrap_err();

    assert!(matches!(
        err,
        NirvanaError::Domain(DomainError::TicketNotFound(key)) if key == "KAR-9"
    ));
    assert!(tickets(&service).await.is_empty());
}

#[tokio::test]
async fn sync_ticket_updates_title() {
    for (name, kind, key) in CONNECTIONS {
        let Setup {
            service,
            mock,
            _dir,
        } = setup().await;
        service
            .store_test_ticket(name, key, "Fix login")
            .await
            .unwrap();
        mock_issue(&mock, kind, key, "Fix logout").await;

        service.sync_ticket(key, None).await.unwrap();

        assert_eq!(tickets(&service).await, [ticket(key, "Fix logout", name)]);
    }
}

#[tokio::test]
async fn sync_ticket_of_missing_issue_keeps_title() {
    for (name, _, key) in CONNECTIONS {
        let Setup {
            service,
            mock: _mock,
            _dir,
        } = setup().await;
        service
            .store_test_ticket(name, key, "Fix login")
            .await
            .unwrap();

        let err = service.sync_ticket(key, None).await.unwrap_err();

        assert_eq!(err.to_string(), format!("ticket not found: {key}"));
        assert_eq!(tickets(&service).await, [ticket(key, "Fix login", name)]);
    }
}
