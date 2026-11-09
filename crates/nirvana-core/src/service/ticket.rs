//! Tickets.
//!
//! A ticket belongs to a profile and usually to one of its connections. It is
//! kept without a connection when its connection is deleted.

use nirvana_entity::connection::Entity as ConnectionEntity;
use nirvana_entity::{connection, ticket};
use sea_orm::sea_query::{Func, NullOrdering, OnConflict, Order};
use sea_orm::{EntityTrait, IntoSimpleExpr, QueryFilter, QueryOrder, Set};
use tokio::task::JoinSet;
use tracing::{debug, warn};

use crate::domain::{Connection, TicketConnection, TicketSort};
use crate::errors::{DomainError, NirvanaError};
use crate::integration::{Integration, IntegrationError, IssueInfo};
use crate::secrets::read_credential;
use crate::service::Nirvana;

impl Nirvana {
    /// Returns the tickets of the active profile with their connections,
    /// sorted by `sort`. Keys, titles and connection names are sorted
    /// ascending and case-insensitively, timestamps descending. Ties are
    /// broken by the most recently worked on ticket.
    ///
    /// `connection` selects all tickets, the tickets of the connection with
    /// that name or the tickets without a connection.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable,
    /// the connection does not exist or a database query fails.
    pub async fn list_tickets(
        &self,
        connection: TicketConnection,
        sort: TicketSort,
    ) -> Result<Vec<(ticket::Model, Option<connection::Model>)>, NirvanaError> {
        let profile = self.find_active_profile().await?;
        let query = ticket::Entity::find()
            .find_also_related(ConnectionEntity)
            .filter(ticket::COLUMN.profile_id.eq(profile.id));
        let query = match connection {
            TicketConnection::Any => query,
            TicketConnection::Named(name) => {
                let connection =
                    ConnectionEntity::find_by_name_in_profile((name.clone(), profile.id))
                        .one(&self.db)
                        .await?
                        .ok_or(DomainError::ConnectionNotFound(name))?;
                query.filter(ticket::COLUMN.connection_id.eq(connection.id))
            }
            TicketConnection::Disconnected => query.filter(ticket::COLUMN.connection_id.is_null()),
        };
        let query = match sort {
            TicketSort::Key => {
                query.order_by_asc(Func::lower(ticket::COLUMN.ticket_key.into_simple_expr()))
            }
            TicketSort::Title => query.order_by_with_nulls(
                Func::lower(ticket::COLUMN.title.into_simple_expr()),
                Order::Asc,
                NullOrdering::Last,
            ),
            TicketSort::Connection => query.order_by_with_nulls(
                Func::lower(connection::COLUMN.name.into_simple_expr()),
                Order::Asc,
                NullOrdering::Last,
            ),
            TicketSort::LastWorked => query,
            TicketSort::Synced => query.order_by_desc(ticket::COLUMN.synced_at),
        };
        Ok(query
            .order_by_desc(ticket::COLUMN.last_worked_at)
            .order_by_asc(ticket::COLUMN.id)
            .all(&self.db)
            .await?)
    }

    /// Fetches the ticket `key` from the connection named `connection`, or
    /// from all connections of the active profile at once, and stores it
    /// with its title. A ticket that is already stored gets its title
    /// updated.
    ///
    /// When several connections are queried, connections that fail are
    /// skipped as long as exactly one connection finds the ticket.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable,
    /// the connection does not exist, no connection finds the ticket, more
    /// than one connection finds it, a connection fails and none finds the
    /// ticket, or a database query fails.
    pub async fn load_ticket(
        &self,
        key: &str,
        connection: Option<&str>,
    ) -> Result<ticket::Model, NirvanaError> {
        let connections = match connection {
            Some(name) => vec![self.get_connection(name).await?],
            None => self.list_connections().await?,
        };

        debug!(%key, connections = connections.len(), "loading ticket");
        let mut fetches = JoinSet::new();
        for (index, connection) in connections.into_iter().enumerate() {
            let integration =
                match read_credential(&self.keyring, connection.model.id, &self.db).await {
                    Ok(token) => Integration::new(&connection, token).map_err(NirvanaError::from),
                    Err(NirvanaError::Db(e)) => return Err(NirvanaError::Db(e)),
                    Err(e) => Err(e),
                };
            let key = key.to_string();
            fetches.spawn(async move {
                let issue = match integration {
                    Ok(integration) => integration
                        .fetch_issue(&key)
                        .await
                        .map_err(NirvanaError::from),
                    Err(e) => Err(e),
                };
                (index, connection, issue)
            });
        }

        let mut results = fetches.join_all().await;
        results.sort_by_key(|(index, ..)| *index);
        let results = results
            .into_iter()
            .map(|(_, connection, issue)| (connection, issue));
        let (connection, issue) = pick_issue(key, results)?;
        self.store_ticket(&connection, issue).await
    }

