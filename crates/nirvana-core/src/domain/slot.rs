use super::Change;

pub enum SlotSort {
    StartedAt,
    TicketId,
}

#[derive(Debug)]
pub struct SlotCreate {
    pub ticket_key: String,
    pub started_at: i64,
    pub stopped_at: i64,
}

#[derive(Debug, Default)]
pub struct SlotEdit {
    pub ticket_key: Option<String>,
    pub started_at: Option<i64>,
    pub stopped_at: Change<i64>,
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
