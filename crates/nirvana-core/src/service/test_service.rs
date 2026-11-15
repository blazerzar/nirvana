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
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockBuilder, MockServer, ResponseTemplate};

use crate::domain::{
    Connection, ConnectionData, ConnectionEdit, Kind, TicketConnection, TicketSort,
};
use crate::paths::AppPaths;
use crate::secrets::mock_store;
use crate::service::Nirvana;

const UNREACHABLE_HOST: &str = "http://127.0.0.1:9";

/// A connection of the test service.
#[derive(Clone, Copy)]
pub(super) enum TestConnection {
    /// "Test Linear".
    Linear,
    /// "Test Jira", a Jira Cloud connection.
    Jira,
}

impl TestConnection {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Linear => "Test Linear",
            Self::Jira => "Test Jira",
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
            match connection {
                TestConnection::Linear => service.seed_linear().await,
                TestConnection::Jira => service.seed_jira().await,
            }
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

    pub(super) async fn seed_linear(&self) {
        self.add_connection(ConnectionData {
            name: TestConnection::Linear.name().to_string(),
            kind: Kind::Linear,
            host: self.host(),
            identity: None,
            secret_store: SecretStore::Plaintext,
            token: "token".into(),
        })
        .await
        .unwrap();
    }

    pub(super) async fn seed_jira(&self) {
        self.add_connection(ConnectionData {
            name: TestConnection::Jira.name().to_string(),
            kind: Kind::JiraCloud,
            host: self.host(),
            identity: Some("tester@example.com".to_string()),
            secret_store: SecretStore::Plaintext,
            token: "token".into(),
        })
        .await
        .unwrap();
    }

    /// Creates the profile `name` and makes it active.
    pub(super) async fn use_profile(&mut self, name: &str) {
        self.create_profile(name.to_string()).await.unwrap();
        self.switch_profile(name).await.unwrap();
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
        let kind = self.connection(connection).await.kind;
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

    /// Returns the passwords stored in the keyring.
    pub(super) fn keyring_passwords(&self) -> Vec<String> {
        let store = (self.keyring)().unwrap();
        let spec = [("service", "nirvana")].into();
        store
            .search(&spec)
            .unwrap()
            .into_iter()
            .filter_map(|entry| match entry.get_password() {
                Ok(password) => Some(password),
                Err(keyring_core::Error::NoEntry) => None,
                Err(e) => panic!("keyring read failed: {e}"),
            })
            .collect()
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
