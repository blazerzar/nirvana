use std::path::{Path, PathBuf};

use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr, RuntimeErr, raw_sql};
use thiserror::Error;
use tracing::info;

use nirvana_migration::{Migrator, MigratorTrait};

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
