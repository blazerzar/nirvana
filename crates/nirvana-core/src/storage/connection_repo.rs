use crate::api::domain::ConnectionData;
use crate::storage::{Database, DbError};

pub(crate) struct ConnectionRecord {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub host: String,
    pub identity: String,
    pub secret_store: String,
    pub created_at: i64,
    pub updated_at: i64,
}

pub(crate) fn list(db: &Database) -> Result<Vec<ConnectionRecord>, DbError> {
    let mut stmt = db.conn().prepare(
        "select id, name, kind, host, identity, secret_store, created_at, updated_at
         from connections
         order by id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ConnectionRecord {
            id: row.get(0)?,
            name: row.get(1)?,
            kind: row.get(2)?,
            host: row.get(3)?,
            identity: row.get(4)?,
            secret_store: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    })?;
    rows.collect::<Result<_, _>>().map_err(DbError::from)
}

pub(crate) fn exists(db: &Database, id: i64) -> Result<bool, DbError> {
    db.conn()
        .query_row(
            "select exists(select 1 from connections where id = ?1)",
            [id],
            |row| row.get(0),
        )
        .map_err(DbError::from)
}

pub(crate) fn has_history(db: &Database, id: i64) -> Result<bool, DbError> {
    let ticket_count: i64 = db.conn().query_row(
        "select count(*) from tickets where connection_id = ?1",
        [id],
        |row| row.get(0),
    )?;
    if ticket_count > 0 {
        return Ok(true);
    }

    let slot_count: i64 = db.conn().query_row(
        "select count(*) from slots where connection_id = ?1",
        [id],
        |row| row.get(0),
    )?;
    Ok(slot_count > 0)
}

pub(crate) fn delete(db: &Database, id: i64) -> Result<bool, DbError> {
    let affected = db
        .conn()
        .execute("delete from connections where id = ?1", [id])?;
    Ok(affected > 0)
}

pub(crate) fn add(db: &Database, data: ConnectionData) -> Result<ConnectionRecord, DbError> {
    if data.secret_store != "plaintext" {
        unimplemented!("only plaintext secret store is currently supported");
    }

    let exists: bool = db.conn().query_row(
        "select exists(select 1 from connections where name = ?1)",
        [&data.name],
        |row| row.get(0),
    )?;
    if exists {
        return Err(DbError::DuplicateName(data.name));
    }

    let now = chrono::Utc::now().timestamp();

    let tx = db.conn().unchecked_transaction()?;

    tx.execute(
        "insert into connections (name, kind, host, identity, secret_store, created_at, updated_at)
         values (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        (
            &data.name,
            &data.kind,
            &data.host,
            &data.identity,
            &data.secret_store,
            now,
            now,
        ),
    )?;

    let id = tx.last_insert_rowid();

    tx.execute(
        "insert into credentials (connection_id, credential) values (?1, ?2)",
        (id, &data.token),
    )?;

    tx.commit()?;

    Ok(ConnectionRecord {
        id,
        name: data.name,
        kind: data.kind,
        host: data.host,
        identity: data.identity,
        secret_store: data.secret_store,
        created_at: now,
        updated_at: now,
    })
}
