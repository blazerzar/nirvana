//! Test service for the unit tests of the service modules.
//!
//! [`TestService`] is a [`Nirvana`] opened in a temporary directory with an
//! in-memory keyring and the active profile "test". It seeds connections,
//! tickets and slots, and mocks the Jira and Linear APIs.
//!
//! All connections share one mock server. Jira and Linear requests differ by
//! path, so a Jira and a Linear connection can be mocked side by side, but two
//! connections of the same kind get the same responses.

use std::ops::{Deref, DerefMut};

use nirvana_entity::credential::SecretStore;
use nirvana_entity::{publish_entry, slot, ticket};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;
use tempfile::TempDir;
use wiremock::matchers::{body_partial_json, body_string_contains, method, path};
use wiremock::{Mock, MockBuilder, MockServer, ResponseTemplate};

use crate::domain::{
    Connection, ConnectionData, ConnectionEdit, JiraSettingsEdit, Kind, LinearSettingsEdit,
    SettingsEdit, TicketConnection, TicketSort,
};
use crate::errors::NirvanaError;
use crate::paths::AppPaths;
use crate::secrets::{KEYRING_SERVICE, mock_store};
use crate::service::Nirvana;

const UNREACHABLE_HOST: &str = "http://127.0.0.1:9";

/// A connection of the test service.
#[derive(Clone, Copy)]
pub(super) enum TestConnection {
    /// "Test Linear".
    Linear,
    /// "Test Jira", a Jira Cloud connection.
    Jira,
    /// "Keyring Linear", with its token in the keyring.
    KeyringLinear,
    /// "Keyring Jira", a Jira Cloud connection with its token in the keyring.
    KeyringJira,
}

impl TestConnection {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Linear => "Test Linear",
            Self::Jira => "Test Jira",
            Self::KeyringLinear => "Keyring Linear",
            Self::KeyringJira => "Keyring Jira",
        }
    }

    pub(super) fn token(self) -> &'static str {
        match self {
            Self::Linear => "linear-token",
            Self::Jira => "jira-token",
            Self::KeyringLinear => "keyring-linear-token",
            Self::KeyringJira => "keyring-jira-token",
        }
    }
}

/// A service in a temporary directory. Seeded tickets go to the active
/// profile and the current connection, and mocked responses use the API of
/// the current connection's kind. Connections point to an unreachable host
/// until the mock server is started.
pub(super) struct TestService {
    service: Nirvana,
    current_connection: Option<TestConnection>,
    mock: Option<MockServer>,
    _dir: TempDir,
}

impl Deref for TestService {
    type Target = Nirvana;

    fn deref(&self) -> &Self::Target {
        &self.service
    }
}

impl DerefMut for TestService {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.service
    }
}

impl TestService {
    /// Returns a service with the active profile "test" and `connections`,
    /// without a current connection.
    pub(super) async fn new(connections: &[TestConnection]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let service = Nirvana::open(AppPaths::in_dir(dir.path()), mock_store())
            .await
            .unwrap();
        let mut service = Self {
            service,
            current_connection: None,
            mock: None,
            _dir: dir,
        };
        service.use_profile("test").await;
        for connection in connections {
            service.seed(*connection).await;
        }
        service
    }

