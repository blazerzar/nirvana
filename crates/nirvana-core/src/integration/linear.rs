//! Linear through its GraphQL API.
//!
//! The API endpoint is constant and the personal API key is sent as the
//! `Authorization` header. GraphQL errors are mapped to [`IntegrationError`]
//! by their code, and a missing issue by its message, since Linear reports it
//! as a generic input error.

use reqwest::header::AUTHORIZATION;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use tracing::debug;

use crate::integration::{IntegrationError, IssueInfo, TIMEOUT};

const API_URL: &str = "https://api.linear.app/graphql";

pub(crate) struct LinearIntegration {
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

impl LinearIntegration {
    pub(crate) fn new(token: SecretString) -> Self {
        Self {
            token,
            client: reqwest::Client::new(),
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
            .map_err(|e| match e {
                IntegrationError::Api(message) if message.starts_with("Entity not found") => {
                    IntegrationError::TicketNotFound(ticket_key.to_string())
                }
                e => e,
            })?;
        Ok(IssueInfo {
            key: data.issue.identifier,
            title: data.issue.title,
        })
    }

    async fn query<T: DeserializeOwned>(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<T, IntegrationError> {
        let response = self
            .client
            .post(API_URL)
            .header(AUTHORIZATION, self.token.expose_secret())
            .json(&json!({ "query": query, "variables": variables }))
            .timeout(TIMEOUT)
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
