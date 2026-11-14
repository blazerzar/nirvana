use chrono::Utc;
use nirvana_core::{
    Change, ConnectionData, DomainError, Kind, Nirvana, NirvanaError, SecretStore, Slot,
    SlotTarget, TicketConnection, TicketSort,
};
use tempfile::TempDir;

const CONNECTION: &str = "Linear";

struct Setup {
    service: Nirvana,
    now: i64,
    _dir: TempDir,
}

async fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let mut service = Nirvana::open_in(dir.path()).await.unwrap();
    service.create_profile("work".to_string()).await.unwrap();
    service.switch_profile("work").await.unwrap();
    service
        .add_connection(ConnectionData {
            name: CONNECTION.to_string(),
            kind: Kind::Linear,
            host: "linear.app/nirvana".to_string(),
            identity: None,
            secret_store: SecretStore::Plaintext,
            token: "token".into(),
        })
        .await
        .unwrap();
    for (key, title) in [("NIR-1", "Fix login"), ("KAR-1", "Add search")] {
        service
            .store_test_ticket(CONNECTION, key, title)
            .await
            .unwrap();
    }

    Setup {
        service,
        now: Utc::now().timestamp(),
        _dir: dir,
    }
}

fn summary(slots: &[Slot]) -> Vec<(&str, i64, Option<i64>)> {
    slots
        .iter()
        .map(|slot| {
            (
                slot.ticket.ticket_key.as_str(),
                slot.model.started_at,
                slot.model.stopped_at,
            )
        })
        .collect()
}

#[tokio::test]
async fn start_switch_stop_resume() {
    let Setup { service, now, .. } = setup().await;
    let (t0, t1, t2, t3, t4) = (now - 3600, now - 2400, now - 1800, now - 1200, now - 600);

    let first = service.start_slot("NIR-1", Some(t0), None).await.unwrap();
    assert!(first.stopped.is_none());

    let second = service.start_slot("KAR-1", Some(t1), None).await.unwrap();
    let stopped = second.stopped.unwrap();
    assert_eq!(stopped.ticket.ticket_key, "NIR-1");
    assert_eq!(stopped.model.stopped_at, Some(t1));

    let switched = service.switch_slot(Some(t2)).await.unwrap();
    assert_eq!(switched.started.ticket.ticket_key, "NIR-1");
    let stopped = switched.stopped.unwrap();
    assert_eq!(stopped.ticket.ticket_key, "KAR-1");
    assert_eq!(stopped.model.stopped_at, Some(t2));

    let stopped = service.stop_slot(Some(t3)).await.unwrap().unwrap();
    assert_eq!(stopped.model.stopped_at, Some(t3));
    assert!(service.running_slot().await.unwrap().is_none());
    assert!(service.stop_slot(None).await.unwrap().is_none());

    let resumed = service.resume_slot(Some(t4)).await.unwrap();
    assert_eq!(resumed.ticket.ticket_key, "NIR-1");
    let running = service.running_slot().await.unwrap().unwrap();
    assert_eq!(running.model.id, resumed.model.id);

    let slots = service.list_slots(t0, None, false).await.unwrap();
    assert_eq!(
        summary(&slots),
        [
            ("NIR-1", t0, Some(t1)),
            ("KAR-1", t1, Some(t2)),
            ("NIR-1", t2, Some(t3)),
            ("NIR-1", t4, None),
        ]
    );

    let tickets = service
        .list_tickets(TicketConnection::Any, TicketSort::LastWorked)
        .await
        .unwrap();
    let last_worked: Vec<_> = tickets
        .iter()
        .map(|(ticket, _)| (ticket.ticket_key.as_str(), ticket.last_worked_at))
        .collect();
    assert_eq!(last_worked, [("NIR-1", t4), ("KAR-1", t1)]);
}

#[tokio::test]
async fn edit_and_delete() {
    let Setup { service, now, .. } = setup().await;
    let added = service
        .add_slot("NIR-1", now - 3600, now - 1800, None)
        .await
        .unwrap();
    let id = added.model.id;

    service
        .edit_slot(
            id,
            Some("KAR-1"),
            None,
            Some(now - 3000),
            Change::Set(now - 1200),
        )
        .await
        .unwrap();
    let (edited, published) = service.get_slot(SlotTarget::At(now - 2000)).await.unwrap();
    assert_eq!(edited.model.id, id);
    assert_eq!(edited.ticket.ticket_key, "KAR-1");
    assert_eq!(edited.model.started_at, now - 3000);
    assert_eq!(edited.model.stopped_at, Some(now - 1200));
    assert!(!published);

    service
        .edit_slot(id, None, None, None, Change::Clear)
        .await
        .unwrap();
    let running = service.running_slot().await.unwrap().unwrap();
    assert_eq!(running.model.id, id);

    service.delete_slot(id).await.unwrap();
    let err = service.get_slot(SlotTarget::Id(id)).await.unwrap_err();
    assert!(matches!(
        err,
        NirvanaError::Domain(DomainError::SlotNotFound(missing)) if missing == id
    ));
    assert!(service.running_slot().await.unwrap().is_none());
}

#[tokio::test]
async fn list_ranges() {
    let Setup { service, now, .. } = setup().await;
    let (h, m) = (3600, 60);
    for (key, started_at, stopped_at) in [
        ("NIR-1", now - 5 * h, now - 4 * h),
        ("KAR-1", now - 3 * h, now - 2 * h),
        ("NIR-1", now - 90 * m, now - 60 * m),
    ] {
        service
            .add_slot(key, started_at, stopped_at, None)
            .await
            .unwrap();
    }
    service
        .start_slot("KAR-1", Some(now - 30 * m), None)
        .await
        .unwrap();

    let partly_overlapping = service
        .list_slots(now - 4 * h - 30 * m, Some(now - 2 * h - 30 * m), false)
        .await
        .unwrap();
    assert_eq!(
        summary(&partly_overlapping),
        [
            ("NIR-1", now - 5 * h, Some(now - 4 * h)),
            ("KAR-1", now - 3 * h, Some(now - 2 * h)),
        ]
    );

    let between_edges = service
        .list_slots(now - 2 * h, Some(now - 90 * m), false)
        .await
        .unwrap();
    assert!(between_edges.is_empty());

    let unbounded = service.list_slots(now - 75 * m, None, false).await.unwrap();
    assert_eq!(
        summary(&unbounded),
        [
            ("NIR-1", now - 90 * m, Some(now - 60 * m)),
            ("KAR-1", now - 30 * m, None),
        ]
    );

    let by_ticket = service.list_slots(now - 6 * h, None, true).await.unwrap();
    assert_eq!(
        summary(&by_ticket),
        [
            ("NIR-1", now - 5 * h, Some(now - 4 * h)),
            ("NIR-1", now - 90 * m, Some(now - 60 * m)),
            ("KAR-1", now - 3 * h, Some(now - 2 * h)),
            ("KAR-1", now - 30 * m, None),
        ]
    );
}
