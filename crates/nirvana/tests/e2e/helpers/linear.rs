use std::mem;
use std::sync::Mutex;

use chrono::{DateTime, Local, NaiveDate};
use serde_json::{Value, json};
use tokio::sync::OnceCell;

use crate::helpers::{Config, at, run_cleanup, timestamp};

const API_URL: &str = "https://api.linear.app/graphql";

/// Linear client that deletes the issues it created when dropped.
pub struct Linear {
    client: reqwest::Client,
    token: String,
    team: String,
    issues: Mutex<Vec<String>>,
}

impl Linear {
    pub fn new(config: &Config) -> Self {
        Self {
            client: reqwest::Client::new(),
            token: config.linear_token.clone(),
            team: config.linear_team.clone(),
            issues: Mutex::default(),
        }
    }

    async fn query(&self, query: &str, variables: Value) -> Value {
        let response = self
            .client
            .post(API_URL)
            .header("Authorization", &self.token)
            .json(&json!({ "query": query, "variables": variables }))
            .send()
            .await
            .unwrap();
        let status = response.status();
        let body = response.text().await.unwrap();
        let json: Value = serde_json::from_str(&body).unwrap_or_default();
        assert!(
            status.is_success() && json.get("data").is_some() && json.get("errors").is_none(),
            "Linear {status}: {body}"
        );
        json["data"].clone()
    }

    /// Returns the workspace URL key, fetched once per run.
    pub async fn workspace(&self) -> &'static str {
        static WORKSPACE: OnceCell<String> = OnceCell::const_new();
        WORKSPACE
            .get_or_init(|| async {
                let data = self
                    .query("query { organization { urlKey } }", json!({}))
                    .await;
                data["organization"]["urlKey"].as_str().unwrap().to_string()
            })
            .await
    }

    /// Returns the ID of the team, fetched once per run.
    async fn team_id(&self) -> &'static str {
        static TEAM_ID: OnceCell<String> = OnceCell::const_new();
        TEAM_ID
            .get_or_init(|| async {
                let teams = self
                    .query(
                        "query($key: String!) { teams(filter: { key: { eq: $key } }) { nodes { id } } }",
                        json!({ "key": self.team }),
                    )
                    .await;
                teams["teams"]["nodes"][0]["id"]
                    .as_str()
                    .expect("team not found")
                    .to_string()
            })
            .await
    }

    pub async fn create_issue(&self, title: &str) -> String {
        let data = self
            .query(
                "mutation($teamId: String!, $title: String!) { \
                    issueCreate(input: { teamId: $teamId, title: $title }) { issue { identifier } } }",
                json!({ "teamId": self.team_id().await, "title": title }),
            )
            .await;
        let identifier = data["issueCreate"]["issue"]["identifier"]
            .as_str()
            .unwrap()
            .to_string();
        self.issues.lock().unwrap().push(identifier.clone());
        identifier
    }

    pub async fn rename_issue(&self, identifier: &str, title: &str) {
        self.query(
            "mutation($id: String!, $title: String!) { \
                issueUpdate(id: $id, input: { title: $title }) { success } }",
            json!({ "id": identifier, "title": title }),
        )
        .await;
    }

    /// Returns the comment bodies of `identifier`, sorted.
    pub async fn comments(&self, identifier: &str) -> Vec<String> {
        let data = self
            .query(
                "query($id: String!) { issue(id: $id) { comments { nodes { body } } } }",
                json!({ "id": identifier }),
            )
            .await;
        let mut comments: Vec<String> = data["issue"]["comments"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["body"].as_str().unwrap().to_string())
            .collect();
        comments.sort();
        comments
    }
}

impl Drop for Linear {
    fn drop(&mut self) {
        let identifiers = mem::take(self.issues.get_mut().unwrap());
        if identifiers.is_empty() {
            return;
        }
        let linear = Self {
            client: reqwest::Client::new(),
            token: self.token.clone(),
            team: self.team.clone(),
            issues: Mutex::default(),
        };
        run_cleanup(async move {
            for identifier in identifiers {
                linear
                    .query(
                        "mutation($id: String!) { issueDelete(id: $id) { success } }",
                        json!({ "id": identifier }),
                    )
                    .await;
            }
        });
    }
}

/// Returns the comment nirvana posts for `duration` started at `time` on `day`.
pub fn comment(day: NaiveDate, time: &str, duration: &str) -> String {
    let offset = DateTime::from_timestamp(timestamp(day, time), 0)
        .unwrap()
        .with_timezone(&Local)
        .format("%:z");
    format!("Logged {duration}, started {} {offset}.", at(day, time))
}
