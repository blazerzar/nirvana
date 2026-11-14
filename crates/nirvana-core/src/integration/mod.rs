//! Clients for the backends behind connections.
//!
//! An [`Integration`] is built from a [`Connection`] and its token, with one
//! variant per backend. Every integration can test its connection, fetch an
//! issue title and publish time. Issue URLs need no client and are built by
//! [`Connection::issue_url`].

pub(crate) mod jira;
pub(crate) mod linear;

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use jira::{JiraAuth, JiraIntegration};
use linear::LinearIntegration;
use reqwest::Url;
use secrecy::SecretString;
use thiserror::Error;

use crate::domain::{Connection, Kind};
use crate::errors::{DomainError, NirvanaError};

const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Error)]
pub enum IntegrationError {
    #[error("HTTP client unavailable")]
    ClientUnavailable(#[source] Arc<reqwest::Error>),
    #[error("network error")]
    Network(#[from] reqwest::Error),
    #[error("ticket not found: {0}")]
    TicketNotFound(String),
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("API error: {0}")]
    Api(String),
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(i64),
    #[error("worklog of {0} s is under the {min} s Jira accepts", min = jira::MIN_WORKLOG_SECONDS)]
    WorklogTooShort(i64),
}

#[derive(Debug)]
pub(crate) struct IssueInfo {
    pub key: String,
    pub title: String,
}

fn utc(timestamp: i64) -> Result<DateTime<Utc>, IntegrationError> {
    DateTime::from_timestamp(timestamp, 0).ok_or(IntegrationError::InvalidTimestamp(timestamp))
}

pub(crate) fn http_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder().timeout(TIMEOUT).build()
}

pub(crate) enum Integration {
    Jira(JiraIntegration),
    Linear(LinearIntegration),
}

impl Integration {
    pub fn new(
        connection: &Connection,
        client: reqwest::Client,
        token: SecretString,
    ) -> Result<Self, NirvanaError> {
        let host = &connection.model.host;
        let jira_url = || {
            Url::parse(&format!("https://{host}/rest/api/2"))
                .map_err(|_| DomainError::InvalidHost(host.clone()))
        };
        let integration = match connection.kind {
            Kind::JiraCloud => {
                let email = connection
                    .model
                    .identity
                    .clone()
                    .ok_or(DomainError::MissingIdentity(connection.kind))?;
                Self::Jira(JiraIntegration::new(
                    client,
                    jira_url()?,
                    JiraAuth::Basic { email, token },
                ))
            }
            Kind::JiraDc => Self::Jira(JiraIntegration::new(
                client,
                jira_url()?,
                JiraAuth::Bearer(token),
            )),
            Kind::Linear => Self::Linear(LinearIntegration::new(
                client,
                Url::parse(linear::API_URL).expect("Linear API URL is valid"),
                token,
            )),
        };
        Ok(integration)
    }

    pub async fn test_connection(&self) -> Result<(), IntegrationError> {
        match self {
            Self::Jira(j) => j.test_connection().await,
            Self::Linear(l) => l.test_connection().await,
        }
    }

    pub async fn fetch_issue(&self, ticket_key: &str) -> Result<IssueInfo, IntegrationError> {
        match self {
            Self::Jira(j) => j.fetch_issue(ticket_key).await,
            Self::Linear(l) => l.fetch_issue(ticket_key).await,
        }
    }

    pub async fn publish_slot(
        &self,
        ticket_key: &str,
        started_at: i64,
        seconds: i64,
    ) -> Result<(), IntegrationError> {
        match self {
            Self::Jira(j) => j.publish_slot(ticket_key, started_at, seconds).await,
            Self::Linear(l) => l.publish_slot(ticket_key, started_at, seconds).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn http_client_times_out() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());

        let request = http_client().unwrap().get(url).send();
        let err = tokio::time::timeout(TIMEOUT * 2, request)
            .await
            .unwrap()
            .unwrap_err();

        assert!(err.is_timeout());
    }
}
