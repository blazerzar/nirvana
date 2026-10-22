use nirvana_entity::slot::Entity as Slot;
use nirvana_entity::ticket::Entity as Ticket;
use nirvana_entity::{connection, jira_settings, slot, ticket};
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DbErr, EntityTrait, IntoActiveModel, PaginatorTrait,
    QueryFilter, QueryOrder, TransactionTrait,
};

use crate::domain::{Connection, SlotCreate};
use crate::errors::{DbError, IntegrationError, TrackingError};
use crate::integration;
use crate::service::Nirvana;
use crate::service::slots::require_ticket;

impl Nirvana {
    pub async fn start_slot(
        &self,
        ticket_key: &str,
        at: Option<i64>,
    ) -> Result<(slot::Model, ticket::Model), super::NirvanaError> {
        let connection_id = self
            .config
            .core
            .active_connection
            .ok_or(TrackingError::NoActiveConnection)?;

        let connection = self.get_connection(connection_id).await?;
        let ticket_key = ticket_key.to_uppercase();
        let now = at.unwrap_or_else(|| chrono::Utc::now().timestamp());

        let ticket = self
            .resolve_ticket_for_work(&connection, connection_id, &ticket_key, now)
            .await?;

        if let Some(running) = Slot::find_running().one(&self.db).await? {
            let stopped_at = std::cmp::max(running.started_at + 1, now);
            let mut running = running.into_active_model();
            running.stopped_at = Set(Some(stopped_at));
            running.update(&self.db).await?;
        }

        let slot = slot::ActiveModel {
            ticket_id: Set(ticket.id),
            connection_id: Set(connection_id),
            started_at: Set(now),
            stopped_at: Set(None),
            ..Default::default()
        }
        .insert(&self.db)
        .await?;
        Ok((slot, ticket))
    }

    pub async fn create_slot(
        &self,
        input: SlotCreate,
    ) -> Result<(slot::Model, ticket::Model), super::NirvanaError> {
        let connection_id = self
            .config
            .core
            .active_connection
            .ok_or(TrackingError::NoActiveConnection)?;

        if input.stopped_at <= input.started_at {
            return Err(TrackingError::InvalidTimeRange.into());
        }

        let now = chrono::Utc::now().timestamp();
        if input.started_at > now || input.stopped_at > now {
            return Err(TrackingError::FutureTime.into());
        }

        let overlapping = Slot::find_overlapping(connection_id, input.started_at, input.stopped_at)
            .count(&self.db)
            .await?;
        if overlapping > 0 {
            return Err(TrackingError::SlotOverlap.into());
        }

        let connection = self.get_connection(connection_id).await?;
        let ticket_key = input.ticket_key.to_uppercase();
        let ticket = self
            .resolve_ticket_for_work(&connection, connection_id, &ticket_key, input.stopped_at)
            .await?;

        let slot = slot::ActiveModel {
            ticket_id: Set(ticket.id),
            connection_id: Set(connection_id),
            started_at: Set(input.started_at),
            stopped_at: Set(Some(input.stopped_at)),
            ..Default::default()
        }
        .insert(&self.db)
        .await?;
        Ok((slot, ticket))
    }

    pub async fn stop_slot(
        &self,
        at: Option<i64>,
    ) -> Result<Option<(slot::Model, ticket::Model)>, super::NirvanaError> {
        let now = at.unwrap_or_else(|| chrono::Utc::now().timestamp());
        let Some(running) = Slot::find_running()
            .find_also_related(ticket::Entity)
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        let (slot, ticket) = require_ticket(running)?;

        if now <= slot.started_at {
            return Err(DbError::StopBeforeStart.into());
        }

        let mut slot = slot.into_active_model();
        slot.stopped_at = Set(Some(now));
        let slot = slot.update(&self.db).await?;
        Ok(Some((slot, ticket)))
    }

    pub(crate) async fn get_connection(&self, id: i64) -> Result<Connection, super::NirvanaError> {
        let (model, jira_settings) = connection::Entity::find_by_id(id)
            .find_also_related(jira_settings::Entity)
            .one(&self.db)
            .await?
            .ok_or(TrackingError::NoActiveConnection)?;
        Ok(Connection::new(model, jira_settings)?)
    }

