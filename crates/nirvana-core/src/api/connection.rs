use crate::api::NirvanaApi;
use crate::api::NirvanaError;
use crate::api::domain::{Connection, ConnectionData};
use crate::api::errors::TrackingError;
use crate::credentials;
use crate::integration;
use crate::storage::connection_repo;

impl NirvanaApi {
    pub fn list_connections(&self) -> Result<Vec<Connection>, NirvanaError> {
        let records = connection_repo::list(&self.db)?;
        Ok(records.into_iter().map(Connection::from).collect())
    }

    pub fn get_active_connection(&self) -> Result<Option<Connection>, NirvanaError> {
        let Some(connection_id) = self.config.active_connection else {
            return Ok(None);
        };

        Ok(Some(self.get_connection(connection_id)?))
    }

    pub fn active_connection(&self) -> Option<i64> {
        self.config.active_connection
    }

    pub fn set_active_connection(&mut self, id: i64) -> Result<(), NirvanaError> {
        if !connection_repo::exists(&self.db, id)? {
            return Err(TrackingError::ConnectionNotFound(id).into());
        }

        self.config.active_connection = Some(id);
        self.config.save(&self.paths)?;
        Ok(())
    }

    pub fn delete_connection(&mut self, id: i64) -> Result<(), NirvanaError> {
        if !connection_repo::exists(&self.db, id)? {
            return Err(TrackingError::ConnectionNotFound(id).into());
        }

        if connection_repo::has_history(&self.db, id)? {
            return Err(TrackingError::ConnectionHasHistory.into());
        }

        connection_repo::delete(&self.db, id)?;

        if self.config.active_connection == Some(id) {
            self.config.active_connection = connection_repo::list(&self.db)?
                .into_iter()
                .map(|connection| connection.id)
                .min();
            self.config.save(&self.paths)?;
        }

        Ok(())
    }

    pub fn add_connection(
        &self,
        mut connection: ConnectionData,
    ) -> Result<Connection, NirvanaError> {
        connection.host = normalize_host(&connection.host);
        let record = connection_repo::add(&self.db, connection)?;
        Ok(record.into())
    }

    pub fn test_connection_data(&self, mut connection: ConnectionData) -> Result<(), NirvanaError> {
        connection.host = normalize_host(&connection.host);
        let candidate = Connection {
            id: 0,
            name: connection.name,
            kind: connection.kind,
            host: connection.host,
            identity: connection.identity,
            secret_store: connection.secret_store,
            created_at: 0,
            updated_at: 0,
        };
        let integ = integration::build_integration(&candidate, &connection.token)?;
        integ.test_connection()?;
        Ok(())
    }

    pub fn test_connection(&self) -> Result<(), NirvanaError> {
        let connection_id = self
            .config
            .active_connection
            .ok_or(TrackingError::NoActiveConnection)?;

        let connection = self.get_connection(connection_id)?;
        let token = credentials::get_token(&self.db, connection_id)?;
        let integ = integration::build_integration(&connection, &token)?;
        integ.test_connection()?;
        Ok(())
    }
}

fn normalize_host(url: &str) -> String {
    let s = url.trim();
    let s = s.strip_prefix("https://").unwrap_or(s);
    let s = s.strip_prefix("http://").unwrap_or(s);
    let s = s.strip_suffix('/').unwrap_or(s);
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::errors::TrackingError;
    use crate::config::AppConfig;
    use crate::paths::AppPaths;
    use crate::storage::{Database, connection_repo, ticket_repo};
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

    impl TestApi {
        fn api_mut(&mut self) -> &mut NirvanaApi {
            self.api.as_mut().unwrap()
        }
    }

    fn test_api() -> TestApi {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "nirvana-connection-test-{}-{suffix}-{counter}",
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

        TestApi {
            api: Some(NirvanaApi {
                paths,
                config: AppConfig::default(),
                db,
            }),
            base,
        }
    }

    fn add_connection(api: &NirvanaApi, name: &str) -> i64 {
        connection_repo::add(
            &api.db,
            ConnectionData {
                name: name.to_string(),
                kind: "jira-cloud".to_string(),
                host: format!("{name}.atlassian.net"),
                identity: "tester@example.com".to_string(),
                secret_store: "plaintext".to_string(),
                token: "token".to_string(),
            },
        )
        .unwrap()
        .id
    }

    #[test]
    fn deletes_unused_connection_and_credentials() {
        let mut test = test_api();
        let id = add_connection(&test, "work");

        test.api_mut().delete_connection(id).unwrap();

        assert!(!connection_repo::exists(&test.db, id).unwrap());
        let credential_count: i64 = test
            .db
            .conn()
            .query_row(
                "select count(*) from credentials where connection_id = ?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(credential_count, 0);
    }

    #[test]
    fn blocks_delete_when_connection_has_history() {
        let mut test = test_api();
        let id = add_connection(&test, "work");
        ticket_repo::insert(&test.db, "NIR-1", Some("Tracked work"), id, 100).unwrap();

        let error = test.api_mut().delete_connection(id).unwrap_err();

        assert!(matches!(
            error,
            NirvanaError::Tracking(TrackingError::ConnectionHasHistory)
        ));
        assert!(connection_repo::exists(&test.db, id).unwrap());
    }

    #[test]
    fn deleting_active_unused_connection_chooses_next_lowest_connection() {
        let mut test = test_api();
        let first = add_connection(&test, "first");
        let second = add_connection(&test, "second");
        test.api_mut().set_active_connection(first).unwrap();

        test.api_mut().delete_connection(first).unwrap();

        assert_eq!(test.api_mut().active_connection(), Some(second));
    }

    #[test]
    fn deleting_last_active_unused_connection_clears_active_connection() {
        let mut test = test_api();
        let id = add_connection(&test, "work");
        test.api_mut().set_active_connection(id).unwrap();

        test.api_mut().delete_connection(id).unwrap();

        assert_eq!(test.api_mut().active_connection(), None);
    }

    #[test]
    fn set_active_connection_rejects_unknown_id() {
        let mut test = test_api();

        let error = test.api_mut().set_active_connection(404).unwrap_err();

        assert!(matches!(
            error,
            NirvanaError::Tracking(TrackingError::ConnectionNotFound(404))
        ));
    }
}
