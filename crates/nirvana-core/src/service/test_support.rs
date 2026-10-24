use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use nirvana_entity::profile;
use sea_orm::{ActiveModelTrait, Set};

use crate::config::{AppConfig, CoreConfig, GuiConfig, IdleConfig};
use crate::domain::ConnectionData;
use crate::paths::AppPaths;
use crate::service::Nirvana;
use crate::storage;

static TEST_DB_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub(super) struct TestApi {
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

pub(super) async fn test_api() -> TestApi {
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