    pub(crate) async fn resolve_ticket_for_work(
        &self,
        connection: &Connection,
        connection_id: i64,
        ticket_key: &str,
        last_worked_at: i64,
    ) -> Result<ticket::Model, super::NirvanaError> {
        let profile_id = connection.model.profile_id;
        let find = Ticket::find()
            .filter(ticket::COLUMN.profile_id.eq(profile_id))
            .filter(ticket::COLUMN.ticket_key.eq(ticket_key));
        if let Some(t) = find.clone().one(&self.db).await? {
            return Ok(touch_ticket(t, last_worked_at, &self.db).await?);
        }

        let token = self.get_token(connection_id).await?;
        let integ = integration::build_integration(connection, &token);
        let issue = integ.fetch_issue(ticket_key).await.map_err(|e| match e {
            IntegrationError::TicketNotFound(key) => {
                super::NirvanaError::Tracking(TrackingError::TicketNotFound(key))
            }
            other => super::NirvanaError::Integration(other),
        })?;

        let txn = self.db.begin().await?;
        let ticket = match find.one(&txn).await? {
            Some(t) => touch_ticket(t, last_worked_at, &txn).await?,
            None => {
                ticket::ActiveModel {
                    profile_id: Set(profile_id),
                    ticket_key: Set(ticket_key.to_string()),
                    title: Set(Some(issue.summary)),
                    connection_id: Set(Some(connection_id)),
                    last_worked_at: Set(last_worked_at),
                    synced_at: Set(chrono::Utc::now().timestamp()),
                    ..Default::default()
                }
                .insert(&txn)
                .await?
            }
        };
        txn.commit().await?;
        Ok(ticket)
    }

    pub async fn list_recent_tickets(&self) -> Result<Vec<ticket::Model>, super::NirvanaError> {
        let connection_id = self
            .config
            .core
            .active_connection
            .ok_or(TrackingError::NoActiveConnection)?;
        let profile_id = self.get_connection(connection_id).await?.model.profile_id;
        Ok(Ticket::find()
            .filter(ticket::COLUMN.profile_id.eq(profile_id))
            .order_by_desc(ticket::COLUMN.last_worked_at)
            .all(&self.db)
            .await?)
    }
}

async fn touch_ticket(
    ticket: ticket::Model,
    last_worked_at: i64,
    db: &impl ConnectionTrait,
) -> Result<ticket::Model, DbErr> {
    let mut ticket = ticket.into_active_model();
    ticket.last_worked_at = Set(last_worked_at);
    ticket.update(db).await
}

#[cfg(test)]
mod tests {
    use std::ops::Deref;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use nirvana_entity::publish_entry::Entity as PublishEntry;
    use nirvana_entity::{profile, publish_entry};
    use sea_orm::ActiveValue::Unchanged;
    use sea_orm::prelude::Uuid;

    use super::*;
    use crate::config::{AppConfig, CoreConfig, GuiConfig, IdleConfig};
    use crate::domain::{Change, ConnectionData, SlotEdit, SlotSort};
    use crate::errors::NirvanaError;
    use crate::integration::IntegrationError;
    use crate::paths::AppPaths;
    use crate::storage;

    static TEST_DB_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TestApi {
        api: Option<Nirvana>,
        base: PathBuf,
    }

    impl Deref for TestApi {
        type Target = Nirvana;

        fn deref(&self) -> &Self::Target {
            self.api.as_ref().unwrap()
        }
    }

    impl Drop for TestApi {
        fn drop(&mut self) {
            drop(self.api.take());
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    async fn test_api() -> TestApi {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "nirvana-create-slot-test-{}-{suffix}-{counter}",
            std::process::id(),
        ));
        let paths = AppPaths {
            config_dir: base.clone(),
            data_dir: base.clone(),
            log_dir: base.clone(),
            config_file: base.join("config.toml"),
            db_file: base.join("nirvana.db"),
            log_file: base.join("nirvana.log"),
            is_dev: true,
        };
        let db = storage::open(&paths.db_file).await.unwrap();
        let mut api = Nirvana {
            paths,
            config: AppConfig {
                schema_version: Some(1),
                core: CoreConfig {
                    active_connection: None,
                },
                gui: GuiConfig {
                    font_scale: 1.0,
                    theme: "high-contrast-dark".to_string(),
                    show_tray_icon: false,
                },
                idle: IdleConfig {
                    enabled: false,
                    methods: vec!["lock".into(), "sleep".into(), "input".into()],
                    threshold_secs: 300,
                },
            },
            db,
        };
        profile::ActiveModel {
            name: Set("test".to_string()),
            ..Default::default()
        }
        .insert(&api.db)
        .await
        .unwrap();
        let connection = api
            .add_connection(ConnectionData {
                name: "Test Jira".to_string(),
                kind: "jira-cloud".to_string(),
                host: "https://example.atlassian.net".to_string(),
                identity: "tester@example.com".to_string(),
                secret_store: "plaintext".to_string(),
                token: "token".to_string(),
            })
            .await
            .unwrap();
        api.config.core.active_connection = Some(connection.model.id);

