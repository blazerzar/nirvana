//! Tickets.
//!
//! A ticket belongs to a profile and usually to one of its connections. It is
//! kept without a connection when its connection is deleted.

use nirvana_entity::connection::Entity as ConnectionEntity;
use nirvana_entity::{connection, ticket};
use sea_orm::sea_query::{Func, NullOrdering, Order};
use sea_orm::{EntityTrait, IntoSimpleExpr, QueryFilter, QueryOrder};

use crate::domain::{TicketConnection, TicketSort};
use crate::errors::{DomainError, NirvanaError};
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
}

#[cfg(test)]
mod tests {
    use nirvana_entity::credential::SecretStore;
    use sea_orm::{ActiveModelTrait, Set};

    use super::*;
    use crate::domain::{ConnectionData, Kind};
    use crate::service::test_support::test_service;

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
}
