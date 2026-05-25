use crate::api::NirvanaApi;
use crate::api::domain::{Connection, Slot, SlotCreate, Ticket};
use crate::api::errors::{IntegrationError, TrackingError};
use crate::credentials;
use crate::integration;
use crate::storage::ticket_repo::TicketRecord;
use crate::storage::{DbError, connection_repo, slot_repo, ticket_repo};

impl NirvanaApi {
    pub fn start_slot(
        &self,
        ticket_key: &str,
        at: Option<i64>,
        note: Option<&str>,
    ) -> Result<Slot, super::NirvanaError> {
        let connection_id = self
            .config
            .core
            .active_connection
            .ok_or(TrackingError::NoActiveConnection)?;

        let connection = self.get_connection(connection_id)?;
        let ticket_key = ticket_key.to_uppercase();
        let now = at.unwrap_or_else(|| chrono::Utc::now().timestamp());

        let ticket = self.resolve_ticket_for_work(&connection, connection_id, &ticket_key, now)?;

        if let Some(running) = slot_repo::find_running(&self.db)? {
            let stopped_at = std::cmp::max(running.started_at + 1, now);
            slot_repo::stop_by_id(&self.db, running.id, stopped_at)?;
        }

        let result = slot_repo::insert(&self.db, ticket.id, connection_id, note, now)?;
        let integ = integration::Integration::build_for_url(&connection)?;
        let issue_url = Some(integ.get_issue_link(&result.ticket_key));
        Ok(Slot::from_record(result, issue_url))
    }

    pub fn create_slot(&self, input: SlotCreate) -> Result<Slot, super::NirvanaError> {
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

        if slot_repo::overlaps(&self.db, connection_id, input.started_at, input.stopped_at)? {
            return Err(TrackingError::SlotOverlap.into());
        }

        let connection = self.get_connection(connection_id)?;
        let ticket_key = input.ticket_key.to_uppercase();
        let ticket = self.resolve_ticket_for_work(
            &connection,
            connection_id,
            &ticket_key,
            input.stopped_at,
        )?;

        let result = slot_repo::insert_completed(
            &self.db,
            ticket.id,
            connection_id,
            input.note.as_deref(),
            input.started_at,
            input.stopped_at,
        )?;
        let integ = integration::Integration::build_for_url(&connection)?;
        let issue_url = Some(integ.get_issue_link(&result.ticket_key));
        Ok(Slot::from_record(result, issue_url))
    }

    pub fn stop_slot(&self, at: Option<i64>) -> Result<Option<Slot>, super::NirvanaError> {
        let now = at.unwrap_or_else(|| chrono::Utc::now().timestamp());
        match slot_repo::stop_running(&self.db, now) {
            Ok(slot) => {
                let connection = self.get_connection(slot.connection_id)?;
                let integ = integration::Integration::build_for_url(&connection)?;
                let issue_url = Some(integ.get_issue_link(&slot.ticket_key));
                let mut s = Slot::from_record(slot, issue_url);
                s.stopped_at = Some(now);
                Ok(Some(s))
            }
            Err(DbError::Sqlite(rusqlite::Error::QueryReturnedNoRows)) => Ok(None),
            Err(e) => Err(super::NirvanaError::Db(e)),
        }
    }

    pub(crate) fn get_connection(&self, id: i64) -> Result<Connection, super::NirvanaError> {
        let records = connection_repo::list(&self.db)?;
        records
            .into_iter()
            .find(|r| r.id == id)
            .map(Connection::from)
            .ok_or(super::NirvanaError::Tracking(
                TrackingError::NoActiveConnection,
            ))
    }

    fn resolve_ticket_for_work(
        &self,
        connection: &Connection,
        connection_id: i64,
        ticket_key: &str,
        last_worked_at: i64,
    ) -> Result<TicketRecord, super::NirvanaError> {
        match ticket_repo::find_by_key(&self.db, ticket_key, connection_id)? {
            Some(t) => {
                ticket_repo::touch_last_worked(&self.db, t.id, last_worked_at)?;
                Ok(t)
            }
            None => {
                let token = credentials::get_token(&self.db, connection_id)?;
                let integ = integration::build_integration(connection, &token)?;
                let issue = integ.fetch_issue(ticket_key).map_err(|e| match e {
                    IntegrationError::TicketNotFound(key) => {
                        super::NirvanaError::Tracking(TrackingError::TicketNotFound(key))
                    }
                    other => super::NirvanaError::Integration(other),
                })?;
                Ok(ticket_repo::insert(
                    &self.db,
                    ticket_key,
                    Some(&issue.summary),
                    connection_id,
                    last_worked_at,
                )?)
            }
        }
    }

