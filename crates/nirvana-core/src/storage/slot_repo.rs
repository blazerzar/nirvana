use crate::storage::{Database, DbError};

pub enum SlotSort {
    StartedAt,
    TicketId,
}

#[derive(Debug, Default)]
pub enum Change<T> {
    #[default]
    Skip,
    Clear,
    Set(T),
}

#[allow(dead_code)]
pub(crate) struct SlotRecord {
    pub id: i64,
    pub ticket_id: i64,
    pub connection_id: i64,
    pub note: Option<String>,
    pub started_at: i64,
    pub stopped_at: Option<i64>,
    pub published_at: Option<i64>,
}

#[allow(dead_code)]
pub(crate) struct SlotWithTicket {
    pub id: i64,
    pub ticket_key: String,
    pub summary: Option<String>,
    pub connection_id: i64,
    pub note: Option<String>,
    pub started_at: i64,
    pub stopped_at: Option<i64>,
    pub published_at: Option<i64>,
}

pub(crate) fn find_running(db: &Database) -> Result<Option<SlotRecord>, DbError> {
    db.conn()
        .query_row(
            "select id, ticket_id, connection_id, note, started_at, stopped_at, published_at
             from slots
             where stopped_at is null",
            [],
            |row| {
                Ok(SlotRecord {
                    id: row.get(0)?,
                    ticket_id: row.get(1)?,
                    connection_id: row.get(2)?,
                    note: row.get(3)?,
                    started_at: row.get(4)?,
                    stopped_at: row.get(5)?,
                    published_at: row.get(6)?,
                })
            },
        )
        .map(Some)
        .or_else(|e| {
            if e == rusqlite::Error::QueryReturnedNoRows {
                Ok(None)
            } else {
                Err(DbError::from(e))
            }
        })
}

pub(crate) fn find_running_with_ticket(db: &Database) -> Result<Option<SlotWithTicket>, DbError> {
    db.conn()
        .query_row(
            "select s.id, t.ticket_key, t.summary, s.connection_id, s.note, s.started_at, s.stopped_at, s.published_at
             from slots s
             join tickets t on t.id = s.ticket_id
             where s.stopped_at is null",
            [],
            |row| {
                Ok(SlotWithTicket {
                    id: row.get(0)?,
                    ticket_key: row.get(1)?,
                    summary: row.get(2)?,
                    connection_id: row.get(3)?,
                    note: row.get(4)?,
                    started_at: row.get(5)?,
                    stopped_at: row.get(6)?,
                    published_at: row.get(7)?,
                })
            },
        )
        .map(Some)
        .or_else(|e| {
            if e == rusqlite::Error::QueryReturnedNoRows {
                Ok(None)
            } else {
                Err(DbError::from(e))
            }
        })
}

pub(crate) fn stop_by_id(db: &Database, slot_id: i64, stopped_at: i64) -> Result<(), DbError> {
    db.conn().execute(
        "update slots set stopped_at = ?1 where id = ?2",
        (stopped_at, slot_id),
    )?;
    Ok(())
}

pub(crate) fn find_by_id_with_ticket(
    db: &Database,
    slot_id: i64,
) -> Result<Option<SlotWithTicket>, DbError> {
    db.conn()
        .query_row(
            "select s.id, t.ticket_key, t.summary, s.connection_id, s.note, s.started_at, s.stopped_at, s.published_at
             from slots s
             join tickets t on t.id = s.ticket_id
             where s.id = ?1",
            [slot_id],
            map_slot_with_ticket,
        )
        .map(Some)
        .or_else(|e| {
            if e == rusqlite::Error::QueryReturnedNoRows {
                Ok(None)
            } else {
                Err(DbError::from(e))
            }
        })
}

pub(crate) fn stop_running(db: &Database, stopped_at: i64) -> Result<SlotWithTicket, DbError> {
    let slot = find_running_with_ticket(db)?
        .ok_or(DbError::Sqlite(rusqlite::Error::QueryReturnedNoRows))?;
    if stopped_at <= slot.started_at {
        return Err(DbError::StopBeforeStart);
    }
    stop_by_id(db, slot.id, stopped_at)?;
    Ok(slot)
}

