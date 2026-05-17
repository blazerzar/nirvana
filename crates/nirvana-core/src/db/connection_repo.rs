use crate::db::{Database, DbError};
use crate::domain::Connection;

pub fn list(db: &Database) -> Result<Vec<Connection>, DbError> {
    let mut stmt = db.conn().prepare(
        "select id, name, kind, base_url, identity, secret_store,
                created_at, updated_at
         from connections
         order by id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Connection {
            id: row.get(0)?,
            name: row.get(1)?,
            kind: row.get(2)?,
            base_url: row.get(3)?,
            identity: row.get(4)?,
            secret_store: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    })?;
    rows.collect::<Result<_, _>>().map_err(DbError::from)
}
