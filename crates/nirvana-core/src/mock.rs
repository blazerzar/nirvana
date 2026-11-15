//! Mocks of the Jira and Linear APIs for tests.
//!
//! Jira and Linear requests differ by path, so one [`MockServer`] can serve
//! a Jira and a Linear connection side by side.

use serde_json::json;
use wiremock::matchers::{body_partial_json, body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::domain::Kind;

/// Answers Linear issue requests that are not mocked with "not found". Jira
/// gets a 404 for them from the mock server.
pub async fn mount_missing_issues(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": null,
            "errors": [{
                "message": "Entity not found: Issue",
                "extensions": { "code": "INVALID_INPUT" },
            }],
        })))
        .with_priority(u8::MAX)
        .mount(server)
        .await;
}

/// Mocks the ticket `key` with `title` in the API of `kind`. The ticket must
/// be requested at least once.
pub async fn mock_issue(server: &MockServer, kind: Kind, key: &str, title: &str) {
    let (request, response) = match kind {
        Kind::Linear => (
            Mock::given(method("POST"))
                .and(path("/graphql"))
                .and(body_partial_json(json!({ "variables": { "id": key } }))),
            ResponseTemplate::new(200).set_body_json(json!({
                "data": { "issue": { "identifier": key, "title": title } },
            })),
        ),
        Kind::JiraCloud | Kind::JiraDc => (
            Mock::given(method("GET")).and(path(format!("/rest/api/2/issue/{key}"))),
            ResponseTemplate::new(200).set_body_json(json!({
                "key": key,
                "fields": { "summary": title },
            })),
        ),
    };
    request
        .respond_with(response)
        .expect(1..)
        .mount(server)
        .await;
}

/// Mocks a Jira worklog of `seconds` on the ticket `key`. One such worklog
/// is expected.
pub async fn mock_worklog(server: &MockServer, key: &str, seconds: i64) {
    Mock::given(method("POST"))
        .and(path(format!("/rest/api/2/issue/{key}/worklog")))
        .and(body_partial_json(json!({ "timeSpentSeconds": seconds })))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(server)
        .await;
}

/// Mocks a Linear comment on the ticket `key` that logs `duration`, such as
/// "1h 5m". One such comment is expected.
pub async fn mock_comment(server: &MockServer, key: &str, duration: &str) {
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_partial_json(
            json!({ "variables": { "issueId": key } }),
        ))
        .and(body_string_contains(format!("Logged {duration},")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "commentCreate": { "success": true } },
        })))
        .expect(1)
        .mount(server)
        .await;
}
