use std::{fs, path::Path};

use rusqlite::Connection;
use thiserror::Error;

pub mod connection_repo;

pub struct Database {
    connection: Connection,
}

#[derive(Debug, Error)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("db I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl Database {
    pub(crate) fn conn(&self) -> &Connection {
        &self.connection
    }

    pub fn initialize(path: &Path) -> Result<Self, DbError> {
        let db = Database::open(path)?;
        db.run_migrations()?;
        Ok(db)
    }

    pub fn open(path: &Path) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

        Ok(Self { connection: conn })
    }

    pub fn run_migrations(&self) -> Result<(), DbError> {
        const MIGRATIONS: &[(i64, &str)] = &[(1, include_str!("migrations/0001_init.sql"))];

        let user_version: i64 = self.connection.query_row(
            "select user_version from pragma_user_version",
            [],
            |row| row.get(0),
        )?;

        for &(version, sql) in MIGRATIONS {
            if user_version < version {
                self.connection.execute_batch(sql)?;
                self.connection
                    .pragma_update(None, "user_version", version)?;
            }
        }

        Ok(())
    }
}