        TestApi {
            api: Some(api),
            base,
        }
    }

    async fn insert_ticket(api: &Nirvana, key: &str) -> i64 {
        let connection_id = api.config.core.active_connection.unwrap();
        ticket::ActiveModel {
            profile_id: Set(api
                .get_connection(connection_id)
                .await
                .unwrap()
                .model
                .profile_id),
            ticket_key: Set(key.to_string()),
            title: Set(Some("Existing ticket".to_string())),
            connection_id: Set(Some(connection_id)),
            last_worked_at: Set(0),
            synced_at: Set(0),
            ..Default::default()
        }
        .insert(&api.db)
        .await
        .unwrap()
        .id
    }

    async fn publish(api: &Nirvana, slot_ids: &[i64], result: Result<(), IntegrationError>) {
        let connection_id = api.config.core.active_connection.unwrap();
        api.mark_pending(connection_id, slot_ids.iter().copied())
            .await
            .unwrap();
        api.record_publish(connection_id, slot_ids, &result, 200)
            .await
            .unwrap();
    }

    async fn publish_entries(api: &Nirvana, slot_id: i64) -> Vec<publish_entry::Model> {
        PublishEntry::find()
            .filter(publish_entry::COLUMN.slot_id.eq(slot_id))
            .all(&api.db)
            .await
            .unwrap()
    }

    async fn unpublished_ids(api: &Nirvana) -> Vec<i64> {
        api.get_unpublished_slots(0, None)
            .await
            .unwrap()
            .into_iter()
            .map(|(slot, _)| slot.id)
            .collect()
    }

    fn slot_create(ticket_key: &str, started_at: i64, stopped_at: i64) -> SlotCreate {
        SlotCreate {
            ticket_key: ticket_key.to_string(),
            started_at,
            stopped_at,
        }
    }

    #[tokio::test]
    async fn creates_completed_unpublished_slot() {
        let api = test_api().await;
        insert_ticket(&api, "DES-1").await;

        let (slot, ticket) = api
            .create_slot(slot_create("des-1", 100, 160))
            .await
            .unwrap();

        assert_eq!(ticket.ticket_key, "DES-1");
        assert_eq!(slot.started_at, 100);
        assert_eq!(slot.stopped_at, Some(160));
        assert!(publish_entries(&api, slot.id).await.is_empty());
    }

    #[tokio::test]
    async fn created_slot_gets_unique_uuid() {
        let api = test_api().await;
        insert_ticket(&api, "DES-1").await;

        let (slot, ticket) = api
            .create_slot(slot_create("DES-1", 100, 160))
            .await
            .unwrap();
        assert!(!slot.uuid.is_nil());

        let error = slot::ActiveModel {
            ticket_id: Set(ticket.id),
            connection_id: Set(slot.connection_id),
            started_at: Set(200),
            stopped_at: Set(Some(260)),
            uuid: Set(slot.uuid),
            ..Default::default()
        }
        .insert(&api.db)
        .await
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("UNIQUE constraint failed: slots.uuid")
        );