    pub fn list_recent_tickets(&self) -> Result<Vec<Ticket>, super::NirvanaError> {
        let connection_id = self
            .config
            .core
            .active_connection
            .ok_or(TrackingError::NoActiveConnection)?;
        let records = ticket_repo::list_by_connection(&self.db, connection_id)?;
        Ok(records.into_iter().map(Ticket::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::NirvanaError;
    use crate::api::domain::ConnectionData;
    use crate::config::{AppConfig, CoreConfig, GuiConfig};
    use crate::paths::AppPaths;
    use crate::storage::{Database, slot_repo::SlotSort};
    use std::ops::Deref;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_DB_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TestApi {
        api: Option<NirvanaApi>,
        base: PathBuf,
    }

    impl Deref for TestApi {
        type Target = NirvanaApi;

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

    fn test_api() -> TestApi {
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
        let db = Database::initialize(&paths.db_file).unwrap();
        let connection = connection_repo::add(
            &db,
            ConnectionData {
                name: "Test Jira".to_string(),
                kind: "jira-cloud".to_string(),
                host: "https://example.atlassian.net".to_string(),
                identity: "tester@example.com".to_string(),
                secret_store: "plaintext".to_string(),
                token: "token".to_string(),
            },
        )
        .unwrap();

        TestApi {
            api: Some(NirvanaApi {
                paths,
                config: AppConfig {
                    schema_version: Some(1),
                    core: CoreConfig {
                        active_connection: Some(connection.id),
                        publish_squashed_worklogs: true,
                    },
                    gui: GuiConfig {
                        font_scale: 1.0,
                        theme: "high-contrast-dark".to_string(),
                        show_tray_icon: false,
                    },
                },
                db,
            }),
            base,
        }
    }

    fn insert_ticket(api: &NirvanaApi, key: &str) -> i64 {
        let connection_id = api.config.core.active_connection.unwrap();
        ticket_repo::insert(&api.db, key, Some("Existing ticket"), connection_id, 0)
            .unwrap()
            .id
    }

    fn slot_create(ticket_key: &str, started_at: i64, stopped_at: i64) -> SlotCreate {
        SlotCreate {
            ticket_key: ticket_key.to_string(),
            note: Some("manual entry".to_string()),
            started_at,
            stopped_at,
        }
    }

    #[test]
    fn creates_completed_unpublished_slot() {
        let api = test_api();
        insert_ticket(&api, "DES-1");

        let slot = api.create_slot(slot_create("des-1", 100, 160)).unwrap();

        assert_eq!(slot.ticket_key, "DES-1");
        assert_eq!(slot.note.as_deref(), Some("manual entry"));
        assert_eq!(slot.started_at, 100);
        assert_eq!(slot.stopped_at, Some(160));
        assert_eq!(slot.published_at, None);
    }

    #[test]
    fn rejects_invalid_time_range() {
        let api = test_api();

        let error = api.create_slot(slot_create("DES-1", 160, 160)).unwrap_err();

        assert!(matches!(
            error,
            NirvanaError::Tracking(TrackingError::InvalidTimeRange)
        ));
    }

    #[test]
    fn rejects_future_times() {
        let api = test_api();
        let future = chrono::Utc::now().timestamp() + 60;

        let error = api
            .create_slot(slot_create("DES-1", future - 10, future))
            .unwrap_err();

        assert!(matches!(
            error,
            NirvanaError::Tracking(TrackingError::FutureTime)
        ));
    }

    #[test]
    fn rejects_overlapping_slot() {
        let api = test_api();
        insert_ticket(&api, "DES-1");
        api.create_slot(slot_create("DES-1", 100, 160)).unwrap();

        let error = api.create_slot(slot_create("DES-1", 150, 200)).unwrap_err();

        assert!(matches!(
            error,
            NirvanaError::Tracking(TrackingError::SlotOverlap)
        ));
    }

    #[test]
    fn does_not_stop_existing_running_slot() {
        let api = test_api();
        let ticket_id = insert_ticket(&api, "DES-1");
        let connection_id = api.config.core.active_connection.unwrap();
        let running = slot_repo::insert(&api.db, ticket_id, connection_id, None, 300).unwrap();

        let slot = api.create_slot(slot_create("DES-1", 100, 160)).unwrap();
        let still_running = slot_repo::find_running(&api.db).unwrap().unwrap();

        assert_eq!(slot.stopped_at, Some(160));
        assert_eq!(still_running.id, running.id);
        assert_eq!(still_running.started_at, 300);
        assert_eq!(still_running.stopped_at, None);
    }

    #[test]
    fn lists_slots_that_overlap_bounded_range() {
        let api = test_api();
        insert_ticket(&api, "DES-1");
        insert_ticket(&api, "DES-2");
        insert_ticket(&api, "DES-3");
        api.create_slot(slot_create("DES-1", 100, 140)).unwrap();
        api.create_slot(slot_create("DES-2", 140, 220)).unwrap();
        api.create_slot(slot_create("DES-3", 260, 320)).unwrap();

        let slots = api.get_slots(150, Some(260), SlotSort::StartedAt).unwrap();

        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].ticket_key, "DES-2");
        assert_eq!(slots[0].started_at, 140);
        assert_eq!(slots[0].stopped_at, Some(220));
    }

    #[test]
    fn lists_open_ended_slots_that_overlap_range_start() {
        let api = test_api();
        insert_ticket(&api, "DES-1");
        insert_ticket(&api, "DES-2");
        api.create_slot(slot_create("DES-1", 100, 140)).unwrap();
        api.create_slot(slot_create("DES-2", 140, 220)).unwrap();

        let slots = api.get_slots(200, None, SlotSort::StartedAt).unwrap();

        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].ticket_key, "DES-2");
        assert_eq!(slots[0].started_at, 140);
        assert_eq!(slots[0].stopped_at, Some(220));
    }
}