    /// Starts a mock server that finds no tickets unless they are mocked,
    /// and points the connections of the active profile to it.
    pub(super) async fn start_mock(&mut self) {
        let mock = MockServer::start().await;
        mount_missing_issues(&mock).await;
        for connection in self.list_connections().await.unwrap() {
            self.edit_connection(
                &connection.model.name,
                ConnectionEdit {
                    host: Some(mock.uri()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        }
        self.mock = Some(mock);
    }

    fn host(&self) -> String {
        self.mock
            .as_ref()
            .map_or_else(|| UNREACHABLE_HOST.to_string(), MockServer::uri)
    }

    /// Returns the data `connection` is seeded with.
    pub(super) fn connection_data(&self, connection: TestConnection) -> ConnectionData {
        let (kind, identity) = match connection {
            TestConnection::Linear | TestConnection::KeyringLinear => (Kind::Linear, None),
            TestConnection::Jira | TestConnection::KeyringJira => {
                (Kind::JiraCloud, Some("tester@example.com".to_string()))
            }
        };
        let secret_store = match connection {
            TestConnection::Linear | TestConnection::Jira => SecretStore::Plaintext,
            TestConnection::KeyringLinear | TestConnection::KeyringJira => SecretStore::Keyring,
        };
        ConnectionData {
            name: connection.name().to_string(),
            kind,
            host: self.host(),
            identity,
            secret_store,
            token: connection.token().into(),
        }
    }

    /// Adds `connection` to the active profile.
    pub(super) async fn seed(&self, connection: TestConnection) {
        self.add_connection(self.connection_data(connection))
            .await
            .unwrap();
    }

    /// Creates the profile `name` and makes it active.
    pub(super) async fn use_profile(&mut self, name: &str) {
        self.create_profile(name.to_string()).await.unwrap();
        self.switch_profile(name).await.unwrap();
    }

    /// Edits the settings of `connection`, keeping the ones that are `None`.
    pub(super) async fn edit_settings(
        &self,
        connection: TestConnection,
        publish: Option<bool>,
        squash: Option<bool>,
    ) -> Result<(), NirvanaError> {
        let edit = match self.connection_data(connection).kind {
            Kind::Linear => SettingsEdit::Linear(LinearSettingsEdit { publish, squash }),
            Kind::JiraCloud | Kind::JiraDc => {
                SettingsEdit::Jira(JiraSettingsEdit { publish, squash })
            }
        };
        self.edit_connection_settings(connection.name(), edit).await
    }

    /// Makes `connection` the connection of seeded tickets, or seeds them
    /// without one.
    pub(super) fn use_connection(&mut self, connection: Option<TestConnection>) {
        self.current_connection = connection;
    }

    /// Returns `connection` of the active profile.
    pub(super) async fn connection(&self, connection: TestConnection) -> Connection {
        self.get_connection(connection.name()).await.unwrap()
    }

    pub(super) async fn seed_ticket(
        &self,
        key: &str,
        title: &str,
        last_worked_at: i64,
        synced_at: i64,
    ) -> ticket::Model {
        let connection_id = match self.current_connection {
            Some(connection) => Some(self.connection(connection).await.model.id),
            None => None,
        };
        ticket::ActiveModel {
            profile_id: Set(self.find_active_profile().await.unwrap().id),
            connection_id: Set(connection_id),
            ticket_key: Set(key.to_string()),
            title: Set(title.to_string()),
            last_worked_at: Set(last_worked_at),
            synced_at: Set(synced_at),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .unwrap()
    }

    pub(super) async fn seed_slot(
        &self,
        ticket: &ticket::Model,
        started_at: i64,
        stopped_at: Option<i64>,
    ) -> slot::Model {
        slot::ActiveModel {
            ticket_id: Set(ticket.id),
            profile_id: Set(ticket.profile_id),
            started_at: Set(started_at),
            stopped_at: Set(stopped_at),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .unwrap()
    }

    pub(super) async fn seed_publish_entry(
        &self,
        slot: &slot::Model,
        connection_id: i64,
        published_at: Option<i64>,
        error: Option<&str>,
        claimed_at: i64,
    ) -> publish_entry::Model {
        publish_entry::ActiveModel {
            slot_id: Set(slot.id),
            connection_id: Set(connection_id),
            published_at: Set(published_at),
            error: Set(error.map(str::to_string)),
            claimed_at: Set(claimed_at),
        }
        .insert(&self.db)
        .await
        .unwrap()
    }

    /// Returns the keys of the tickets that `list_tickets` returns.
    pub(super) async fn ticket_keys(
        &self,
        connection: TicketConnection,
        sort: TicketSort,
    ) -> Vec<String> {
        self.list_tickets(connection, sort)
            .await
            .unwrap()
            .into_iter()
            .map(|(ticket, _)| ticket.ticket_key)
            .collect()
    }

    /// Mocks the ticket `key` with `title`. The ticket must be requested at
    /// least once.
    pub(super) async fn mock_issue(&self, key: &str, title: &str) {
        let mock = self.mock.as_ref().expect("mocks need start_mock");
        let connection = self
            .current_connection
            .expect("mocks need a current connection");
        let kind = self.connection_data(connection).kind;
        let response = match kind {
            Kind::Linear => ResponseTemplate::new(200).set_body_json(json!({
                "data": { "issue": { "identifier": key, "title": title } },
            })),
            Kind::JiraCloud | Kind::JiraDc => ResponseTemplate::new(200).set_body_json(json!({
                "key": key,
                "fields": { "summary": title },
            })),
        };
        issue_request(kind, key)
            .respond_with(response)
            .expect(1..)
            .mount(mock)
            .await;
    }

    /// Mocks a Jira worklog of `seconds` on the ticket `key`. One such
    /// worklog is expected.
    pub(super) async fn mock_worklog(&self, key: &str, seconds: i64) {
        Mock::given(method("POST"))
            .and(path(format!("/rest/api/2/issue/{key}/worklog")))
            .and(body_partial_json(json!({ "timeSpentSeconds": seconds })))
            .respond_with(ResponseTemplate::new(201))
            .expect(1)
            .mount(self.mock_server())
            .await;
    }

    /// Mocks a Linear comment on the ticket `key` that logs `duration`, such
    /// as "1h 5m". One such comment is expected.
    pub(super) async fn mock_comment(&self, key: &str, duration: &str) {
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
            .mount(self.mock_server())
            .await;
    }

    fn mock_server(&self) -> &MockServer {
        self.mock.as_ref().expect("mocks need start_mock")
    }

    /// Returns the paths of the requests the mock server received.
    pub(super) async fn requested_paths(&self) -> Vec<String> {
        let mock = self.mock.as_ref().expect("requests need start_mock");
        mock.received_requests()
            .await
            .unwrap()
            .into_iter()
            .map(|request| request.url.path().to_string())
            .collect()
    }

    fn keyring_entries(&self) -> Vec<keyring_core::Entry> {
        let store = (self.keyring)().unwrap();
        store
            .search(&[("service", KEYRING_SERVICE)].into())
            .unwrap()
    }

    /// Returns the passwords stored in the keyring.
    pub(super) fn keyring_passwords(&self) -> Vec<String> {
        self.keyring_entries()
            .into_iter()
            .filter_map(|entry| match entry.get_password() {
                Ok(password) => Some(password),
                Err(keyring_core::Error::NoEntry) => None,
                Err(e) => panic!("keyring read failed: {e}"),
            })
            .collect()
    }

    /// Deletes every password stored in the keyring.
    pub(super) fn clear_keyring(&self) {
        for entry in self.keyring_entries() {
            entry.delete_credential().unwrap();
        }
    }
}

/// Answers Linear issue requests that are not mocked with "not found". Jira
/// gets a 404 for them from the mock server.
async fn mount_missing_issues(server: &MockServer) {
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

fn issue_request(kind: Kind, key: &str) -> MockBuilder {
    match kind {
        Kind::Linear => Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_partial_json(json!({ "variables": { "id": key } }))),
        Kind::JiraCloud | Kind::JiraDc => {
            Mock::given(method("GET")).and(path(format!("/rest/api/2/issue/{key}")))
        }
    }
}
