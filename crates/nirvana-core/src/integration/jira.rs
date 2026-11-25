//! Jira Cloud and Jira Data Center through REST API v2.
//!
//! Jira Cloud authenticates with the email and API token (basic auth), Jira
//! Data Center with a personal access token (bearer auth). Time is published
//! as worklogs.

use std::collections::BTreeMap;

use reqwest::{Method, RequestBuilder, Response, StatusCode, Url};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tracing::debug;

use crate::integration::{IntegrationError, IssueInfo, utc};

pub(crate) const MIN_WORKLOG_SECONDS: i64 = 60;
const MAX_ERROR_BODY_CHARS: usize = 300;

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
        check_status(response, None).await?;
        Ok(())
    }

    pub(crate) async fn fetch_issue(
        &self,
        ticket_key: &str,
    ) -> Result<IssueInfo, IntegrationError> {
        let mut url = self.url(&["issue", ticket_key]);
        url.set_query(Some("fields=summary"));
        let response = self.request(Method::GET, url).send().await?;
        let issue: Issue = check_status(response, Some(ticket_key))
            .await?
            .json()
            .await?;
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
        if seconds < MIN_WORKLOG_SECONDS {
            return Err(IntegrationError::WorklogTooShort(seconds));
        }
        let started = utc(started_at)?.format("%Y-%m-%dT%H:%M:%S.000%z");

        let body = serde_json::json!({
            "timeSpentSeconds": seconds,
            "started": started.to_string(),
        });

        let url = self.url(&["issue", ticket_key, "worklog"]);
        let response = self.request(Method::POST, url).json(&body).send().await?;
        check_status(response, Some(ticket_key)).await?;
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

async fn check_status(
    response: Response,
    ticket_key: Option<&str>,
) -> Result<Response, IntegrationError> {
    let status = response.status();
    debug!(url = %response.url(), %status, "jira response");
    match (status, ticket_key) {
        (StatusCode::NOT_FOUND, Some(key)) => {
            Err(IntegrationError::TicketNotFound(key.to_string()))
        }
        (StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN, _) => {
            Err(IntegrationError::Auth(status.to_string()))
        }
        _ if status.is_client_error() || status.is_server_error() => {
            let body = response.text().await?;
            let reason = error_messages(&body).unwrap_or_else(|| truncate(body.trim()));
            Err(IntegrationError::Api(format!("{status}: {reason}")))
        }
        _ => Ok(response),
    }
}

/// Returns the REST API v2 endpoint of the Jira at `host_url`.
pub(crate) fn api_url(host_url: &str) -> Option<Url> {
    Url::parse(&format!("{host_url}/rest/api/2")).ok()
}

/// The error body Jira sends with a rejected request.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody {
    #[serde(default)]
    error_messages: Vec<String>,
    #[serde(default)]
    errors: BTreeMap<String, String>,
}

/// Returns the messages of a Jira error body, joined by `; `, or `None` when
/// `body` is not one or has no messages.
fn error_messages(body: &str) -> Option<String> {
    let body: ErrorBody = serde_json::from_str(body).ok()?;
    let messages: Vec<String> = body
        .error_messages
        .into_iter()
        .chain(body.errors.into_values())
        .collect();
    (!messages.is_empty()).then(|| truncate(&messages.join("; ")))
}

