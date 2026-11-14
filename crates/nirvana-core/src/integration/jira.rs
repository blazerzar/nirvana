//! Jira Cloud and Jira Data Center through REST API v2.
//!
//! Jira Cloud authenticates with the email and API token (basic auth), Jira
//! Data Center with a personal access token (bearer auth). Time is published
//! as worklogs.

use reqwest::{Method, RequestBuilder, Response, StatusCode, Url};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tracing::debug;

use crate::integration::{IntegrationError, IssueInfo, utc};

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
        let started = utc(started_at)?.format("%Y-%m-%dT%H:%M:%S.000%z");

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
