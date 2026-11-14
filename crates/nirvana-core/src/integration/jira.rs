//! Jira Cloud and Jira Data Center through REST API v2.
//!
//! Jira Cloud authenticates with the email and API token (basic auth), Jira
//! Data Center with a personal access token (bearer auth). Time is published
//! as worklogs.

use reqwest::{Method, RequestBuilder, Response, StatusCode, Url};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tracing::debug;

use crate::integration::{IntegrationError, IssueInfo};

pub(crate) enum JiraAuth {
    Basic { email: String, token: SecretString },
    Bearer(SecretString),
}

pub(crate) struct JiraIntegration {
    api_url: Url,
    auth: JiraAuth,
    client: reqwest::Client,
}

#[derive(Deserialize)]
struct Issue {
    key: String,
    fields: IssueFields,
}

#[derive(Deserialize)]
struct IssueFields {
    summary: String,
}

impl JiraIntegration {
    pub(crate) fn new(client: reqwest::Client, api_url: Url, auth: JiraAuth) -> Self {
        Self {
            api_url,
            auth,
            client,
        }
    }

    pub(crate) async fn test_connection(&self) -> Result<(), IntegrationError> {
        let response = self
            .request(Method::GET, self.url(&["myself"]))
            .send()
            .await?;
        check_status(response, None)?;
        Ok(())
    }

    pub(crate) async fn fetch_issue(
        &self,
        ticket_key: &str,
    ) -> Result<IssueInfo, IntegrationError> {
        let mut url = self.url(&["issue", ticket_key]);
        url.set_query(Some("fields=summary"));
        let response = self.request(Method::GET, url).send().await?;
        let issue: Issue = check_status(response, Some(ticket_key))?.json().await?;
        Ok(IssueInfo {
            key: issue.key,
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

        let url = self.url(&["issue", ticket_key, "worklog"]);
        let response = self.request(Method::POST, url).json(&body).send().await?;
        check_status(response, Some(ticket_key))?;
        Ok(())
    }

    fn url(&self, segments: &[&str]) -> Url {
        let mut url = self.api_url.clone();
        url.path_segments_mut()
            .expect("https URL has path segments")
            .extend(segments);
        url
    }

    fn request(&self, method: Method, url: Url) -> RequestBuilder {
        let request = self.client.request(method, url);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_encodes_ticket_key() {
        let jira = JiraIntegration::new(
            reqwest::Client::new(),
            Url::parse("https://jira.example.com/rest/api/2").unwrap(),
            JiraAuth::Bearer("token".into()),
        );

        for (key, path) in [
            ("DES-1", "/rest/api/2/issue/DES-1"),
            ("DES-1?x", "/rest/api/2/issue/DES-1%3Fx"),
            ("DES/1#x", "/rest/api/2/issue/DES%2F1%23x"),
            ("..", "/rest/api/2/issue"),
        ] {
            assert_eq!(jira.url(&["issue", key]).path(), path, "{key}");
        }
    }
}
