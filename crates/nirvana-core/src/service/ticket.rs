//! Tickets.
//!
//! A ticket belongs to a profile and usually to one of its connections. It is
//! kept without a connection when its connection is deleted.

use nirvana_entity::connection::Entity as ConnectionEntity;
use nirvana_entity::{connection, slot, ticket};
use sea_orm::sea_query::{Func, NullOrdering, OnConflict, Order};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, EntityTrait, IntoSimpleExpr, PaginatorTrait, QueryFilter,
    QueryOrder, Set, TransactionTrait,
};
use tokio::task::JoinSet;
use tracing::{debug, warn};

use crate::domain::{Connection, TicketConnection, TicketSort, TicketTarget};
use crate::errors::{DomainError, NirvanaError};
use crate::integration::{Integration, IntegrationError, IssueInfo};
use crate::secrets::read_credential;
use crate::service::Nirvana;
use crate::service::connection::find_connection_in_profile;

impl Nirvana {
    /// Returns the tickets of the active profile with their connections,
    /// sorted by `sort`. Keys, titles and connection names are sorted
    /// ascending and case-insensitively, timestamps descending. Ties are
    /// broken by the most recently worked on ticket.
    ///
    /// `connection` selects all tickets, the tickets of the connection with
    /// that name, the tickets with a connection or the tickets without one.
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
                let connection = find_connection_in_profile(&self.db, profile.id, name).await?;
                query.filter(ticket::COLUMN.connection_id.eq(connection.id))
            }
            TicketConnection::Connected => query.filter(ticket::COLUMN.connection_id.is_not_null()),
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

    /// Deletes the ticket of the active profile with the key and optional
    /// connection name, or with the ID, of `target`. A ticket with slots is
    /// only deleted with `with_slots`, which deletes its slots too.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable,
    /// the connection does not exist, no ticket or more than one ticket
    /// matches, the ticket has slots and `with_slots` is not set, or a
    /// database query fails.
    pub async fn delete_ticket(
        &self,
        target: TicketTarget,
        with_slots: bool,
    ) -> Result<ticket::Model, NirvanaError> {
        let profile = self.find_active_profile().await?;
        let txn = self.db.begin().await?;
        let ticket = find_ticket(&txn, profile.id, target).await?;

        let slots = slot::Entity::find()
            .filter(slot::COLUMN.ticket_id.eq(ticket.id))
            .count(&txn)
            .await?;
        if slots > 0 {
            if !with_slots {
                return Err(DomainError::TicketHasSlots(ticket.ticket_key, slots).into());
            }
            slot::Entity::delete_many()
                .filter(slot::COLUMN.ticket_id.eq(ticket.id))
                .exec(&txn)
                .await?;
        }
        ticket::Entity::delete_by_id(ticket.id).exec(&txn).await?;
        txn.commit().await?;
        Ok(ticket)
    }

    /// Fetches the title of the ticket `key` of the active profile from the
    /// ticket's connection and stores it. `connection` selects the ticket of
    /// the connection with that name.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable,
    /// the connection does not exist, no ticket or more than one ticket
    /// matches, the ticket has no connection, the credential cannot be read,
    /// fetching the ticket fails or a database query fails.
    pub async fn sync_ticket(
        &self,
        key: &str,
        connection: Option<&str>,
    ) -> Result<ticket::Model, NirvanaError> {
        let profile = self.find_active_profile().await?;
        let target = TicketTarget::Key {
            key: key.to_string(),
            connection: connection.map(str::to_string),
        };
        let ticket = find_ticket(&self.db, profile.id, target).await?;
        let connection_id = ticket
            .connection_id
            .ok_or_else(|| DomainError::TicketHasNoConnection(ticket.ticket_key.clone()))?;
        let connection = self.find_connection(connection_id).await?;

        debug!(key = %ticket.ticket_key, connection = %connection.model.name, "syncing ticket");
        let token = read_credential(&self.keyring, connection.model.id, &self.db).await?;
        let issue = Integration::new(&connection, token)?
            .fetch_issue(&ticket.ticket_key)
            .await?;
        self.update_title(ticket, issue.title).await
    }

    /// Updates the title and sync time of the stored `ticket`.
    async fn update_title(
        &self,
        ticket: ticket::Model,
        title: String,
    ) -> Result<ticket::Model, NirvanaError> {
        let mut ticket: ticket::ActiveModel = ticket.into();
        ticket.title = Set(Some(title));
        ticket.synced_at = Set(chrono::Utc::now().timestamp());
        Ok(ticket.update(&self.db).await?)
    }

    /// Inserts the ticket of `connection` under the key the backend returned,
    /// or updates its title if it is already stored.
    pub(super) async fn store_ticket(
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

    /// Returns the stored ticket `key` of the profile, or loads it from the
    /// connections when it is not stored. `connection` selects the ticket of
    /// the connection with that name and limits loading to it.
    pub(super) async fn find_or_load_ticket(
        &self,
        profile_id: i64,
        key: &str,
        connection: Option<&str>,
    ) -> Result<ticket::Model, NirvanaError> {
        let target = TicketTarget::Key {
            key: key.to_string(),
            connection: connection.map(str::to_string),
        };
        match find_ticket(&self.db, profile_id, target).await {
            Err(NirvanaError::Domain(DomainError::TicketNotFound(_))) => {
                self.load_ticket(key, connection).await
            }
            result => result,
        }
    }
}

