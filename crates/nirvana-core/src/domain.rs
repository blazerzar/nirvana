use nirvana_entity::{connection, jira_settings};

use crate::errors::TrackingError;

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
pub struct Connection {
    pub model: connection::Model,
    pub kind: ConnectionKind,
}

impl Connection {
    pub(crate) fn new(
        model: connection::Model,
        jira_settings: Option<jira_settings::Model>,
    ) -> Result<Self, TrackingError> {
        let kind = match (model.kind.as_str(), jira_settings.map(JiraSettings::from)) {
            ("jira-cloud", Some(settings)) => ConnectionKind::JiraCloud(settings),
            ("jira-dc", Some(settings)) => ConnectionKind::JiraDc(settings),
            ("jira-cloud" | "jira-dc", None) => {
                return Err(TrackingError::MissingConnectionSettings(model.id));
            }
            (kind, _) => return Err(TrackingError::UnsupportedConnectionKind(kind.to_string())),
        };
        Ok(Self { model, kind })
    }
}

#[derive(Debug)]
pub enum ConnectionKind {
    JiraCloud(JiraSettings),
    JiraDc(JiraSettings),
}

impl ConnectionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::JiraCloud(_) => "jira-cloud",
            Self::JiraDc(_) => "jira-dc",
        }
    }

    pub fn jira_settings(&self) -> &JiraSettings {
        match self {
            Self::JiraCloud(settings) | Self::JiraDc(settings) => settings,
        }
    }
}

#[derive(Debug)]
pub struct JiraSettings {
    pub publish: bool,
    pub squash: bool,
}

impl From<jira_settings::Model> for JiraSettings {
    fn from(model: jira_settings::Model) -> Self {
        Self {
            publish: model.publish,
            squash: model.squash,
        }
    }
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
    pub started_at: Option<i64>,
    pub stopped_at: Change<i64>,
}