        let error = slot::ActiveModel {
            id: Unchanged(slot.id),
            uuid: Set(Uuid::nil()),
            ..Default::default()
        }
        .update(&api.db)
        .await
        .unwrap_err();
        assert!(error.to_string().contains("uuid must not change"));
    }

    #[tokio::test]
    async fn rejects_invalid_time_range() {
        let api = test_api().await;

        let error = api
            .create_slot(slot_create("DES-1", 160, 160))
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            NirvanaError::Tracking(TrackingError::InvalidTimeRange)
        ));
    }

    #[tokio::test]
    async fn rejects_future_times() {
        let api = test_api().await;
        let future = chrono::Utc::now().timestamp() + 60;

        let error = api
            .create_slot(slot_create("DES-1", future - 10, future))
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            NirvanaError::Tracking(TrackingError::FutureTime)
        ));
    }

    #[tokio::test]
    async fn rejects_overlapping_slot() {
        let api = test_api().await;
        insert_ticket(&api, "DES-1").await;
        api.create_slot(slot_create("DES-1", 100, 160))
            .await
            .unwrap();

        let error = api
            .create_slot(slot_create("DES-1", 150, 200))
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            NirvanaError::Tracking(TrackingError::SlotOverlap)
        ));
    }

    #[tokio::test]
    async fn does_not_stop_existing_running_slot() {
        let api = test_api().await;
        let ticket_id = insert_ticket(&api, "DES-1").await;
        let connection_id = api.config.core.active_connection.unwrap();
        let running = slot::ActiveModel {
            ticket_id: Set(ticket_id),
            connection_id: Set(connection_id),
            started_at: Set(300),
            stopped_at: Set(None),
            ..Default::default()
        }
        .insert(&api.db)
        .await
        .unwrap();

        let (slot, _) = api
            .create_slot(slot_create("DES-1", 100, 160))
            .await
            .unwrap();
        let still_running = Slot::find_running().one(&api.db).await.unwrap().unwrap();

        assert_eq!(slot.stopped_at, Some(160));
        assert_eq!(still_running.id, running.id);
        assert_eq!(still_running.started_at, 300);
        assert_eq!(still_running.stopped_at, None);
    }

    #[tokio::test]
    async fn lists_slots_that_overlap_bounded_range() {
        let api = test_api().await;
        insert_ticket(&api, "DES-1").await;
        insert_ticket(&api, "DES-2").await;
        insert_ticket(&api, "DES-3").await;
        api.create_slot(slot_create("DES-1", 100, 140))
            .await
            .unwrap();
        api.create_slot(slot_create("DES-2", 140, 220))
            .await
            .unwrap();
        api.create_slot(slot_create("DES-3", 260, 320))
            .await
            .unwrap();

        let slots = api
            .get_slots(150, Some(260), SlotSort::StartedAt)
            .await
            .unwrap();

        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].1.ticket_key, "DES-2");
        assert_eq!(slots[0].0.started_at, 140);
        assert_eq!(slots[0].0.stopped_at, Some(220));
    }

    #[tokio::test]
    async fn lists_open_ended_slots_that_overlap_range_start() {
        let api = test_api().await;
        insert_ticket(&api, "DES-1").await;
        insert_ticket(&api, "DES-2").await;
        api.create_slot(slot_create("DES-1", 100, 140))
            .await
            .unwrap();
        api.create_slot(slot_create("DES-2", 140, 220))
            .await
            .unwrap();

        let slots = api.get_slots(200, None, SlotSort::StartedAt).await.unwrap();

        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].1.ticket_key, "DES-2");
        assert_eq!(slots[0].0.started_at, 140);
        assert_eq!(slots[0].0.stopped_at, Some(220));
    }

    #[tokio::test]
    async fn edits_slot_ticket_to_existing_ticket() {
        let api = test_api().await;
        insert_ticket(&api, "DES-1").await;
        insert_ticket(&api, "DES-2").await;
        let (slot, _) = api
            .create_slot(slot_create("DES-1", 100, 160))
            .await
            .unwrap();

        let (edited, edited_ticket) = api
            .edit_slot(
                slot.id,
                SlotEdit {
                    ticket_key: Some("des-2".to_string()),
                    started_at: None,
                    stopped_at: Change::Skip,
                },
            )
            .await
            .unwrap();

        assert_eq!(edited.id, slot.id);
        assert_eq!(edited_ticket.ticket_key, "DES-2");
        assert_eq!(edited.started_at, 100);
        assert_eq!(edited.stopped_at, Some(160));
    }

    #[tokio::test]
    async fn rejects_ticket_edit_for_published_slot() {
        let api = test_api().await;
        insert_ticket(&api, "DES-1").await;
        insert_ticket(&api, "DES-2").await;
        let (slot, _) = api
            .create_slot(slot_create("DES-1", 100, 160))
            .await
            .unwrap();
        publish(&api, &[slot.id], Ok(())).await;

        let error = api
            .edit_slot(
                slot.id,
                SlotEdit {
                    ticket_key: Some("DES-2".to_string()),
                    started_at: None,
                    stopped_at: Change::Skip,
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            NirvanaError::Tracking(TrackingError::SlotAlreadyPublished)
        ));
    }

    #[tokio::test]
    async fn edits_slot_times() {
        let api = test_api().await;
        insert_ticket(&api, "DES-1").await;
        let (slot, _) = api
            .create_slot(slot_create("DES-1", 100, 160))
            .await
            .unwrap();

        let (edited, edited_ticket) = api
            .edit_slot(
                slot.id,
                SlotEdit {
                    ticket_key: None,
                    started_at: Some(90),
                    stopped_at: Change::Set(170),
                },
            )
            .await
            .unwrap();

        assert_eq!(edited_ticket.ticket_key, "DES-1");
        assert_eq!(edited.started_at, 90);
        assert_eq!(edited.stopped_at, Some(170));
    }

    #[tokio::test]
    async fn publish_marks_every_squashed_slot() {
        let api = test_api().await;
        insert_ticket(&api, "DES-1").await;
        let (first, _) = api
            .create_slot(slot_create("DES-1", 100, 160))
            .await
            .unwrap();
        let (second, _) = api
            .create_slot(slot_create("DES-1", 200, 260))
            .await
            .unwrap();

        publish(&api, &[first.id, second.id], Ok(())).await;

        for slot_id in [first.id, second.id] {
            let entries = publish_entries(&api, slot_id).await;
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].published_at, Some(200));
            assert_eq!(entries[0].error, None);
        }
        assert!(unpublished_ids(&api).await.is_empty());
    }

    #[tokio::test]
    async fn failed_publish_keeps_slot_unpublished_and_editable() {
        let api = test_api().await;
        insert_ticket(&api, "DES-1").await;
        let (slot, _) = api
            .create_slot(slot_create("DES-1", 100, 160))
            .await
            .unwrap();

        publish(
            &api,
            &[slot.id],
            Err(IntegrationError::Auth("expired token".to_string())),
        )
        .await;

        let entries = publish_entries(&api, slot.id).await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].published_at, None);
        assert_eq!(
            entries[0].error.as_deref(),
            Some("authentication failed: expired token")
        );
        assert_eq!(unpublished_ids(&api).await, [slot.id]);

        let (edited, _) = api
            .edit_slot(
                slot.id,
                SlotEdit {
                    ticket_key: None,
                    started_at: Some(90),
                    stopped_at: Change::Skip,
                },
            )
            .await
            .unwrap();
        assert_eq!(edited.started_at, 90);
    }

    #[tokio::test]
    async fn retried_publish_clears_error() {
        let api = test_api().await;
        insert_ticket(&api, "DES-1").await;
        let (slot, _) = api
            .create_slot(slot_create("DES-1", 100, 160))
            .await
            .unwrap();
        publish(
            &api,
            &[slot.id],
            Err(IntegrationError::Auth("expired token".to_string())),
        )
        .await;

        publish(&api, &[slot.id], Ok(())).await;

        let entries = publish_entries(&api, slot.id).await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].published_at, Some(200));
        assert_eq!(entries[0].error, None);
    }

    #[tokio::test]
    async fn failed_publish_keeps_ok_entry() {
        let api = test_api().await;
        insert_ticket(&api, "DES-1").await;
        let (slot, _) = api
            .create_slot(slot_create("DES-1", 100, 160))
            .await
            .unwrap();
        publish(&api, &[slot.id], Ok(())).await;

        publish(
            &api,
            &[slot.id],
            Err(IntegrationError::Auth("expired token".to_string())),
        )
        .await;

        let entries = publish_entries(&api, slot.id).await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].published_at, Some(200));
        assert_eq!(entries[0].error, None);
    }

    #[tokio::test]
    async fn pending_entry_keeps_slot_unpublished() {
        let api = test_api().await;
        insert_ticket(&api, "DES-1").await;
        let (slot, _) = api
            .create_slot(slot_create("DES-1", 100, 160))
            .await
            .unwrap();
        let connection_id = api.config.core.active_connection.unwrap();

        api.mark_pending(connection_id, [slot.id]).await.unwrap();

        let entries = publish_entries(&api, slot.id).await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].published_at, None);
        assert_eq!(entries[0].error, None);
        assert_eq!(unpublished_ids(&api).await, [slot.id]);
    }

    #[tokio::test]
    async fn deletes_published_slot_with_its_entries() {
        let api = test_api().await;
        insert_ticket(&api, "DES-1").await;
        let (slot, _) = api
            .create_slot(slot_create("DES-1", 100, 160))
            .await
            .unwrap();
        publish(&api, &[slot.id], Ok(())).await;

        api.delete_slot(slot.id).await.unwrap();

        assert!(publish_entries(&api, slot.id).await.is_empty());
    }
}