/// Returns the ticket of the profile with the key and optional connection
/// name, or with the ID, of `target`.
pub(super) async fn find_ticket(
    db: &impl ConnectionTrait,
    profile_id: i64,
    target: TicketTarget,
) -> Result<ticket::Model, NirvanaError> {
    match target {
        TicketTarget::Key { key, connection } => {
            let query = ticket::Entity::find_by_profile_key(profile_id, &key);
            let query = match connection {
                Some(name) => {
                    let connection = find_connection_in_profile(db, profile_id, name).await?;
                    query.filter(ticket::COLUMN.connection_id.eq(connection.id))
                }
                None => query,
            };
            let mut found = query
                .find_also_related(ConnectionEntity)
                .order_by_asc(ticket::COLUMN.id)
                .all(db)
                .await?;
            match found.len() {
                0 => Err(DomainError::TicketNotFound(key).into()),
                1 => Ok(found.remove(0).0),
                _ => {
                    let connections = found
                        .into_iter()
                        .map(|(_, connection)| {
                            connection.map_or_else(|| "no connection".to_string(), |c| c.name)
                        })
                        .collect();
                    Err(DomainError::AmbiguousTicket(key, connections).into())
                }
            }
        }
        TicketTarget::Id(id) => Ok(ticket::Entity::find_by_id(id)
            .filter(ticket::COLUMN.profile_id.eq(profile_id))
            .one(db)
            .await?
            .ok_or(DomainError::TicketIdNotFound(id))?),
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

    async fn add_ticket(service: &Nirvana, data: TicketData) -> ticket::Model {
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
        .unwrap()
    }

    /// Adds a "linear" connection and three tickets to the active profile:
    /// "JIRA-1" in "Test Jira", "LIN-1" in "linear" and "OLD-1" without a
    /// connection.
    async fn add_tickets(service: &Nirvana) -> [ticket::Model; 3] {
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
        let mut tickets = Vec::new();
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
            tickets.push(add_ticket(service, data).await);
        }
        tickets.try_into().unwrap()
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
    async fn list_tickets_filters_connected_tickets() {
        let service = test_service().await;
        add_tickets(&service).await;

        assert_eq!(
            ticket_keys(&service, TicketConnection::Connected, TicketSort::default()).await,
            ["LIN-1", "JIRA-1"]
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

    fn key_target(key: &str, connection: Option<&str>) -> TicketTarget {
        TicketTarget::Key {
            key: key.to_string(),
            connection: connection.map(str::to_string),
        }
    }

    /// Adds the tickets of `add_tickets` and a disconnected "JIRA-1", which
    /// it returns.
    async fn add_duplicate_ticket(service: &Nirvana) -> ticket::Model {
        add_tickets(service).await;
        add_ticket(
            service,
            TicketData {
                key: "JIRA-1",
                title: None,
                connection_id: None,
                last_worked_at: 0,
                synced_at: 0,
            },
        )
        .await
    }

    #[tokio::test]
    async fn find_ticket_ignores_key_case() {
        let service = test_service().await;
        let [jira, ..] = add_tickets(&service).await;
        let profile_id = service.find_active_profile().await.unwrap().id;

        let ticket = find_ticket(&service.db, profile_id, key_target("jira-1", None))
            .await
            .unwrap();

        assert_eq!(ticket, jira);
    }

    #[tokio::test]
    async fn delete_ticket_rejects_ambiguous_key() {
        let service = test_service().await;
        add_duplicate_ticket(&service).await;

        let err = service
            .delete_ticket(key_target("JIRA-1", None), false)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::AmbiguousTicket(key, connections))
                if key == "JIRA-1" && connections == ["Test Jira", "no connection"]
        ));
    }

    #[tokio::test]
    async fn delete_ticket_deletes_match_in_connection() {
        let service = test_service().await;
        let disconnected = add_duplicate_ticket(&service).await;

        let ticket = service
            .delete_ticket(key_target("JIRA-1", Some("Test Jira")), false)
            .await
            .unwrap();

        assert_ne!(ticket, disconnected);
        assert_eq!(
            ticket_keys(&service, TicketConnection::Any, TicketSort::default()).await,
            ["LIN-1", "OLD-1", "JIRA-1"]
        );
    }

    #[tokio::test]
    async fn delete_ticket_deletes_ticket_by_id() {
        let service = test_service().await;
        let disconnected = add_duplicate_ticket(&service).await;

        let ticket = service
            .delete_ticket(TicketTarget::Id(disconnected.id), false)
            .await
            .unwrap();

        assert_eq!(ticket, disconnected);
        assert_eq!(
            ticket_keys(&service, TicketConnection::Any, TicketSort::default()).await,
            ["LIN-1", "JIRA-1", "OLD-1"]
        );
    }

    #[tokio::test]
    async fn delete_ticket_deletes_disconnected_match() {
        let service = test_service().await;
        add_tickets(&service).await;

        service
            .delete_ticket(key_target("OLD-1", None), false)
            .await
            .unwrap();

        assert_eq!(
            ticket_keys(&service, TicketConnection::Any, TicketSort::default()).await,
            ["LIN-1", "JIRA-1"]
        );
    }

    #[tokio::test]
    async fn delete_ticket_rejects_missing_ticket_or_connection() {
        let service = test_service().await;
        add_tickets(&service).await;

        let err = service
            .delete_ticket(key_target("JIRA-2", None), false)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::TicketNotFound(key)) if key == "JIRA-2"
        ));

        let err = service
            .delete_ticket(key_target("JIRA-1", Some("missing")), false)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::ConnectionNotFound(name)) if name == "missing"
        ));
    }

    #[tokio::test]
    async fn delete_ticket_keeps_other_profiles_tickets() {
        let mut service = test_service().await;
        let [.., other] = add_tickets(&service).await;
        service.create_profile("work".to_string()).await.unwrap();
        service.switch_profile("work").await.unwrap();

        let err = service
            .delete_ticket(key_target("OLD-1", None), false)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::TicketNotFound(_))
        ));
        let err = service
            .delete_ticket(TicketTarget::Id(other.id), false)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::TicketIdNotFound(id)) if id == other.id
        ));
        assert_eq!(ticket::Entity::find().count(&service.db).await.unwrap(), 3);
    }

    #[tokio::test]
    async fn delete_ticket_deletes_slots_only_with_slots() {
        let service = test_service().await;
        let [_, kept, deleted] = add_tickets(&service).await;
        for ticket in [&kept, &deleted, &deleted] {
            slot::ActiveModel {
                ticket_id: Set(ticket.id),
                profile_id: Set(ticket.profile_id),
                started_at: Set(0),
                stopped_at: Set(Some(60)),
                ..Default::default()
            }
            .insert(&service.db)
            .await
            .unwrap();
        }

        let err = service
            .delete_ticket(key_target("OLD-1", None), false)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::TicketHasSlots(key, 2)) if key == "OLD-1"
        ));
        assert_eq!(slot::Entity::find().count(&service.db).await.unwrap(), 3);

        service
            .delete_ticket(key_target("OLD-1", None), true)
            .await
            .unwrap();
        assert_eq!(
            ticket_keys(&service, TicketConnection::Any, TicketSort::default()).await,
            ["LIN-1", "JIRA-1"]
        );
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].ticket_id, kept.id);
    }

    #[tokio::test]
    async fn sync_ticket_rejects_disconnected_ticket() {
        let service = test_service().await;
        add_tickets(&service).await;

        let err = service.sync_ticket("OLD-1", None).await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::TicketHasNoConnection(key)) if key == "OLD-1"
        ));
    }

    #[tokio::test]
    async fn sync_ticket_rejects_ambiguous_key() {
        let service = test_service().await;
        add_duplicate_ticket(&service).await;

        let err = service.sync_ticket("JIRA-1", None).await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::AmbiguousTicket(key, _)) if key == "JIRA-1"
        ));
    }

    #[tokio::test]
    async fn sync_ticket_rejects_missing_ticket_or_connection() {
        let service = test_service().await;
        add_tickets(&service).await;

        let err = service.sync_ticket("JIRA-2", None).await.unwrap_err();
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::TicketNotFound(key)) if key == "JIRA-2"
        ));

        let err = service
            .sync_ticket("JIRA-1", Some("missing"))
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::ConnectionNotFound(name)) if name == "missing"
        ));
    }

    #[tokio::test]
    async fn update_title_keeps_key_and_last_worked_at() {
        let service = test_service().await;
        let [jira, ..] = add_tickets(&service).await;

        let ticket = service
            .update_title(jira.clone(), "Fix logout".to_string())
            .await
            .unwrap();

        assert_eq!(ticket.title.as_deref(), Some("Fix logout"));
        assert!(ticket.synced_at > jira.synced_at);
        assert_eq!(ticket.ticket_key, jira.ticket_key);
        assert_eq!(ticket.last_worked_at, jira.last_worked_at);
    }
}
