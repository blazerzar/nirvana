use std::path::{Path, PathBuf};

use nirvana_migration::{Migrator, MigratorTrait};
use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr, RuntimeErr, raw_sql};
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Db(#[from] sea_orm::DbErr),
    #[error("db I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("duplicate connection name: '{0}'")]
    DuplicateName(String),
    #[error("stop time must be after the slot start time")]
    StopBeforeStart,
    #[error("invalid db path")]
    InvalidDbPath,
}

/// Opens the SQLite database at `db_file`, creating it if missing, and applies
/// pending migrations. An existing database is backed up to `backup/` next to
/// it before migrating.
pub(crate) async fn open(db_file: &Path) -> Result<DatabaseConnection, DbError> {
    let exists = db_file.exists();

    let parent = db_file.parent().ok_or(DbError::InvalidDbPath)?;
    std::fs::create_dir_all(parent)?;

    let options = SqliteConnectOptions::new()
        .filename(db_file)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|e| DbErr::Conn(RuntimeErr::SqlxError(e.into())))?;
    let db = DatabaseConnection::from(pool);

    let pending = Migrator::get_pending_migrations_read_only(&db).await?;
    if !pending.is_empty() {
        info!("found {} pending migrations", pending.len());
        // Only backup existing databases
        if exists {
            let backup_path = backup(&db, db_file).await?;
            info!("db backed up to {}", backup_path.display());
        }
        Migrator::up(&db, None).await?;
        info!("db migration complete");
    }

    Ok(db)
}

async fn backup(db: &DatabaseConnection, db_file: &Path) -> Result<PathBuf, DbError> {
    // nirvana.db -> backup/nirvana.db.20261019-203400.bak
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let mut name = db_file
        .file_name()
        .ok_or(DbError::InvalidDbPath)?
        .to_os_string();
    name.push(format!(".{stamp}.bak"));

    let backup_dir = db_file
        .parent()
        .ok_or(DbError::InvalidDbPath)?
        .join("backup");
    std::fs::create_dir_all(&backup_dir)?;
    let backup_path = backup_dir.join(name);

    let p = backup_path.to_str().ok_or(DbError::InvalidDbPath)?;
    db.execute_raw(raw_sql!(Sqlite, "VACUUM INTO {p}")).await?;

    Ok(backup_path)
}

#[cfg(test)]
mod tests {
    use nirvana_entity::prelude::*;
    use nirvana_entity::{connection, credential, jira_settings, profile, ticket};
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{ActiveModelTrait, DbBackend, EntityTrait, PaginatorTrait, Statement};

    use super::*;