/// Cuts `body` to [`MAX_ERROR_BODY_CHARS`], marking the cut with `…`.
fn truncate(body: &str) -> String {
    match body.char_indices().nth(MAX_ERROR_BODY_CHARS) {
        Some((end, _)) => format!("{}…", &body[..end]),
        None => body.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{basic_auth, bearer_token, body_json, method, path};
    use wiremock::{Match, Mock, MockServer, ResponseTemplate};

    use super::*;

    fn jira(server: &MockServer, auth: JiraAuth) -> JiraIntegration {
        JiraIntegration::new(
            reqwest::Client::new(),
            Url::parse(&format!("{}/rest/api/2", server.uri())).unwrap(),
            auth,
        )
    }

    fn bearer() -> JiraAuth {
        JiraAuth::Bearer("token".into())
    }

    async fn assert_publish_slot_posts_worklog(auth: JiraAuth, auth_matcher: impl Match + 'static) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/api/2/issue/KAR-1/worklog"))
            .and(auth_matcher)
            .and(body_json(serde_json::json!({
                "timeSpentSeconds": 5400,
                "started": "2026-11-14T09:00:00.000+0000",
            })))
            .respond_with(ResponseTemplate::new(201))
            .expect(1)
            .mount(&server)
            .await;

        jira(&server, auth)
            .publish_slot("KAR-1", 1_794_646_800, 5400)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn publish_slot_posts_worklog_to_cloud() {
        assert_publish_slot_posts_worklog(
            JiraAuth::Basic {
                email: "me@example.com".to_string(),
                token: "token".into(),
            },
            basic_auth("me@example.com", "token"),
        )
        .await;
    }

    #[tokio::test]
    async fn publish_slot_posts_worklog_to_dc() {
        assert_publish_slot_posts_worklog(bearer(), bearer_token("token")).await;
    }

    #[tokio::test]
    async fn publish_slot_to_missing_issue_fails() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let err = jira(&server, bearer())
            .publish_slot("KAR-1", 1_794_646_800, 5400)
            .await
            .unwrap_err();

        assert!(matches!(err, IntegrationError::TicketNotFound(key) if key == "KAR-1"));
    }

    #[tokio::test]
    async fn publish_slot_keeps_error_of_rejected_worklog() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_string(r#"{"errors":{"timeLogged":"invalid"}}"#),
            )
            .mount(&server)
            .await;

        let err = jira(&server, bearer())
            .publish_slot("KAR-1", 1_794_646_800, 5400)
            .await
            .unwrap_err();

        assert_eq!(err.to_string(), "API error: 400 Bad Request: invalid");
    }

    #[test]
    fn error_messages_joins_messages_and_field_errors() {
        let body = r#"{"errorMessages":["You cannot log work on a closed issue."],"errors":{"started":"Invalid date"}}"#;
        assert_eq!(
            error_messages(body).as_deref(),
            Some("You cannot log work on a closed issue.; Invalid date")
        );
    }

    #[test]
    fn error_messages_skips_bodies_without_messages() {
        assert_eq!(error_messages(r#"{"errorMessages":[],"errors":{}}"#), None);
        assert_eq!(error_messages("<html>bad gateway</html>"), None);
    }

    #[tokio::test]
    async fn publish_slot_rejects_worklog_under_a_minute() {
        let server = MockServer::start().await;

        let err = jira(&server, bearer())
            .publish_slot("KAR-1", 1_794_646_800, 59)
            .await
            .unwrap_err();

        assert!(matches!(err, IntegrationError::WorklogTooShort(59)));
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn publish_slot_rejects_invalid_timestamp() {
        let server = MockServer::start().await;

        let err = jira(&server, bearer())
            .publish_slot("KAR-1", i64::MAX, 5400)
            .await
            .unwrap_err();

        assert!(matches!(err, IntegrationError::InvalidTimestamp(i64::MAX)));
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[test]
    fn api_url_adds_rest_api_path() {
        assert_eq!(
            api_url("https://jira.example.com").unwrap().as_str(),
            "https://jira.example.com/rest/api/2"
        );
        assert_eq!(
            api_url("http://127.0.0.1:8080").unwrap().as_str(),
            "http://127.0.0.1:8080/rest/api/2"
        );
    }

    #[test]
    fn truncate_cuts_long_body() {
        assert_eq!(truncate("short"), "short");
        let long = "é".repeat(MAX_ERROR_BODY_CHARS + 1);
        assert_eq!(
            truncate(&long),
            format!("{}…", "é".repeat(MAX_ERROR_BODY_CHARS))
        );
    }

    #[test]
    fn url_encodes_ticket_key() {
        let jira = JiraIntegration::new(
            reqwest::Client::new(),
            Url::parse("https://jira.example.com/rest/api/2").unwrap(),
            bearer(),
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
