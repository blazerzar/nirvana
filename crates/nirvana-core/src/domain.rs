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

#[derive(Debug)]
pub struct ConnectionData {
    pub name: String,
    pub kind: String,
    pub host: String,
    pub identity: String,
    pub secret_store: String,
    pub token: String,
}

#[derive(Debug)]
pub struct SlotCreate {
    pub ticket_key: String,
    pub note: Option<String>,
    pub started_at: i64,
    pub stopped_at: i64,
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
    pub idle_enabled: bool,
    pub idle_methods: Vec<String>,
    pub idle_threshold_secs: u64,
}

#[derive(Debug, Default)]
pub struct SlotEdit {
    pub ticket_key: Option<String>,
    pub note: Change<String>,
    pub started_at: Option<i64>,
    pub stopped_at: Change<i64>,
}
