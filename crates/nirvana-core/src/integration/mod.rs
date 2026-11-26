//! Clients for the backends behind connections.
//!
//! An [`Integration`] is built from a [`Connection`] and its token, with one
//! variant per backend. Every integration can test its connection, fetch an
//! issue title and publish time. Issue URLs need no client and are built by
//! [`Connection::issue_url`].

pub(crate) mod jira;
pub(crate) mod linear;

use std::io::ErrorKind;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use jira::{JiraAuth, JiraIntegration};
use linear::LinearIntegration;
use secrecy::SecretString;
use thiserror::Error;
use tracing::debug;

use crate::domain::{Connection, Kind};
use crate::errors::{DomainError, NirvanaError, error_chain};

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
        let host_url = connection.host_url();
        let invalid_host = || DomainError::InvalidHost(connection.model.host.clone());
        let integration = match connection.kind {
            Kind::JiraCloud => {
                let email = connection
                    .model
                    .identity
                    .clone()
                    .ok_or(DomainError::MissingIdentity(connection.kind))?;
                Self::Jira(JiraIntegration::new(
                    client,
                    jira::api_url(&host_url).ok_or_else(invalid_host)?,
                    JiraAuth::Basic { email, token },
                ))
            }
            Kind::JiraDc => Self::Jira(JiraIntegration::new(
                client,
                jira::api_url(&host_url).ok_or_else(invalid_host)?,
                JiraAuth::Bearer(token),
            )),
            Kind::Linear => Self::Linear(LinearIntegration::new(
                client,
                linear::api_url(&host_url).ok_or_else(invalid_host)?,
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
        let published = match self {
            Self::Jira(j) => j.publish_slot(ticket_key, started_at, seconds).await,
            Self::Linear(l) => l.publish_slot(ticket_key, started_at, seconds).await,
        };
        published.inspect_err(|e| debug!(ticket_key, error = ?e, "publishing failed"))
    }
}

impl IntegrationError {
    /// Returns what went wrong with the request or the token, and the whole
    /// chain for other errors, which are readable already.
    pub fn reason(&self) -> String {
        match self {
            Self::Network(e) => network_error(e).unwrap_or_else(|| error_chain(self)),
            Self::Auth(reason) => format!("the token was rejected ({reason})"),
            _ => error_chain(self),
        }
    }
}

/// Describes a failed request by its host: not reached, not answered in time,
/// the connection lost while sending, or a response that cannot be read.
fn network_error(error: &reqwest::Error) -> Option<String> {
    let host = error.url()?.host_str()?;
    let unreachable = error.is_connect()
        || io_kind(error).is_some_and(|kind| {
            matches!(
                kind,
                ErrorKind::HostUnreachable
                    | ErrorKind::NetworkUnreachable
                    | ErrorKind::NetworkDown
                    | ErrorKind::ConnectionRefused
                    | ErrorKind::AddrNotAvailable
            )
        });
    if error.is_timeout() {
        Some(format!("{host} timed out"))
    } else if unreachable {
        Some(format!("couldn't reach {host}"))
    } else if error.is_request() {
        Some(format!("lost the connection to {host}"))
    } else if error.is_decode() || error.is_body() {
        Some(format!("couldn't read the response of {host}"))
    } else {
        None
    }
}

/// Returns the kind of the first I/O error in the chain of `error`.
fn io_kind(error: &(dyn std::error::Error + 'static)) -> Option<ErrorKind> {
    std::iter::successors(Some(error), |e| e.source())
        .find_map(|e| e.downcast_ref::<std::io::Error>())
        .map(std::io::Error::kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reason_names_an_unreachable_host() {
        let error = crate::integration::http_client()
            .unwrap()
            .get("http://127.0.0.1:9")
            .send()
            .await
            .unwrap_err();
        assert_eq!(
            IntegrationError::Network(error).reason(),
            "couldn't reach 127.0.0.1"
        );
    }

    #[test]
    fn reason_names_a_rejected_token() {
        let error = IntegrationError::Auth("401 Unauthorized".to_string());
        assert_eq!(error.reason(), "the token was rejected (401 Unauthorized)");
    }

    #[test]
    fn reason_keeps_the_chain_of_other_errors() {
        let error = IntegrationError::TicketNotFound("KAR-1".to_string());
        assert_eq!(error.reason(), "ticket not found: KAR-1");
    }

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
