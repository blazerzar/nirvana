use crate::storage::connection_repo::ConnectionRecord;
use crate::storage::slot_repo::SlotWithTicket;
use crate::storage::ticket_repo::TicketRecord;

pub use crate::storage::slot_repo::Change;

#[derive(Debug)]
pub struct Connection {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub host: String,
    pub identity: String,
    pub secret_store: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug)]
pub struct ConnectionData {
    pub name: String,
    pub kind: String,
    pub host: String,
    pub identity: String,
    pub secret_store: String,
    pub token: String,
}

impl From<ConnectionRecord> for Connection {
    fn from(r: ConnectionRecord) -> Self {
        Self {
            id: r.id,
            name: r.name,
            kind: r.kind,
            host: r.host,
            identity: r.identity,
            secret_store: r.secret_store,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug)]
pub struct Slot {
    pub id: i64,
    pub ticket_key: String,
    pub summary: Option<String>,
    pub note: Option<String>,
    pub started_at: i64,
    pub stopped_at: Option<i64>,
    pub published_at: Option<i64>,
    pub issue_url: Option<String>,
}

#[derive(Debug)]
pub struct SlotCreate {
    pub ticket_key: String,
    pub note: Option<String>,
    pub started_at: i64,
    pub stopped_at: i64,
}

impl Slot {
    pub(crate) fn from_record(record: SlotWithTicket, issue_url: Option<String>) -> Self {
        Self {
            id: record.id,
            ticket_key: record.ticket_key,
            summary: record.summary,
            note: record.note,
            started_at: record.started_at,
            stopped_at: record.stopped_at,
            published_at: record.published_at,
            issue_url,
        }
    }
}

#[derive(Debug)]
pub struct Ticket {
    pub id: i64,
    pub ticket_key: String,
    pub summary: Option<String>,
    pub connection_id: i64,
    pub last_worked_at: i64,
}

impl From<TicketRecord> for Ticket {
    fn from(r: TicketRecord) -> Self {
        Self {
            id: r.id,
            ticket_key: r.ticket_key,
            summary: r.summary,
            connection_id: r.connection_id,
            last_worked_at: r.last_worked_at,
        }
    }
}

#[derive(Debug)]
pub struct PublishResult {
    pub published_count: usize,
    pub failed: Vec<PublishFailure>,
    pub timestamp: i64,
}

#[derive(Debug)]
pub struct PublishFailure {
    pub ticket_key: String,
    pub error: String,
}

#[derive(Debug, Clone)]
pub struct AppSettings {
    pub publish_squashed_worklogs: bool,
    pub font_scale: f64,
    pub theme: String,
    pub show_tray_icon: bool,
}

#[derive(Debug, Default)]
pub struct SlotEdit {
    pub ticket_key: Option<String>,
    pub note: Change<String>,
    pub started_at: Option<i64>,
    pub stopped_at: Change<i64>,
}
