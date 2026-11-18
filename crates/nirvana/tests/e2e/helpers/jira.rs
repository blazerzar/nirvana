use std::mem;
use std::sync::Mutex;

use chrono::DateTime;
use reqwest::{Method, RequestBuilder};
use serde_json::{Value, json};
use tokio::sync::OnceCell;

use crate::helpers::{Config, run_cleanup};

/// Jira client that deletes the issues it created when dropped.
pub struct Jira {
    client: reqwest::Client,
    url: String,
    email: String,
    token: String,
    project: String,
    issues: Mutex<Vec<String>>,
}

impl Jira {
    pub fn new(config: &Config) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: format!("https://{}/rest/api/2", config.jira_host),
            email: config.jira_email.clone(),
            token: config.jira_token.clone(),
            project: config.jira_project.clone(),
            issues: Mutex::default(),
        }
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.client
            .request(method, format!("{}/{path}", self.url))
            .basic_auth(&self.email, Some(&self.token))
    }

    async fn send(&self, request: RequestBuilder) -> Value {
        let response = request.send().await.unwrap();
        let status = response.status();
        let body = response.text().await.unwrap();
        assert!(status.is_success(), "Jira {status}: {body}");
        match body.is_empty() {
            true => Value::Null,
            false => serde_json::from_str(&body).unwrap(),
        }
    }

    /// Returns the ID of the issue type to create issues with, fetched once
    /// per run.
    async fn issue_type(&self) -> &'static str {
        static ISSUE_TYPE: OnceCell<String> = OnceCell::const_new();
        ISSUE_TYPE
            .get_or_init(|| async {
                let project = self
                    .send(self.request(Method::GET, &format!("project/{}", self.project)))
                    .await;
                let types: Vec<&Value> = project["issueTypes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|kind| {
                        kind["subtask"] == false
                            && kind["hierarchyLevel"].as_i64().unwrap_or(0) == 0
                    })
                    .collect();
                let issue_type = types
                    .iter()
                    .find(|kind| kind["name"] == "Task")
                    .or(types.first())
                    .expect("project has no issue type");
                issue_type["id"].as_str().unwrap().to_string()
            })
            .await
    }

    pub async fn create_issue(&self, summary: &str) -> String {
        let issue = self
            .send(self.request(Method::POST, "issue").json(&json!({
                "fields": {
                    "project": { "key": self.project },
                    "issuetype": { "id": self.issue_type().await },
                    "summary": summary,
                }
            })))
            .await;
        let key = issue["key"].as_str().unwrap().to_string();
        self.issues.lock().unwrap().push(key.clone());
        key
    }

    pub async fn rename_issue(&self, key: &str, summary: &str) {
        self.send(
            self.request(Method::PUT, &format!("issue/{key}"))
                .json(&json!({ "fields": { "summary": summary } })),
        )
        .await;
    }

    /// Returns the start timestamp and seconds of each worklog of `key`, sorted.
    pub async fn worklogs(&self, key: &str) -> Vec<(i64, i64)> {
        let response = self
            .send(self.request(Method::GET, &format!("issue/{key}/worklog")))
            .await;
        let mut worklogs: Vec<(i64, i64)> = response["worklogs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|worklog| {
                let started = DateTime::parse_from_str(
                    worklog["started"].as_str().unwrap(),
                    "%Y-%m-%dT%H:%M:%S%.3f%z",
                )
                .unwrap();
                (
                    started.timestamp(),
                    worklog["timeSpentSeconds"].as_i64().unwrap(),
                )
            })
            .collect();
        worklogs.sort();
        worklogs
    }
}

impl Drop for Jira {
    fn drop(&mut self) {
        let keys = mem::take(self.issues.get_mut().unwrap());
        if keys.is_empty() {
            return;
        }
        let jira = Self {
            client: reqwest::Client::new(),
            url: self.url.clone(),
            email: self.email.clone(),
            token: self.token.clone(),
            project: self.project.clone(),
            issues: Mutex::default(),
        };
        run_cleanup(async move {
            for key in keys {
                jira.send(jira.request(Method::DELETE, &format!("issue/{key}")))
                    .await;
            }
        });
    }
}
