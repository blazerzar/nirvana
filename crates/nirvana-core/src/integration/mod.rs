#![expect(dead_code)]

pub(crate) mod jira;

use jira::JiraIntegration;
use secrecy::SecretString;
use thiserror::Error;

use crate::domain::{Connection, ConnectionKind};

#[derive(Debug, Error)]
pub enum IntegrationError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("ticket not found: {0}")]
    TicketNotFound(String),
    #[error("authentication failed: {0}")]
    Auth(String),
}

pub(crate) struct IssueInfo {
    pub summary: String,
}

enum IntegrationKind {
    Jira(JiraIntegration),
}

pub(crate) struct Integration {
    kind: IntegrationKind,
}

impl Integration {
    pub fn build_for_url(connection: &Connection) -> Integration {
        let kind = match &connection.kind {
            ConnectionKind::JiraCloud(_) | ConnectionKind::JiraDc(_) => {
                IntegrationKind::Jira(JiraIntegration::new_unauthenticated(
                    &connection.model.host,
                    connection.kind.as_str(),
                ))
            }
        };
        Integration { kind }
    }

    pub fn get_issue_link(&self, ticket_key: &str) -> String {
        match &self.kind {
            IntegrationKind::Jira(j) => j.get_issue_link(ticket_key),
        }
    }

    pub async fn test_connection(&self) -> Result<(), IntegrationError> {
        match &self.kind {
            IntegrationKind::Jira(j) => j.test_connection().await,
        }
    }

    pub async fn fetch_issue(&self, ticket_key: &str) -> Result<IssueInfo, IntegrationError> {
        match &self.kind {
            IntegrationKind::Jira(j) => j.fetch_issue(ticket_key).await,
        }
    }

    pub async fn publish_slot(
        &self,
        ticket_key: &str,
        started_at: i64,
        seconds: i64,
    ) -> Result<(), IntegrationError> {
        match &self.kind {
            IntegrationKind::Jira(j) => j.publish_slot(ticket_key, started_at, seconds).await,
        }
    }
}

pub(crate) fn build_integration(connection: &Connection, token: &SecretString) -> Integration {
    let kind = match &connection.kind {
        ConnectionKind::JiraCloud(_) | ConnectionKind::JiraDc(_) => {
            IntegrationKind::Jira(JiraIntegration::new(
                &connection.model.host,
                &connection.model.identity,
                token,
                connection.kind.as_str(),
            ))
        }
    };
    Integration { kind }
}
