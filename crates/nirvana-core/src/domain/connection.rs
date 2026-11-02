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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    JiraCloud,
    JiraDc,
    Linear,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::JiraCloud => "jira-cloud",
            Self::JiraDc => "jira-dc",
            Self::Linear => "linear",
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
        match (self.kind, self.identity.as_deref().map(str::trim)) {
            (Kind::JiraCloud, None | Some("")) => Err(DomainError::MissingIdentity(self.kind)),
            (Kind::JiraDc | Kind::Linear, Some(_)) => {
                Err(DomainError::UnexpectedIdentity(self.kind))
            }
            (Kind::JiraCloud, Some(_)) | (Kind::JiraDc | Kind::Linear, None) => Ok(()),
        }
    }
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
    fn rejects_unknown_kind() {
        assert!(matches!(
            "unsupported".parse::<Kind>(),
            Err(DomainError::UnsupportedConnectionKind(kind)) if kind == "unsupported"
        ));
    }
}
