//! Jira Cloud and Jira Data Center through REST API v2.
//!
//! Jira Cloud authenticates with the email and API token (basic auth), Jira
//! Data Center with a personal access token (bearer auth). Time is published
//! as worklogs.

use reqwest::{Method, RequestBuilder, Response, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tracing::debug;

use crate::integration::{IntegrationError, IssueInfo, TIMEOUT};

pub(crate) enum JiraAuth {
    Basic { email: String, token: SecretString },
    Bearer(SecretString),
}

pub(crate) struct JiraIntegration {
    host: String,
    auth: JiraAuth,
    client: reqwest::Client,
}

#[derive(Deserialize)]
struct Issue {
    fields: IssueFields,
}

#[derive(Deserialize)]
struct IssueFields {
    summary: String,
}

impl JiraIntegration {
    pub(crate) fn new(host: &str, auth: JiraAuth) -> Self {
        Self {
            host: host.to_string(),
            auth,
            client: reqwest::Client::new(),
        }
    }

    pub(crate) async fn test_connection(&self) -> Result<(), IntegrationError> {
        let response = self.request(Method::GET, "myself").send().await?;
        check_status(response, None)?;
        Ok(())
    }

    pub(crate) async fn fetch_issue(
        &self,
        ticket_key: &str,
    ) -> Result<IssueInfo, IntegrationError> {
        let path = format!("issue/{ticket_key}?fields=summary");
        let response = self.request(Method::GET, &path).send().await?;
        let issue: Issue = check_status(response, Some(ticket_key))?.json().await?;
        Ok(IssueInfo {
            title: issue.fields.summary,
        })
    }

    pub(crate) async fn publish_slot(
        &self,
        ticket_key: &str,
        started_at: i64,
        seconds: i64,
    ) -> Result<(), IntegrationError> {
        let started = chrono::DateTime::from_timestamp(started_at, 0)
            .unwrap()
            .format("%Y-%m-%dT%H:%M:%S.000%z");

        let body = serde_json::json!({
            "timeSpentSeconds": seconds,
            "started": started.to_string(),
        });

        let path = format!("issue/{ticket_key}/worklog");
        let response = self.request(Method::POST, &path).json(&body).send().await?;
        check_status(response, Some(ticket_key))?;
        Ok(())
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let url = format!("https://{}/rest/api/2/{path}", self.host);
        let request = self.client.request(method, url).timeout(TIMEOUT);
        match &self.auth {
            JiraAuth::Basic { email, token } => {
                request.basic_auth(email, Some(token.expose_secret()))
            }
            JiraAuth::Bearer(token) => request.bearer_auth(token.expose_secret()),
        }
    }
}

fn check_status(
    response: Response,
    ticket_key: Option<&str>,
) -> Result<Response, IntegrationError> {
    debug!(url = %response.url(), status = %response.status(), "jira response");
    match (response.status(), ticket_key) {
        (StatusCode::NOT_FOUND, Some(key)) => {
            Err(IntegrationError::TicketNotFound(key.to_string()))
        }
        (status @ (StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN), _) => {
            Err(IntegrationError::Auth(status.to_string()))
        }
        _ => Ok(response.error_for_status()?),
    }
}
