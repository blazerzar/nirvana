//! Linear through its GraphQL API.
//!
//! The API endpoint is passed in, built from the workspace host by
//! [`api_url`] in production, and the personal API key is sent as the
//! `Authorization` header. GraphQL errors are mapped to [`IntegrationError`]
//! by their code, and a missing issue by its message, since Linear reports it
//! as a generic input error. Linear has no worklogs, so time is published as
//! a comment on the issue.

use chrono::Local;
use reqwest::Url;
use reqwest::header::AUTHORIZATION;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use tracing::debug;

use crate::integration::{IntegrationError, IssueInfo, utc};

const DOMAIN: &str = "linear.app";
const API_DOMAIN: &str = "api.linear.app";

pub(crate) struct LinearIntegration {
    api_url: Url,
    token: SecretString,
    client: reqwest::Client,
}

#[derive(Deserialize)]
struct GraphQlResponse<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<GraphQlError>,
}

#[derive(Deserialize)]
struct GraphQlError {
    message: String,
    #[serde(default)]
    extensions: GraphQlErrorExtensions,
}

#[derive(Default, Deserialize)]
struct GraphQlErrorExtensions {
    code: Option<String>,
}

#[derive(Deserialize)]
struct IssueData {
    issue: Issue,
}

#[derive(Deserialize)]
struct Issue {
    identifier: String,
    title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentCreateData {
    comment_create: CommentCreate,
}

#[derive(Deserialize)]
struct CommentCreate {
    success: bool,
}

impl LinearIntegration {
    pub(crate) fn new(client: reqwest::Client, api_url: Url, token: SecretString) -> Self {
        Self {
            api_url,
            token,
            client,
        }
    }

    pub(crate) async fn test_connection(&self) -> Result<(), IntegrationError> {
        self.query::<serde_json::Value>("query { viewer { id } }", json!({}))
            .await?;
        Ok(())
    }

    pub(crate) async fn fetch_issue(
        &self,
        ticket_key: &str,
    ) -> Result<IssueInfo, IntegrationError> {
        let data: IssueData = self
            .query(
                "query($id: String!) { issue(id: $id) { identifier title } }",
                json!({ "id": ticket_key }),
            )
            .await
            .map_err(|e| not_found(e, ticket_key))?;
        Ok(IssueInfo {
            key: data.issue.identifier,
            title: data.issue.title,
        })
    }

    pub(crate) async fn publish_slot(
        &self,
        ticket_key: &str,
        started_at: i64,
        seconds: i64,
    ) -> Result<(), IntegrationError> {
        let body = format!(
            "Logged {}, started {}.",
            format_duration(seconds),
            utc(started_at)?
                .with_timezone(&Local)
                .format("%Y-%m-%d %H:%M %:z"),
        );
        let data: CommentCreateData = self
            .query(
                "mutation($issueId: String!, $body: String!) { \
                    commentCreate(input: { issueId: $issueId, body: $body }) { success } }",
                json!({ "issueId": ticket_key, "body": body }),
            )
            .await
            .map_err(|e| not_found(e, ticket_key))?;
        if !data.comment_create.success {
            return Err(IntegrationError::Api("comment was not created".to_string()));
        }
        Ok(())
    }

    async fn query<T: DeserializeOwned>(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<T, IntegrationError> {
        let response = self
            .client
            .post(self.api_url.clone())
            .header(AUTHORIZATION, self.token.expose_secret())
            .json(&json!({ "query": query, "variables": variables }))
            .send()
            .await?;
        debug!(status = %response.status(), "linear response");
        let response: GraphQlResponse<T> = response.json().await?;

        if let Some(error) = response.errors.into_iter().next() {
            return Err(match error.extensions.code.as_deref() {
                Some("AUTHENTICATION_ERROR") => IntegrationError::Auth(error.message),
                _ => IntegrationError::Api(error.message),
            });
        }
        response
            .data
            .ok_or_else(|| IntegrationError::Api("response has no data".to_string()))
    }
}

/// Returns the GraphQL endpoint for the workspace at `host_url`. The API of
/// linear.app is on its `api.` subdomain.
pub(crate) fn api_url(host_url: &str) -> Option<Url> {
    let mut url = Url::parse(host_url).ok()?;
    if url.host_str() == Some(DOMAIN) {
        url.set_host(Some(API_DOMAIN)).ok()?;
    }
    url.set_path("graphql");
    url.set_query(None);
    Some(url)
}

fn not_found(error: IntegrationError, ticket_key: &str) -> IntegrationError {
    match error {
        IntegrationError::Api(message) if message.starts_with("Entity not found") => {
            IntegrationError::TicketNotFound(ticket_key.to_string())
        }
        e => e,
    }
}

fn format_duration(seconds: i64) -> String {
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    [(hours, "h"), (minutes, "m"), (seconds, "s")]
        .into_iter()
        .filter(|(value, _)| *value > 0)
        .map(|(value, unit)| format!("{value}{unit}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use wiremock::matchers::{body_partial_json, header, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn linear(server: &MockServer) -> LinearIntegration {
        LinearIntegration::new(
            reqwest::Client::new(),
            Url::parse(&server.uri()).unwrap(),
            "token".into(),
        )
    }

    #[tokio::test]
    async fn publish_slot_creates_comment() {
        let server = MockServer::start().await;
        let started = Local.with_ymd_and_hms(2026, 11, 14, 9, 0, 0).unwrap();
        Mock::given(method("POST"))
            .and(header("authorization", "token"))
            .and(body_partial_json(json!({
                "variables": {
                    "issueId": "NIR-1",
                    "body": format!("Logged 1h 30m 5s, started 2026-11-14 09:00 {}.", started.format("%:z")),
                },
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "commentCreate": { "success": true } },
            })))
            .expect(1)
            .mount(&server)
            .await;

        linear(&server)
            .publish_slot("NIR-1", started.timestamp(), 5405)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn publish_slot_to_missing_issue_fails() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": null,
                "errors": [{
                    "message": "Entity not found: Issue",
                    "extensions": { "code": "INVALID_INPUT" },
                }],
            })))
            .mount(&server)
            .await;

        let err = linear(&server)
            .publish_slot("NIR-1", 1_794_646_800, 5400)
            .await
            .unwrap_err();

        assert!(matches!(err, IntegrationError::TicketNotFound(key) if key == "NIR-1"));
    }

    #[tokio::test]
    async fn publish_slot_fails_when_comment_is_not_created() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "commentCreate": { "success": false } },
            })))
            .mount(&server)
            .await;

        let err = linear(&server)
            .publish_slot("NIR-1", 1_794_646_800, 5400)
            .await
            .unwrap_err();

        assert!(matches!(err, IntegrationError::Api(_)));
    }

    #[test]
    fn api_url_uses_api_subdomain_of_linear_app() {
        assert_eq!(
            api_url("https://linear.app/nirvana").unwrap().as_str(),
            "https://api.linear.app/graphql"
        );
        assert_eq!(
            api_url("http://127.0.0.1:8080/nirvana").unwrap().as_str(),
            "http://127.0.0.1:8080/graphql"
        );
    }

    #[test]
    fn format_duration_shows_exact_time() {
        assert_eq!(format_duration(45), "45s");
        assert_eq!(format_duration(45 * 60), "45m");
        assert_eq!(format_duration(3600), "1h");
        assert_eq!(format_duration(3600 + 5 * 60 + 30), "1h 5m 30s");
    }
}
