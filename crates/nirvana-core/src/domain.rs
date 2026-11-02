use std::fmt;
use std::str::FromStr;

use nirvana_entity::credential::SecretStore;
pub use nirvana_entity::profile::Model as Profile;
use nirvana_entity::{connection, jira_settings};
use secrecy::SecretString;

use crate::errors::DomainError;

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
pub struct ActiveProfile {
    pub profile: Profile,
    pub connections: Vec<Connection>,
}

#[derive(Debug)]
pub struct Connection {
    pub model: connection::Model,
    pub kind: Kind,
    pub settings: Settings,
}

impl Connection {
    pub(crate) fn new(
        model: connection::Model,
        jira_settings: Option<jira_settings::Model>,
    ) -> Result<Self, DomainError> {
        let kind = model.kind.parse()?;
        let settings = match kind {
            Kind::JiraCloud | Kind::JiraDc => Settings::Jira(
                jira_settings
                    .ok_or(DomainError::MissingConnectionSettings(model.id))?
                    .into(),
            ),
        };
        Ok(Self {
            model,
            kind,
            settings,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    JiraCloud,
    JiraDc,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::JiraCloud => "jira-cloud",
            Self::JiraDc => "jira-dc",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Kind {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "jira-cloud" => Ok(Self::JiraCloud),
            "jira-dc" => Ok(Self::JiraDc),
            kind => Err(DomainError::UnsupportedConnectionKind(kind.to_string())),
        }
    }
}

#[derive(Debug)]
pub enum Settings {
    Jira(JiraSettings),
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
    pub kind: Kind,
    pub host: String,
    pub identity: Option<String>,
    pub secret_store: SecretStore,
    pub token: SecretString,
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

#[derive(Debug, Default)]
pub struct SlotEdit {
    pub ticket_key: Option<String>,
    pub started_at: Option<i64>,
    pub stopped_at: Change<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_round_trips() {
        for kind in [Kind::JiraCloud, Kind::JiraDc] {
            assert_eq!(kind.as_str().parse::<Kind>().unwrap(), kind);
        }
    }

    #[test]
    fn rejects_unknown_kind() {
        assert!(matches!(
            "linear".parse::<Kind>(),
            Err(DomainError::UnsupportedConnectionKind(kind)) if kind == "linear"
        ));
    }
}
