use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};

use crate::integration::{IntegrationError, IssueInfo};

pub(crate) struct JiraIntegration {
    host: String,
    identity: String,
    token: SecretString,
    client: reqwest::Client,
    kind: String,
}

impl JiraIntegration {
    pub(crate) fn new(host: &str, identity: &str, token: &SecretString, kind: &str) -> Self {
        Self {
            host: host.to_string(),
            identity: identity.to_string(),
            token: token.clone(),
            client: reqwest::Client::new(),
            kind: kind.to_string(),
        }
    }

    pub(crate) fn new_unauthenticated(host: &str, kind: &str) -> Self {
        Self {
            host: host.to_string(),
            identity: String::new(),
            token: SecretString::default(),
            client: reqwest::Client::new(),
            kind: kind.to_string(),
        }
    }
}

impl JiraIntegration {
    pub(crate) fn get_issue_link(&self, ticket_key: &str) -> String {
        format!("https://{}/browse/{}", self.host, ticket_key)
    }

    pub(crate) async fn test_connection(&self) -> Result<(), IntegrationError> {
        let url = format!("https://{}/rest/api/2/myself", self.host);
        let response = (match self.kind.as_str() {
            "jira-cloud" => self
                .client
                .get(&url)
                .basic_auth(&self.identity, Some(self.token.expose_secret())),

            _ => self
                .client
                .get(&url)
                .bearer_auth(self.token.expose_secret()),
        })
        .timeout(Duration::from_secs(30))
        .send()
        .await?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(IntegrationError::Auth(status.to_string()));
        }
        if !status.is_success() {
            return Err(IntegrationError::Network(
                response.error_for_status_ref().unwrap_err(),
            ));
        }
        Ok(())
    }

    pub(crate) async fn fetch_issue(
        &self,
        ticket_key: &str,
    ) -> Result<IssueInfo, IntegrationError> {
        let url = format!(
            "https://{}/rest/api/2/issue/{}?fields=summary",
            self.host, ticket_key
        );

        let response = (match self.kind.as_str() {
            "jira-cloud" => self
                .client
                .get(&url)
                .basic_auth(&self.identity, Some(self.token.expose_secret())),

            _ => self
                .client
                .get(&url)
                .bearer_auth(self.token.expose_secret()),
        })
        .timeout(Duration::from_secs(30))
        .send()
        .await?;

        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(IntegrationError::TicketNotFound(ticket_key.to_string()));
        }
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(IntegrationError::Auth(status.to_string()));
        }
        if !status.is_success() {
            return Err(IntegrationError::Network(
                response.error_for_status_ref().unwrap_err(),
            ));
        }

        let body: serde_json::Value = response.json().await?;
        let summary = body["fields"]["summary"].as_str().unwrap_or("").to_string();

        Ok(IssueInfo { summary })
    }

    pub(crate) async fn publish_slot(
        &self,
        ticket_key: &str,
        started_at: i64,
        seconds: i64,
    ) -> Result<(), IntegrationError> {
        let url = format!(
            "https://{}/rest/api/2/issue/{}/worklog",
            self.host, ticket_key
        );

        let started = chrono::DateTime::from_timestamp(started_at, 0)
            .unwrap()
            .format("%Y-%m-%dT%H:%M:%S.000%z");

        let body = serde_json::json!({
            "timeSpentSeconds": seconds,
            "started": started.to_string(),
        });

        let response = (match self.kind.as_str() {
            "jira-cloud" => self
                .client
                .post(&url)
                .basic_auth(&self.identity, Some(self.token.expose_secret())),

            _ => self
                .client
                .post(&url)
                .bearer_auth(self.token.expose_secret()),
        })
        .json(&body)
        .timeout(Duration::from_secs(30))
        .send()
        .await?;

        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(IntegrationError::TicketNotFound(ticket_key.to_string()));
        }
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(IntegrationError::Auth(status.to_string()));
        }
        if !status.is_success() {
            return Err(IntegrationError::Network(
                response.error_for_status_ref().unwrap_err(),
            ));
        }

        Ok(())
    }
}