    /// Inserts the ticket of `connection` under the key the backend returned,
    /// or updates its title if it is already stored.
    async fn store_ticket(
        &self,
        connection: &Connection,
        issue: IssueInfo,
    ) -> Result<ticket::Model, NirvanaError> {
        let ticket = ticket::ActiveModel {
            profile_id: Set(connection.model.profile_id),
            connection_id: Set(Some(connection.model.id)),
            ticket_key: Set(issue.key),
            title: Set(Some(issue.title)),
            synced_at: Set(chrono::Utc::now().timestamp()),
            ..Default::default()
        };
        Ok(ticket::Entity::insert(ticket)
            .on_conflict(
                OnConflict::columns([ticket::Column::ConnectionId, ticket::Column::TicketKey])
                    .update_columns([ticket::Column::Title, ticket::Column::SyncedAt])
                    .to_owned(),
            )
            .exec_with_returning(&self.db)
            .await?)
    }
}

/// Picks the only connection that found the ticket `key`. Without one, the
/// first failure other than a missing ticket is returned.
fn pick_issue(
    key: &str,
    results: impl IntoIterator<Item = (Connection, Result<IssueInfo, NirvanaError>)>,
) -> Result<(Connection, IssueInfo), NirvanaError> {
    let mut found = Vec::new();
    let mut error = None;
    for (connection, result) in results {
        match result {
            Ok(issue) => found.push((connection, issue)),
            Err(NirvanaError::Integration(IntegrationError::TicketNotFound(_))) => {}
            Err(e) => {
                warn!(connection = %connection.model.name, error = %e, "cannot fetch ticket");
                error.get_or_insert(e);
            }
        }
    }
    match found.len() {
        0 => Err(error.unwrap_or_else(|| DomainError::TicketNotFound(key.to_string()).into())),
        1 => Ok(found.remove(0)),
        _ => Err(DomainError::AmbiguousTicket(
            key.to_string(),
            found
                .into_iter()
                .map(|(connection, _)| connection.model.name)
                .collect(),
        )
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use nirvana_entity::credential::SecretStore;
    use sea_orm::{ActiveModelTrait, Set};

    use super::*;
    use crate::domain::{ConnectionData, Kind};
    use crate::service::test_support::test_service;

    fn issue(key: &str, title: &str) -> Result<IssueInfo, NirvanaError> {
        Ok(IssueInfo {
            key: key.to_string(),
            title: title.to_string(),
        })
    }

    fn not_found(key: &str) -> Result<IssueInfo, NirvanaError> {
        Err(IntegrationError::TicketNotFound(key.to_string()).into())
    }

    fn auth_failed() -> Result<IssueInfo, NirvanaError> {
        Err(IntegrationError::Auth("401 Unauthorized".to_string()).into())
    }

    /// Adds a second Jira connection named "Other Jira" and returns both
    /// connections of the active profile.
    async fn two_connections(service: &Nirvana) -> (Connection, Connection) {
        service
            .add_connection(ConnectionData {
                name: "Other Jira".to_string(),
                kind: Kind::JiraDc,
                host: "jira.example.com".to_string(),
                identity: None,
                secret_store: SecretStore::Plaintext,
                token: "token".into(),
            })
            .await
            .unwrap();
        let mut connections = service.list_connections().await.unwrap().into_iter();
        (connections.next().unwrap(), connections.next().unwrap())
    }

    struct TicketData {
        key: &'static str,
        title: Option<&'static str>,
        connection_id: Option<i64>,
        last_worked_at: i64,
        synced_at: i64,
    }

    async fn add_ticket(service: &Nirvana, data: TicketData) {
        ticket::ActiveModel {
            profile_id: Set(service.find_active_profile().await.unwrap().id),
            connection_id: Set(data.connection_id),
            ticket_key: Set(data.key.to_string()),
            title: Set(data.title.map(str::to_string)),
            last_worked_at: Set(data.last_worked_at),
            synced_at: Set(data.synced_at),
            ..Default::default()
        }
        .insert(&service.db)
        .await
        .unwrap();
    }

    /// Adds a "linear" connection and three tickets to the active profile:
    /// "JIRA-1" in "Test Jira", "LIN-1" in "linear" and "OLD-1" without a
    /// connection.
    async fn add_tickets(service: &Nirvana) {
        let jira = service.list_connections().await.unwrap()[0].model.id;
        let linear = service
            .add_connection(ConnectionData {
                name: "linear".to_string(),
                kind: Kind::Linear,
                host: "linear.app".to_string(),
                identity: None,
                secret_store: SecretStore::Plaintext,
                token: "token".into(),
            })
            .await
            .unwrap()
            .model
            .id;
        for data in [
            TicketData {
                key: "JIRA-1",
                title: Some("Fix login"),
                connection_id: Some(jira),
                last_worked_at: 200,
                synced_at: 10,
            },
            TicketData {
                key: "LIN-1",
                title: None,
                connection_id: Some(linear),
                last_worked_at: 300,
                synced_at: 30,
            },
            TicketData {
                key: "OLD-1",
                title: Some("add export"),
                connection_id: None,
                last_worked_at: 100,
                synced_at: 20,
            },
        ] {
            add_ticket(service, data).await;
        }
    }

    async fn ticket_keys(
        service: &Nirvana,
        connection: TicketConnection,
        sort: TicketSort,
    ) -> Vec<String> {
        service
            .list_tickets(connection, sort)
            .await
            .unwrap()
            .into_iter()
            .map(|(ticket, _)| ticket.ticket_key)
            .collect()
    }

    #[tokio::test]
    async fn list_tickets_orders_by_last_worked_at_by_default() {
        let service = test_service().await;
        add_tickets(&service).await;

        assert_eq!(
            ticket_keys(&service, TicketConnection::Any, TicketSort::default()).await,
            ["LIN-1", "JIRA-1", "OLD-1"]
        );
    }

    #[tokio::test]
    async fn list_tickets_sorts_by_each_column() {
        let service = test_service().await;
        add_tickets(&service).await;

        for (sort, keys) in [
            (TicketSort::Key, ["JIRA-1", "LIN-1", "OLD-1"]),
            (TicketSort::Title, ["OLD-1", "JIRA-1", "LIN-1"]),
            (TicketSort::Connection, ["LIN-1", "JIRA-1", "OLD-1"]),
            (TicketSort::Synced, ["LIN-1", "OLD-1", "JIRA-1"]),
        ] {
            assert_eq!(
                ticket_keys(&service, TicketConnection::Any, sort).await,
                keys,
                "{sort:?}"
            );
        }
    }

    #[tokio::test]
    async fn list_tickets_shows_disconnected_tickets() {
        let service = test_service().await;
        add_tickets(&service).await;

        let tickets = service
            .list_tickets(TicketConnection::Any, TicketSort::default())
            .await
            .unwrap();

        let connections: Vec<_> = tickets
            .iter()
            .map(|(_, connection)| connection.as_ref().map(|c| c.name.as_str()))
            .collect();
        assert_eq!(connections, [Some("linear"), Some("Test Jira"), None]);
    }

    #[tokio::test]
    async fn list_tickets_filters_by_connection() {
        let service = test_service().await;
        add_tickets(&service).await;

        assert_eq!(
            ticket_keys(
                &service,
                TicketConnection::Named("Test Jira".to_string()),
                TicketSort::default()
            )
            .await,
            ["JIRA-1"]
        );
    }

    #[tokio::test]
    async fn list_tickets_filters_disconnected_tickets() {
        let service = test_service().await;
        add_tickets(&service).await;

        assert_eq!(
            ticket_keys(
                &service,
                TicketConnection::Disconnected,
                TicketSort::default()
            )
            .await,
            ["OLD-1"]
        );
    }

    #[tokio::test]
    async fn list_tickets_rejects_missing_connection() {
        let service = test_service().await;

        let err = service
            .list_tickets(
                TicketConnection::Named("missing".to_string()),
                TicketSort::default(),
            )
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::ConnectionNotFound(name)) if name == "missing"
        ));
    }

    #[tokio::test]
    async fn list_tickets_hides_other_profiles_tickets() {
        let mut service = test_service().await;
        add_tickets(&service).await;
        service.create_profile("work".to_string()).await.unwrap();
        service.switch_profile("work").await.unwrap();

        assert!(
            service
                .list_tickets(TicketConnection::Any, TicketSort::default())
                .await
                .unwrap()
                .is_empty()
        );
        let err = service
            .list_tickets(
                TicketConnection::Named("Test Jira".to_string()),
                TicketSort::default(),
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::ConnectionNotFound(_))
        ));
    }

    #[tokio::test]
    async fn load_ticket_rejects_missing_connection() {
        let service = test_service().await;

        let err = service
            .load_ticket("JIRA-1", Some("missing"))
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::ConnectionNotFound(name)) if name == "missing"
        ));
    }

    #[tokio::test]
    async fn load_ticket_without_connections_does_not_find_ticket() {
        let mut service = test_service().await;
        service.create_profile("work".to_string()).await.unwrap();
        service.switch_profile("work").await.unwrap();

        let err = service.load_ticket("JIRA-1", None).await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::TicketNotFound(key)) if key == "JIRA-1"
        ));
    }

    #[tokio::test]
    async fn pick_issue_returns_only_connection_that_found_ticket() {
        let service = test_service().await;
        let (first, second) = two_connections(&service).await;

        let (connection, issue) = pick_issue(
            "JIRA-1",
            vec![
                (first, not_found("JIRA-1")),
                (second, issue("JIRA-1", "Fix login")),
            ],
        )
        .unwrap();

        assert_eq!(connection.model.name, "Other Jira");
        assert_eq!(issue.title, "Fix login");
    }

    #[tokio::test]
    async fn pick_issue_skips_failed_connection_when_ticket_is_found() {
        let service = test_service().await;
        let (first, second) = two_connections(&service).await;

        let (connection, _) = pick_issue(
            "JIRA-1",
            vec![
                (first, auth_failed()),
                (second, issue("JIRA-1", "Fix login")),
            ],
        )
        .unwrap();

        assert_eq!(connection.model.name, "Other Jira");
    }

    #[tokio::test]
    async fn pick_issue_rejects_ticket_found_in_several_connections() {
        let service = test_service().await;
        let (first, second) = two_connections(&service).await;

        let err = pick_issue(
            "JIRA-1",
            vec![
                (first, issue("JIRA-1", "Fix login")),
                (second, issue("JIRA-1", "Fix logout")),
            ],
        )
        .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::AmbiguousTicket(key, connections))
                if key == "JIRA-1" && connections == ["Test Jira", "Other Jira"]
        ));
    }

    #[tokio::test]
    async fn pick_issue_reports_ticket_not_found_in_any_connection() {
        let service = test_service().await;
        let (first, second) = two_connections(&service).await;

        let err = pick_issue(
            "JIRA-1",
            vec![(first, not_found("JIRA-1")), (second, not_found("JIRA-1"))],
        )
        .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::TicketNotFound(key)) if key == "JIRA-1"
        ));
    }

    #[tokio::test]
    async fn pick_issue_reports_failure_when_ticket_is_not_found() {
        let service = test_service().await;
        let (first, second) = two_connections(&service).await;

        let err = pick_issue(
            "JIRA-1",
            vec![(first, not_found("JIRA-1")), (second, auth_failed())],
        )
        .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Integration(IntegrationError::Auth(_))
        ));
    }

    #[tokio::test]
    async fn store_ticket_inserts_ticket() {
        let service = test_service().await;
        let connection = service.get_connection("Test Jira").await.unwrap();

        let ticket = service
            .store_ticket(&connection, issue("JIRA-1", "Fix login").unwrap())
            .await
            .unwrap();

        assert_eq!(ticket.ticket_key, "JIRA-1");
        assert_eq!(ticket.title.as_deref(), Some("Fix login"));
        assert_eq!(ticket.connection_id, Some(connection.model.id));
        assert_eq!(ticket.profile_id, connection.model.profile_id);
        assert!(ticket.synced_at > 0);
    }

    #[tokio::test]
    async fn store_ticket_updates_stored_ticket() {
        let service = test_service().await;
        let connection = service.get_connection("Test Jira").await.unwrap();
        add_ticket(
            &service,
            TicketData {
                key: "JIRA-1",
                title: Some("Fix login"),
                connection_id: Some(connection.model.id),
                last_worked_at: 200,
                synced_at: 10,
            },
        )
        .await;

        let ticket = service
            .store_ticket(&connection, issue("JIRA-1", "Fix logout").unwrap())
            .await
            .unwrap();

        assert_eq!(ticket.title.as_deref(), Some("Fix logout"));
        assert_eq!(ticket.last_worked_at, 200);
        assert!(ticket.synced_at > 10);
        assert_eq!(
            ticket::Entity::find().all(&service.db).await.unwrap().len(),
            1
        );
    }
}