    const FIXTURE_030: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/v0_3_0/nirvana_030.db"
    );

    /// Migrates a copy of the 0.3 fixture and attaches an untouched copy as `old`.
    async fn migrate_fixture(dir: &Path) -> DatabaseConnection {
        let db_file = dir.join("nirvana.db");
        let old_file = dir.join("old.db");
        std::fs::copy(FIXTURE_030, &db_file).unwrap();
        std::fs::copy(FIXTURE_030, &old_file).unwrap();

        let db = open(&db_file).await.unwrap();
        db.execute_unprepared(&format!("attach '{}' as old", old_file.display()))
            .await
            .unwrap();
        db
    }

    async fn text_rows(db: &DatabaseConnection, sql: &str) -> Vec<String> {
        db.query_all_raw(Statement::from_string(DbBackend::Sqlite, sql))
            .await
            .unwrap()
            .iter()
            .map(|row| row.try_get_by_index(0).unwrap())
            .collect()
    }

    /// Columns, foreign keys and indexes of every app table, one row each.
    async fn schema(db: &DatabaseConnection) -> Vec<String> {
        const QUERIES: [&str; 3] = [
            "select m.name || '|' || p.name || '|' || p.type || '|' || p.\"notnull\"
                 || '|' || coalesce(p.dflt_value, '') || '|' || p.pk
             from main.sqlite_master m, pragma_table_info(m.name, 'main') p
             where m.type = 'table' and m.name not like 'seaql%'
             order by m.name, p.cid",
            "select m.name || '|' || p.\"from\" || '|' || p.\"table\" || '|' || p.\"to\"
                 || '|' || p.on_update || '|' || p.on_delete
             from main.sqlite_master m, pragma_foreign_key_list(m.name, 'main') p
             where m.type = 'table' and m.name not like 'seaql%'
             order by m.name, p.id, p.seq",
            "select m.name || '|' || i.name || '|' || i.\"unique\" || '|' || i.partial
                 || '|' || c.seqno || '|' || coalesce(c.name, '')
             from main.sqlite_master m, pragma_index_list(m.name, 'main') i,
                 pragma_index_info(i.name, 'main') c
             where m.type = 'table' and m.name not like 'seaql%'
             order by m.name, i.name, c.seqno",
        ];

        let mut rows = Vec::new();
        for sql in QUERIES {
            rows.extend(text_rows(db, sql).await);
        }
        rows
    }

    #[tokio::test]
    async fn migrates_030_db_without_data_loss() {
        let dir = tempfile::tempdir().unwrap();
        let db = migrate_fixture(dir.path()).await;

        let tables = text_rows(
            &db,
            "select name from old.sqlite_master where type = 'table'",
        )
        .await;
        assert!(!tables.is_empty());
        for table in tables {
            let columns = text_rows(
                &db,
                &format!(
                    "select name from pragma_table_info('{table}', 'old')
                     where not ('{table}' = 'slots' and name in ('note', 'published_at'))
                         and not ('{table}' = 'tickets' and name = 'summary')"
                ),
            )
            .await
            .join(", ");
            let rows = |schema: &str| {
                format!("select json_array({columns}) from {schema}.{table} order by rowid")
            };
            assert_eq!(
                text_rows(&db, &rows("main")).await,
                text_rows(&db, &rows("old")).await,
                "{table} rows changed"
            );
        }

        let profiles = Profile::find().all(&db).await.unwrap();
        assert_eq!(
            profiles.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["default"]
        );
        let connections = Connection::find().all(&db).await.unwrap();
        assert!(connections.iter().all(|c| c.profile_id == profiles[0].id));

        assert!(
            text_rows(&db, "select \"table\" from pragma_foreign_key_check")
                .await
                .is_empty()
        );
        assert_eq!(
            text_rows(
                &db,
                "select cast(foreign_keys as text) from pragma_foreign_keys"
            )
            .await,
            ["1"]
        );
    }

    #[tokio::test]
    async fn migrated_030_tickets_get_default_profile_and_title() {
        let dir = tempfile::tempdir().unwrap();
        let db = migrate_fixture(dir.path()).await;

        let profile_id = Profile::find().one(&db).await.unwrap().unwrap().id;
        let tickets = text_rows(
            &db,
            &format!(
                "select json_array(t.id, t.profile_id = {profile_id}, t.title is o.summary,
                     t.connection_id is o.connection_id, t.synced_at = o.last_worked_at)
                 from main.tickets t join old.tickets o on o.id = t.id order by t.id"
            ),
        )
        .await;
        let expected = text_rows(
            &db,
            "select json_array(id, 1, 1, 1, 1) from old.tickets order by id",
        )
        .await;
        assert!(!expected.is_empty());
        assert_eq!(tickets, expected);
        assert_eq!(
            text_rows(&db, "select cast(count(*) as text) from main.slots").await,
            text_rows(&db, "select cast(count(*) as text) from old.slots").await
        );
    }

    #[tokio::test]
    async fn migrated_030_db_matches_fresh_schema() {
        let migrated_dir = tempfile::tempdir().unwrap();
        let migrated = migrate_fixture(migrated_dir.path()).await;
        let fresh_dir = tempfile::tempdir().unwrap();
        let fresh = open(&fresh_dir.path().join("nirvana.db")).await.unwrap();

        let migrated_schema = schema(&migrated).await;
        assert!(migrated_schema.contains(&"connections|profile_id|INTEGER|1||0".to_string()));
        assert!(migrated_schema.contains(&"tickets|profile_id|INTEGER|1||0".to_string()));
        assert!(migrated_schema.contains(&"tickets|connection_id|INTEGER|0||0".to_string()));
        assert!(migrated_schema.contains(&"tickets|title|TEXT|0||0".to_string()));
        assert!(migrated_schema.contains(&"tickets|synced_at|INTEGER|1||0".to_string()));
        assert!(
            migrated_schema
                .contains(&"connections|profile_id|profiles|id|NO ACTION|CASCADE".to_string())
        );
        assert!(
            migrated_schema
                .contains(&"tickets|profile_id|profiles|id|NO ACTION|CASCADE".to_string())
        );
        assert!(
            migrated_schema
                .contains(&"tickets|tickets_by_last_worked|0|0|0|profile_id".to_string())
        );
        assert!(
            migrated_schema
                .contains(&"tickets|tickets_by_profile_key|0|0|1|ticket_key".to_string())
        );
        assert!(
            !migrated_schema
                .iter()
                .any(|row| row.starts_with("tickets|sqlite_autoindex"))
        );
        assert!(migrated_schema.contains(&"slots|uuid|BLOB|1||0".to_string()));
        assert!(migrated_schema.contains(&"slots|slots_by_uuid|1|0|0|uuid".to_string()));
        assert!(
            migrated_schema.contains(
                &"jira_settings|connection_id|connections|id|NO ACTION|CASCADE".to_string()
            )
        );
        assert!(
            migrated_schema
                .contains(&"publish_entries|slot_id|slots|id|NO ACTION|CASCADE".to_string())
        );
        assert!(migrated_schema.contains(
            &"publish_entries|connection_id|connections|id|NO ACTION|NO ACTION".to_string()
        ));
        assert!(
            !migrated_schema
                .iter()
                .any(|row| row.contains("slots_unpublished"))
        );
        assert_eq!(schema(&fresh).await, migrated_schema);
        assert_eq!(Profile::find().count(&fresh).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn migrated_030_slots_get_distinct_blob_uuids() {
        let dir = tempfile::tempdir().unwrap();
        let db = migrate_fixture(dir.path()).await;

        let counts = text_rows(
            &db,
            "select count(*) || '|' || count(distinct uuid) || '|' || count(*) filter
                 (where typeof(uuid) = 'blob' and length(uuid) = 16)
             from slots",
        )
        .await;
        let slots = Slot::find().all(&db).await.unwrap().len();
        assert!(slots > 0);
        assert_eq!(counts, [format!("{slots}|{slots}|{slots}")]);
    }

    #[tokio::test]
    async fn migrated_030_published_slots_get_ok_publish_entries() {
        let dir = tempfile::tempdir().unwrap();
        let db = migrate_fixture(dir.path()).await;

        let entries = text_rows(
            &db,
            "select slot_id || '|' || connection_id || '|' || published_at || '|'
                 || coalesce(error, 'null')
             from main.publish_entries order by slot_id",
        )
        .await;
        let published = text_rows(
            &db,
            "select id || '|' || connection_id || '|' || published_at || '|null'
             from old.slots where published_at is not null order by id",
        )
        .await;
        assert!(!published.is_empty());
        assert_eq!(entries, published);
    }

    #[tokio::test]
    async fn migrated_030_db_requires_existing_profile_and_unique_name() {
        let dir = tempfile::tempdir().unwrap();
        let db = migrate_fixture(dir.path()).await;
        let profile_id = Profile::find().one(&db).await.unwrap().unwrap().id;
        let insert = |name: &str, profile_id| {
            connection::ActiveModel {
                name: Set(name.to_string()),
                kind: Set("jira-cloud".to_string()),
                host: Set("host".to_string()),
                identity: Set("identity".to_string()),
                secret_store: Set("plaintext".to_string()),
                profile_id,
                ..Default::default()
            }
            .insert(&db)
        };

        let error = |result: Result<connection::Model, DbErr>| result.unwrap_err().to_string();
        assert!(error(insert("new", NotSet).await).contains("NOT NULL"));
        assert!(error(insert("new", Set(profile_id + 1)).await).contains("FOREIGN KEY"));
        assert!(error(insert("work", Set(profile_id)).await).contains("UNIQUE"));
    }

    #[tokio::test]
    async fn migrated_030_jira_connections_publish_squashed() {
        let dir = tempfile::tempdir().unwrap();
        let db = migrate_fixture(dir.path()).await;

        let connections = Connection::find()
            .find_also_related(JiraSettings)
            .all(&db)
            .await
            .unwrap();

        assert_eq!(
            connections
                .iter()
                .map(|(c, _)| c.kind.as_str())
                .collect::<Vec<_>>(),
            ["jira-cloud", "jira-dc"]
        );
        for (connection, settings) in connections {
            let settings = settings.unwrap();
            assert_eq!(settings.connection_id, connection.id);
            assert!(settings.publish);
            assert!(settings.squash);
        }
    }

    #[tokio::test]
    async fn jira_settings_default_and_cascade_on_delete() {
        let dir = tempfile::tempdir().unwrap();
        let db = open(&dir.path().join("nirvana.db")).await.unwrap();
        let profile = profile::ActiveModel {
            name: Set("default".to_string()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        let connection = connection::ActiveModel {
            name: Set("work".to_string()),
            kind: Set("jira-cloud".to_string()),
            host: Set("host".to_string()),
            identity: Set("identity".to_string()),
            secret_store: Set("plaintext".to_string()),
            profile_id: Set(profile.id),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        let settings = jira_settings::ActiveModel {
            connection_id: Set(connection.id),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        assert!(settings.publish);
        assert!(!settings.squash);

        Connection::delete_by_id(connection.id)
            .exec(&db)
            .await
            .unwrap();

        assert_eq!(JiraSettings::find().count(&db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn profile_delete_cascades() {
        let dir = tempfile::tempdir().unwrap();
        let db = open(&dir.path().join("nirvana.db")).await.unwrap();
        let profile = profile::ActiveModel {
            name: Set("default".to_string()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        let connection = connection::ActiveModel {
            name: Set("work".to_string()),
            kind: Set("jira-cloud".to_string()),
            host: Set("host".to_string()),
            identity: Set("identity".to_string()),
            secret_store: Set("plaintext".to_string()),
            profile_id: Set(profile.id),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        credential::ActiveModel {
            connection_id: Set(connection.id),
            credential: Set("token".to_string()),
        }
        .insert(&db)
        .await
        .unwrap();
        jira_settings::ActiveModel {
            connection_id: Set(connection.id),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        ticket::ActiveModel {
            profile_id: Set(profile.id),
            connection_id: Set(Some(connection.id)),
            ticket_key: Set("KEY-1".to_string()),
            synced_at: Set(0),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();

        Profile::delete_by_id(profile.id).exec(&db).await.unwrap();

        assert_eq!(Connection::find().count(&db).await.unwrap(), 0);
        assert_eq!(Credential::find().count(&db).await.unwrap(), 0);
        assert_eq!(JiraSettings::find().count(&db).await.unwrap(), 0);
        assert_eq!(Ticket::find().count(&db).await.unwrap(), 0);
    }
}