pub(crate) fn insert(
    db: &Database,
    ticket_id: i64,
    connection_id: i64,
    note: Option<&str>,
    started_at: i64,
) -> Result<SlotWithTicket, DbError> {
    db.conn().execute(
        "insert into slots (ticket_id, connection_id, note, started_at, stopped_at, published_at)
         values (?1, ?2, ?3, ?4, null, null)",
        (ticket_id, connection_id, note, started_at),
    )?;

    let slot_id = db.conn().last_insert_rowid();

    db.conn()
        .query_row(
            "select s.id, t.ticket_key, t.summary, s.connection_id, s.note, s.started_at, s.stopped_at, s.published_at
             from slots s
             join tickets t on t.id = s.ticket_id
             where s.id = ?1",
            [slot_id],
            |row| {
                Ok(SlotWithTicket {
                    id: row.get(0)?,
                    ticket_key: row.get(1)?,
                    summary: row.get(2)?,
                    connection_id: row.get(3)?,
                    note: row.get(4)?,
                    started_at: row.get(5)?,
                    stopped_at: row.get(6)?,
                    published_at: row.get(7)?,
                })
            },
        )
        .map_err(DbError::from)
}

pub(crate) fn insert_completed(
    db: &Database,
    ticket_id: i64,
    connection_id: i64,
    note: Option<&str>,
    started_at: i64,
    stopped_at: i64,
) -> Result<SlotWithTicket, DbError> {
    db.conn().execute(
        "insert into slots (ticket_id, connection_id, note, started_at, stopped_at, published_at)
         values (?1, ?2, ?3, ?4, ?5, null)",
        (ticket_id, connection_id, note, started_at, stopped_at),
    )?;

    let slot_id = db.conn().last_insert_rowid();

    db.conn()
        .query_row(
            "select s.id, t.ticket_key, t.summary, s.connection_id, s.note, s.started_at, s.stopped_at, s.published_at
             from slots s
             join tickets t on t.id = s.ticket_id
             where s.id = ?1",
            [slot_id],
            map_slot_with_ticket,
        )
        .map_err(DbError::from)
}

