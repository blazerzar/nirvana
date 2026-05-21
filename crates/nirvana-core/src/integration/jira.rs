use crate::integration::{IntegrationError, IssueInfo};

pub(crate) struct JiraIntegration {
    host: String,
    identity: String,
    token: String,
    client: reqwest::blocking::Client,
    kind: String,
}

impl JiraIntegration {
    pub(crate) fn new(host: &str, identity: &str, token: &str, kind: &str) -> Self {
        Self {
            host: host.to_string(),
            identity: identity.to_string(),
            token: token.to_string(),
            client: reqwest::blocking::Client::new(),
            kind: kind.to_string(),
        }
    }

    pub(crate) fn new_unauthenticated(host: &str, kind: &str) -> Self {
        Self {
            host: host.to_string(),
            identity: String::new(),
            token: String::new(),
            client: reqwest::blocking::Client::new(),
            kind: kind.to_string(),
        }
    }
}

impl JiraIntegration {
    pub(crate) fn get_issue_link(&self, ticket_key: &str) -> String {
        format!("https://{}/browse/{}", self.host, ticket_key)
    }

    pub(crate) fn test_connection(&self) -> Result<(), IntegrationError> {
        let url = format!("https://{}/rest/api/2/myself", self.host);
        let response = (match self.kind.as_str() {
            "jira-cloud" => self
                .client
                .get(&url)
                .basic_auth(&self.identity, Some(&self.token)),

            _ => self.client.get(&url).header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", self.token),
            ),
        })
        .send()?;

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

    pub(crate) fn fetch_issue(&self, ticket_key: &str) -> Result<IssueInfo, IntegrationError> {
        let url = format!(
            "https://{}/rest/api/2/issue/{}?fields=summary",
            self.host, ticket_key
        );

        let response = (match self.kind.as_str() {
            "jira-cloud" => self
                .client
                .get(&url)
                .basic_auth(&self.identity, Some(&self.token)),

            _ => self.client.get(&url).header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", self.token),
            ),
        })
        .send()?;

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

        let body: serde_json::Value = response.json()?;
        let summary = body["fields"]["summary"].as_str().unwrap_or("").to_string();

        Ok(IssueInfo { summary })
    }

    pub(crate) fn publish_slot(
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
                .basic_auth(&self.identity, Some(&self.token)),

            _ => self.client.post(&url).header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", self.token),
            ),
        })
        .json(&body)
        .send()?;

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
