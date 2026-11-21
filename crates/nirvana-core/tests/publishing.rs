use chrono::Utc;
use nirvana_core::mock::{mock_comment, mock_worklog, mount_missing_issues};
use nirvana_core::{
    ConnectionData, Kind, LinearSettingsEdit, Nirvana, PublishState, SecretStore, SettingsEdit,
    Slot, SlotTarget, TargetState,
};
use tempfile::TempDir;
use wiremock::MockServer;

const JIRA: &str = "Jira";
const LINEAR: &str = "Linear";

struct Setup {
    service: Nirvana,
    mock: MockServer,
    now: i64,
    _dir: TempDir,
}

/// Returns a service with a Jira and a Linear connection, both publishing
/// to the mock server, and the tickets KAR-1 of Jira and NIR-1 of Linear.
async fn setup() -> Setup {
    let mock = MockServer::start().await;
    mount_missing_issues(&mock).await;
    let dir = tempfile::tempdir().unwrap();
    let service = Nirvana::open_in(dir.path()).await.unwrap();
    service.create_profile("work".to_string()).await.unwrap();
    service.switch_profile("work").await.unwrap();
    for (name, kind, identity) in [
        (JIRA, Kind::JiraCloud, Some("me@example.com".to_string())),
        (LINEAR, Kind::Linear, None),
    ] {
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
    service
        .edit_connection_settings(
            LINEAR,
            SettingsEdit::Linear(LinearSettingsEdit {
                publish: Some(true),
                squash: None,
            }),
        )
        .await
        .unwrap();
    for (connection, key) in [(JIRA, "KAR-1"), (LINEAR, "NIR-1")] {
        service
            .store_test_ticket(connection, key, "Title")
            .await
            .unwrap();
    }
    Setup {
        service,
        mock,
        now: Utc::now().timestamp(),
        _dir: dir,
    }
}

fn ids(slots: &[Slot]) -> Vec<i64> {
    slots.iter().map(|slot| slot.model.id).collect()
}

async fn states(service: &Nirvana, from: i64) -> Vec<(i64, PublishState)> {
    service
        .list_slots(from, None, false)
        .await
        .unwrap()
        .into_iter()
        .map(|(slot, state)| (slot.model.id, state))
        .collect()
}

#[tokio::test]
async fn publish_sends_worklogs_and_comments_once() {
    let Setup {
        service,
        mock,
        now,
        _dir,
    } = setup().await;
    let start = now - 7200;
    let jira = service
        .add_slot("KAR-1", start, start + 3600, Some(JIRA))
        .await
        .unwrap();
    let linear = service
        .add_slot("NIR-1", start + 3600, start + 4200, Some(LINEAR))
        .await
        .unwrap();
    mock_worklog(&mock, "KAR-1", 3600).await;
    mock_comment(&mock, "NIR-1", "10m").await;

    let result = service
        .publish_slots(None, None, None, false)
        .await
        .unwrap();

    assert!(result.failed.is_empty());
    assert_eq!(ids(&result.published), [jira.model.id, linear.model.id]);
    assert_eq!(
        states(&service, start).await,
        [
            (jira.model.id, PublishState::Published),
            (linear.model.id, PublishState::Published),
        ]
    );

    let result = service.publish_slots(None, None, None, true).await.unwrap();

    assert!(result.published.is_empty());
    assert!(result.failed.is_empty());
}

/// Mocks a worklog of KAR-1 or a comment on NIR-1 that logs one hour.
async fn mock_publish(mock: &MockServer, key: &str) {
    match key {
        "KAR-1" => mock_worklog(mock, key, 3600).await,
        _ => mock_comment(mock, key, "1h").await,
    }
}

#[tokio::test]
async fn publish_stores_failure_and_retry_publishes_it() {
    for (connection, key) in [(JIRA, "KAR-1"), (LINEAR, "NIR-1")] {
        let Setup {
            service,
            mock,
            now,
            _dir,
        } = setup().await;
        let start = now - 7200;
        let slot = service
            .add_slot(key, start, start + 3600, Some(connection))
            .await
            .unwrap();
        let id = slot.model.id;
        let error = format!("ticket not found: {key}");

        let result = service
            .publish_slots(None, None, Some(connection), false)
            .await
            .unwrap();

        assert!(result.published.is_empty());
        let failures: Vec<_> = result
            .failed
            .iter()
            .map(|failure| (failure.slot.model.id, failure.error.as_str()))
            .collect();
        assert_eq!(failures, [(id, error.as_str())]);
        assert_eq!(states(&service, start).await, [(id, PublishState::Failed)]);
        let (_, targets) = service.get_slot_targets(SlotTarget::Id(id)).await.unwrap();
        let targets: Vec<_> = targets
            .iter()
            .map(|target| {
                (
                    target.connection.name.as_str(),
                    target.state,
                    target.entry.error.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            targets,
            [(connection, TargetState::Failed, Some(error.as_str()))]
        );

        mock_publish(&mock, key).await;
        let result = service
            .publish_slots(None, None, Some(connection), false)
            .await
            .unwrap();
        assert!(result.published.is_empty());
        assert!(result.failed.is_empty());
        assert_eq!(states(&service, start).await, [(id, PublishState::Failed)]);

        let result = service
            .publish_slots(None, None, Some(connection), true)
            .await
            .unwrap();

        assert!(result.failed.is_empty());
        assert_eq!(ids(&result.published), [id]);
        assert_eq!(
            states(&service, start).await,
            [(id, PublishState::Published)]
        );
    }
}
