use std::fmt;
use std::str::FromStr;

use nirvana_entity::credential::SecretStore;
use nirvana_entity::{connection, jira_settings};
use secrecy::SecretString;

use crate::errors::DomainError;

#[derive(Debug)]
pub struct Connection {
    pub model: connection::Model,
    pub kind: Kind,
    pub settings: Settings,
    pub secret_store: SecretStore,
}

impl Connection {
    /// Builds a connection from a row loaded with its credential and the
    /// settings of its kind.
    pub(crate) fn new(mut model: connection::ModelEx) -> Result<Self, DomainError> {
        let secret_store = model
            .credential
            .take()
            .ok_or(DomainError::MissingCredential(model.id))?
            .secret_store;
        let kind = model.kind.parse()?;
        let settings = match kind {
            Kind::JiraCloud | Kind::JiraDc => Settings::Jira(
                model
                    .jira_settings
                    .take()
                    .map(jira_settings::Model::from)
                    .ok_or(DomainError::MissingConnectionSettings(model.id))?
                    .into(),
            ),
            Kind::Linear => Settings::Linear,
        };
        Ok(Self {
            model: model.into(),
            kind,
            settings,
            secret_store,
        })
    }

    pub fn issue_url(&self, ticket_key: &str) -> String {
        let host = &self.model.host;
        match self.kind {
            Kind::JiraCloud | Kind::JiraDc => format!("https://{host}/browse/{ticket_key}"),
            Kind::Linear => format!("https://{host}/issue/{ticket_key}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    JiraCloud,
    JiraDc,
    Linear,
}

impl Kind {
    pub const ALL: [Self; 3] = [Self::JiraCloud, Self::JiraDc, Self::Linear];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::JiraCloud => "jira-cloud",
            Self::JiraDc => "jira-dc",
            Self::Linear => "linear",
        }
    }

    pub(crate) fn validate_identity(self, identity: Option<&str>) -> Result<(), DomainError> {
        match (self, identity.map(str::trim)) {
            (Self::JiraCloud, None | Some("")) => Err(DomainError::MissingIdentity(self)),
            (Self::JiraDc | Self::Linear, Some(_)) => Err(DomainError::UnexpectedIdentity(self)),
            (Self::JiraCloud, Some(_)) | (Self::JiraDc | Self::Linear, None) => Ok(()),
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
            "linear" => Ok(Self::Linear),
            kind => Err(DomainError::UnsupportedConnectionKind(kind.to_string())),
        }
    }
}

#[derive(Debug)]
pub enum Settings {
    Jira(JiraSettings),
    Linear,
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
pub enum SettingsEdit {
    Jira(JiraSettingsEdit),
}

#[derive(Debug, Default)]
pub struct JiraSettingsEdit {
    pub publish: Option<bool>,
    pub squash: Option<bool>,
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

impl ConnectionData {
    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        if self.name.trim().is_empty() {
            return Err(DomainError::EmptyConnectionName);
        }
        self.kind.validate_identity(self.identity.as_deref())
    }
}

#[derive(Debug, Default)]
pub struct ConnectionEdit {
    pub name: Option<String>,
    pub host: Option<String>,
    pub identity: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_round_trips() {
        for kind in [Kind::JiraCloud, Kind::JiraDc, Kind::Linear] {
            assert_eq!(kind.as_str().parse::<Kind>().unwrap(), kind);
        }
    }

    #[test]
    fn builds_issue_url_per_kind() {
        for (kind, host, url) in [
            (
                Kind::JiraCloud,
                "nirvana.atlassian.net",
                "https://nirvana.atlassian.net/browse/ENG-1",
            ),
            (
                Kind::JiraDc,
                "jira.nirvana.com",
                "https://jira.nirvana.com/browse/ENG-1",
            ),
            (
                Kind::Linear,
                "linear.app/nirvana",
                "https://linear.app/nirvana/issue/ENG-1",
            ),
        ] {
            let connection = Connection {
                model: connection::Model {
                    id: 1,
                    name: "nirvana".to_string(),
                    kind: kind.to_string(),
                    host: host.to_string(),
                    identity: None,
                    created_at: 0,
                    updated_at: 0,
                    profile_id: 1,
                },
                kind,
                settings: Settings::Linear,
                secret_store: SecretStore::Plaintext,
            };
            assert_eq!(connection.issue_url("ENG-1"), url);
        }
    }

    #[test]
    fn rejects_unknown_kind() {
        assert!(matches!(
            "unsupported".parse::<Kind>(),
            Err(DomainError::UnsupportedConnectionKind(kind)) if kind == "unsupported"
        ));
    }
}