pub(crate) fn overlaps(
    db: &Database,
    connection_id: i64,
    started_at: i64,
    stopped_at: i64,
) -> Result<bool, DbError> {
    let count: i64 = db.conn().query_row(
        "select count(*)
         from slots
         where connection_id = ?1
           and started_at < ?3
           and coalesce(stopped_at, 9223372036854775807) > ?2",
        (connection_id, started_at, stopped_at),
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub(crate) fn get_slots(
    db: &Database,
    connection_id: i64,
    from: i64,
    to: Option<i64>,
    sort: SlotSort,
) -> Result<Vec<SlotWithTicket>, DbError> {
    let order_by = match sort {
        SlotSort::StartedAt => "s.started_at",
        SlotSort::TicketId => "s.ticket_id",
    };

    let sql = if to.is_some() {
        format!(
            "select s.id, t.ticket_key, t.summary, s.connection_id, s.note, s.started_at, s.stopped_at, s.published_at
             from slots s
             join tickets t on t.id = s.ticket_id
             where s.connection_id = ?1
               and s.started_at < ?3
               and coalesce(s.stopped_at, 9223372036854775807) > ?2
             order by {order_by}"
        )
    } else {
        format!(
            "select s.id, t.ticket_key, t.summary, s.connection_id, s.note, s.started_at, s.stopped_at, s.published_at
             from slots s
             join tickets t on t.id = s.ticket_id
             where s.connection_id = ?1
               and coalesce(s.stopped_at, 9223372036854775807) > ?2
             order by {order_by}"
        )
    };

    let mut stmt = db.conn().prepare(&sql)?;
    let rows = match to {
        Some(to) => stmt.query_map((connection_id, from, to), map_slot_with_ticket)?,
        None => stmt.query_map((connection_id, from), map_slot_with_ticket)?,
    };

    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub(crate) fn get_unpublished(
    db: &Database,
    connection_id: i64,
    from: i64,
    to: Option<i64>,
) -> Result<Vec<SlotWithTicket>, DbError> {
    let sql = if to.is_some() {
        "select s.id, t.ticket_key, t.summary, s.connection_id, s.note, s.started_at, s.stopped_at, s.published_at
         from slots s
         join tickets t on t.id = s.ticket_id
         where s.connection_id = ?1 and s.started_at >= ?2 and s.started_at < ?3
           and s.stopped_at is not null and s.published_at is null
         order by s.started_at"
    } else {
        "select s.id, t.ticket_key, t.summary, s.connection_id, s.note, s.started_at, s.stopped_at, s.published_at
         from slots s
         join tickets t on t.id = s.ticket_id
         where s.connection_id = ?1 and s.started_at >= ?2
           and s.stopped_at is not null and s.published_at is null
         order by s.started_at"
    };

    let mut stmt = db.conn().prepare(sql)?;
    let rows = match to {
        Some(to) => stmt.query_map((connection_id, from, to), map_slot_with_ticket)?,
        None => stmt.query_map((connection_id, from), map_slot_with_ticket)?,
    };

    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub(crate) fn mark_published(db: &Database, ids: &[i64], published_at: i64) -> Result<(), DbError> {
    for id in ids {
        db.conn().execute(
            "update slots set published_at = ?1 where id = ?2",
            (published_at, id),
        )?;
    }
    Ok(())
}

fn map_slot_with_ticket(row: &rusqlite::Row<'_>) -> rusqlite::Result<SlotWithTicket> {
    Ok(SlotWithTicket {
        id: row.get(0)?,
        ticket_key: row.get(1)?,
        summary: row.get(2)?,
        connection_id: row.get(3)?,
        note: row.get(4)?,
        started_at: row.get(5)?,
        stopped_at: row.get(6)?,
        published_at: row.get(7)?,
    })
}

pub(crate) struct SlotUpdate {
    pub ticket_id: Option<i64>,
    pub note: Change<String>,
    pub started_at: Option<i64>,
    pub stopped_at: Change<i64>,
}

pub(crate) fn delete(db: &Database, slot_id: i64) -> Result<bool, DbError> {
    let affected = db
        .conn()
        .execute("delete from slots where id = ?1", [slot_id])?;
    Ok(affected > 0)
}

pub(crate) fn update(db: &Database, slot_id: i64, changes: &SlotUpdate) -> Result<(), DbError> {
    use rusqlite::types::Value;

    let mut set_clauses: Vec<String> = Vec::new();
    let mut params: Vec<Value> = Vec::new();
    let mut param_idx = 1;

    if let Some(ticket_id) = changes.ticket_id {
        set_clauses.push(format!("ticket_id = ?{param_idx}"));
        params.push(Value::Integer(ticket_id));
        param_idx += 1;
    }

    match &changes.note {
        Change::Skip => {}
        Change::Clear => {
            set_clauses.push(format!("note = ?{param_idx}"));
            params.push(Value::Null);
            param_idx += 1;
        }
        Change::Set(v) => {
            set_clauses.push(format!("note = ?{param_idx}"));
            params.push(Value::Text(v.clone()));
            param_idx += 1;
        }
    }

    if let Some(started_at) = changes.started_at {
        set_clauses.push(format!("started_at = ?{param_idx}"));
        params.push(Value::Integer(started_at));
        param_idx += 1;
    }

    match &changes.stopped_at {
        Change::Skip => {}
        Change::Clear => {
            set_clauses.push(format!("stopped_at = ?{param_idx}"));
            params.push(Value::Null);
            param_idx += 1;
        }
        Change::Set(v) => {
            set_clauses.push(format!("stopped_at = ?{param_idx}"));
            params.push(Value::Integer(*v));
            param_idx += 1;
        }
    }

    if set_clauses.is_empty() {
        return Ok(());
    }

    params.push(Value::Integer(slot_id));
    let sql = format!(
        "update slots set {} where id = ?{param_idx}",
        set_clauses.join(", ")
    );
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
        .iter()
        .map(|p| p as &dyn rusqlite::types::ToSql)
        .collect();
    db.conn().execute(&sql, param_refs.as_slice())?;
    Ok(())
}
