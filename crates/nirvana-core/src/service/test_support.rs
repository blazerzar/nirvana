use std::ops::{Deref, DerefMut};

use nirvana_entity::credential::SecretStore;
use nirvana_entity::profile;
use sea_orm::{ActiveModelTrait, Set};
use tempfile::TempDir;

use crate::config::AppConfig;
use crate::domain::ConnectionData;
use crate::paths::AppPaths;
use crate::secrets::mock_store;
use crate::service::Nirvana;
use crate::storage;

pub(super) struct TestService {
    service: Nirvana,
    _dir: TempDir,
}

impl Deref for TestService {
    type Target = Nirvana;

    fn deref(&self) -> &Self::Target {
        &self.service
    }
}

impl DerefMut for TestService {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.service
    }
}

pub(super) async fn test_service() -> TestService {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::in_dir(dir.path());
    let db = storage::open(&paths.db_file).await.unwrap();
    let mut service = Nirvana {
        paths,
        config: Ok(AppConfig::default()),
        db,
        keyring: mock_store(),
    };
    profile::ActiveModel {
        name: Set("test".to_string()),
        ..Default::default()
    }
    .insert(&service.db)
    .await
    .unwrap();
    service
        .add_connection(ConnectionData {
            name: "Test Jira".to_string(),
            kind: "jira-cloud".to_string(),
            host: "https://example.atlassian.net".to_string(),
            identity: "tester@example.com".to_string(),
            secret_store: SecretStore::Plaintext,
            token: "token".into(),
        })
        .await
        .unwrap();
    let config = service.config.as_mut().unwrap();
    config.core.profile = Some("test".to_string());
    config.idle.enabled = false;

    TestService { service, _dir: dir }
}
