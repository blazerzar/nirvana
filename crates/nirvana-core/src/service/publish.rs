//! Publishing.
//!
//! A stopped slot is published to its ticket's connection when that
//! connection has `publish` enabled: Jira as worklogs, Linear as comments. A
//! publish first claims its slots with pending entries in one transaction and
//! sends only the claimed ones, so concurrent publishes do not send a slot
//! twice. The result of each slot is stored in its entry, as ok or as failed
//! with the error. A pending entry claimed more than [`STUCK_AFTER`] seconds
//! ago was left by an interrupted publish.

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use nirvana_entity::{publish_entry, slot, ticket};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, EntityTrait, ExprTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait, TryInsertResult,
};
use tokio::task::JoinSet;
use tracing::debug;

use crate::domain::{PublishFailure, PublishResult, Slot};
use crate::errors::{DomainError, NirvanaError};
use crate::integration::Integration;
use crate::service::Nirvana;

/// Seconds after which a pending entry counts as stuck and is claimed again
/// on retry.
const STUCK_AFTER: i64 = 60;

impl Nirvana {
    /// Publishes the stopped slots of the active profile that overlap the
    /// range from `from` to `to`, each unbounded when `None`. Only slots
    /// without publish entries are sent, and with `retry` also failed and
    /// stuck ones.
    /// `target` limits publishing to the connection with that name, otherwise
    /// every connection with `publish` enabled is used.
    ///
    /// A slot that cannot be sent is returned as a failure and its error is
    /// stored in its publish entry. A slot whose result cannot be stored is
    /// returned as a failure too.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable,
    /// the target does not exist or has `publish` disabled, or a database
    /// query fails.
    pub async fn publish_slots(
        &self,
        from: Option<i64>,
        to: Option<i64>,
        target: Option<&str>,
        retry: bool,
    ) -> Result<PublishResult, NirvanaError> {
        let profile = self.find_active_profile().await?;
        let connections = match target {
            Some(name) => {
                let connection = self.get_connection(name).await?;
                if !connection.settings.publish() {
                    return Err(DomainError::PublishDisabled(name.to_owned()).into());
                }
                vec![connection]
            }
            None => self
                .find_connections(profile.id)
                .await?
                .into_iter()
                .filter(|connection| connection.settings.publish())
                .collect(),
        };

        let connection_ids = connections.iter().map(|connection| connection.model.id);
        let slots = find_publishable(&self.db, profile.id, from, to, connection_ids, retry).await?;
        let mut claimed = claim(&self.db, slots, retry).await?;
        debug!(
            claimed = claimed.values().map(Vec::len).sum::<usize>(),
            retry, "publishing slots"
        );

        let mut publishes = JoinSet::new();
        for connection in &connections {
            let Some(slots) = claimed.remove(&connection.model.id) else {
                continue;
            };
            let integration = self
                .integration(connection)
                .await
                .map_err(|e| error_chain(&e));
            publishes.spawn(publish_to_connection(
                self.db.clone(),
                connection.model.id,
                integration,
                slots,
            ));
        }

        let mut result = PublishResult::default();
        for published in publishes.join_all().await {
            result.published.extend(published.published);
            result.failed.extend(published.failed);
        }
        result.published.sort_by_key(|slot| slot.model.started_at);
        result
            .failed
            .sort_by_key(|failure| failure.slot.model.started_at);
        Ok(result)
    }
}

/// Sends the slots to the connection one by one and stores the result of
/// each. When `integration` could not be built, its error is stored for every
/// slot.
async fn publish_to_connection(
    db: DatabaseConnection,
    connection_id: i64,
    integration: Result<Integration, String>,
    slots: Vec<Slot>,
) -> PublishResult {
    let mut result = PublishResult::default();
    for slot in slots {
        let sent = match &integration {
            Ok(integration) => integration
                .publish_slot(
                    &slot.ticket.ticket_key,
                    slot.model.started_at,
                    slot.elapsed(),
                )
                .await
                .map_err(|e| error_chain(&e)),
            Err(error) => Err(error.clone()),
        };
        let stored = store_result(&db, slot.model.id, connection_id, sent.as_ref().err()).await;
        let error = match (sent, stored) {
            (Ok(()), Ok(())) => {
                result.published.push(slot);
                continue;
            }
            (Ok(()), Err(e)) => format!(
                "published, but storing the result failed: {}",
                error_chain(&e)
            ),
            (Err(error), Ok(())) => error,
            (Err(error), Err(e)) => {
                format!("{error}, and storing the error failed: {}", error_chain(&e))
            }
        };
        debug!(slot_id = slot.model.id, %error, "slot not published");
        result.failed.push(PublishFailure { slot, error });
    }
    result
}

/// Returns the stopped slots of the profile that overlap the range, whose
/// ticket belongs to one of `connection_ids` and that have no publish entry,
/// or with `retry` a failed or stuck one, grouped by that connection and
/// ordered by start.
async fn find_publishable(
    db: &impl ConnectionTrait,
    profile_id: i64,
    from: Option<i64>,
    to: Option<i64>,
    connection_ids: impl IntoIterator<Item = i64>,
    retry: bool,
) -> Result<HashMap<i64, Vec<Slot>>, NirvanaError> {
    let state = match retry {
        true => {
            publish_entry::Entity::unpublished().or(publish_entry::Entity::failed(stuck_before()))
        }
        false => publish_entry::Entity::unpublished(),
    };
    let rows = slot::Entity::find_in_range(profile_id, from.unwrap_or(i64::MIN), to)
        .filter(slot::COLUMN.stopped_at.is_not_null())
        .filter(state)
        .find_also_related(ticket::Entity)
        .filter(ticket::COLUMN.connection_id.is_in(connection_ids))
        .order_by_asc(slot::COLUMN.started_at)
        .all(db)
        .await?;

    let mut groups: HashMap<i64, Vec<Slot>> = HashMap::new();
    for (model, ticket) in rows {
        let ticket = ticket.ok_or(DomainError::TicketIdNotFound(model.ticket_id))?;
        let Some(connection_id) = ticket.connection_id else {
            continue;
        };
        groups
            .entry(connection_id)
            .or_default()
            .push(Slot { model, ticket });
    }
    Ok(groups)
}

/// Claims the slots, grouped by their ticket's connection, in one transaction
/// and returns the claimed ones. A slot without an entry gets a pending one,
/// and with `retry` a failed or stuck entry is reset to pending. Slots claimed
/// by another publish are skipped.
async fn claim(
    db: &DatabaseConnection,
    mut slots: HashMap<i64, Vec<Slot>>,
    retry: bool,
) -> Result<HashMap<i64, Vec<Slot>>, NirvanaError> {
    let targets: Vec<(i64, i64)> = slots
        .iter()
        .flat_map(|(&connection_id, slots)| {
            slots.iter().map(move |slot| (slot.model.id, connection_id))
        })
        .collect();
    if targets.is_empty() {
        return Ok(HashMap::new());
    }

    let now = Utc::now().timestamp();
    let txn = db.begin().await?;
    let inserted =
        publish_entry::Entity::insert_many(targets.iter().map(|&(slot_id, connection_id)| {
            publish_entry::ActiveModel {
                slot_id: Set(slot_id),
                connection_id: Set(connection_id),
                published_at: Set(None),
                error: Set(None),
                claimed_at: Set(now),
            }
        }))
        .on_conflict_do_nothing()
        .exec_with_returning_many(&txn)
        .await?;
    let mut entries = match inserted {
        TryInsertResult::Inserted(entries) => entries,
        TryInsertResult::Empty | TryInsertResult::Conflicted => Vec::new(),
    };
    if retry {
        let reset = publish_entry::Entity::update_many()
            .set(publish_entry::ActiveModel {
                error: Set(None),
                claimed_at: Set(now),
                ..Default::default()
            })
            .filter(
                Expr::tuple([
                    Expr::col(publish_entry::Column::SlotId),
                    Expr::col(publish_entry::Column::ConnectionId),
                ])
                .in_tuples(targets),
            )
            .filter(publish_entry::Entity::retryable(now - STUCK_AFTER))
            .exec_with_returning(&txn)
            .await?;
        entries.extend(reset);
    }
    txn.commit().await?;

    let claimed: HashSet<(i64, i64)> = entries
        .into_iter()
        .map(|entry| (entry.slot_id, entry.connection_id))
        .collect();
    slots.retain(|&connection_id, slots| {
        slots.retain(|slot| claimed.contains(&(slot.model.id, connection_id)));
        !slots.is_empty()
    });
    Ok(slots)
}

/// Stores the result of sending the slot to the connection in its entry: ok
/// with the publish time, or failed with `error`. Fails if the entry no
/// longer exists.
async fn store_result(
    db: &impl ConnectionTrait,
    slot_id: i64,
    connection_id: i64,
    error: Option<&String>,
) -> Result<(), NirvanaError> {
    let published_at = match error {
        Some(_) => None,
        None => Some(Utc::now().timestamp()),
    };
    let result = publish_entry::Entity::update_many()
        .set(publish_entry::ActiveModel {
            published_at: Set(published_at),
            error: Set(error.cloned()),
            ..Default::default()
        })
        .filter(publish_entry::COLUMN.slot_id.eq(slot_id))
        .filter(publish_entry::COLUMN.connection_id.eq(connection_id))
        .exec(db)
        .await?;
    if result.rows_affected == 0 {
        return Err(DomainError::PublishEntryNotFound(slot_id).into());
    }
    Ok(())
}

fn stuck_before() -> i64 {
    Utc::now().timestamp() - STUCK_AFTER
}

/// Formats the error with its sources, separated by colons.
fn error_chain(error: &dyn std::error::Error) -> String {
    std::iter::successors(Some(error), |e| e.source())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ")
}

#[cfg(test)]
mod tests {
    use nirvana_entity::credential::SecretStore;
    use sea_orm::{ActiveModelTrait, PaginatorTrait};

    use super::*;
    use crate::domain::{ConnectionData, Kind, LinearSettingsEdit, SettingsEdit};
    use crate::service::test_support::test_service;

    async fn add_ticket(service: &Nirvana, connection: &str, key: &str) -> ticket::Model {
        let connection = service.get_connection(connection).await.unwrap();
        ticket::ActiveModel {
            profile_id: Set(connection.model.profile_id),
            connection_id: Set(Some(connection.model.id)),
            ticket_key: Set(key.to_string()),
            synced_at: Set(0),
            ..Default::default()
        }
        .insert(&service.db)
        .await
        .unwrap()
    }

    async fn add_slot(
        service: &Nirvana,
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
        .insert(&service.db)
        .await
        .unwrap()
    }

    async fn add_entry(
        service: &Nirvana,
        slot: &slot::Model,
        ticket: &ticket::Model,
        published_at: Option<i64>,
        error: Option<&str>,
        claimed_at: i64,
    ) {
        publish_entry::ActiveModel {
            slot_id: Set(slot.id),
            connection_id: Set(ticket.connection_id.unwrap()),
            published_at: Set(published_at),
            error: Set(error.map(str::to_string)),
            claimed_at: Set(claimed_at),
        }
        .insert(&service.db)
        .await
        .unwrap();
    }

    async fn entry(service: &Nirvana, slot: &slot::Model) -> Option<publish_entry::Model> {
        publish_entry::Entity::find()
            .filter(publish_entry::COLUMN.slot_id.eq(slot.id))
            .one(&service.db)
            .await
            .unwrap()
    }

    async fn add_linear(service: &Nirvana, publish: bool) {
        service
            .add_connection(ConnectionData {
                name: "Test Linear".to_string(),
                kind: Kind::Linear,
                host: "linear.app/test".to_string(),
                identity: None,
                secret_store: SecretStore::Plaintext,
                token: "token".into(),
            })
            .await
            .unwrap();
        service
            .edit_connection_settings(
                "Test Linear",
                SettingsEdit::Linear(LinearSettingsEdit {
                    publish: Some(publish),
                    squash: None,
                }),
            )
            .await
            .unwrap();
    }

    fn ids(slots: &[Slot]) -> Vec<i64> {
        slots.iter().map(|slot| slot.model.id).collect()
    }

    #[tokio::test]
    async fn find_publishable_selects_stopped_unpublished_slots_in_range() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "Test Jira", "KAR-1").await;
        let before = add_slot(&service, &ticket, 1000, Some(2000)).await;
        let overlapping = add_slot(&service, &ticket, 2500, Some(3500)).await;
        let published = add_slot(&service, &ticket, 3500, Some(4000)).await;
        let failed = add_slot(&service, &ticket, 4000, Some(4500)).await;
        let stuck = add_slot(&service, &ticket, 4500, Some(4600)).await;
        let pending = add_slot(&service, &ticket, 4600, Some(4700)).await;
        add_slot(&service, &ticket, 5000, None).await;
        let now = Utc::now().timestamp();
        add_entry(&service, &published, &ticket, Some(4000), None, 4000).await;
        add_entry(&service, &failed, &ticket, None, Some("error"), now).await;
        add_entry(&service, &stuck, &ticket, None, None, now - STUCK_AFTER - 1).await;
        add_entry(&service, &pending, &ticket, None, None, now).await;
        let connection_id = ticket.connection_id.unwrap();

        let slots = find_publishable(
            &service.db,
            ticket.profile_id,
            Some(3000),
            None,
            [connection_id],
            false,
        )
        .await
        .unwrap();
        assert_eq!(ids(&slots[&connection_id]), [overlapping.id]);

        let slots = find_publishable(
            &service.db,
            ticket.profile_id,
            None,
            Some(5000),
            [connection_id],
            true,
        )
        .await
        .unwrap();
        assert_eq!(
            ids(&slots[&connection_id]),
            [before.id, overlapping.id, failed.id, stuck.id]
        );
    }

    #[tokio::test]
    async fn find_publishable_skips_other_connections() {
        let service = test_service().await;
        add_linear(&service, false).await;
        let ticket = add_ticket(&service, "Test Linear", "NIR-1").await;
        add_slot(&service, &ticket, 1000, Some(2000)).await;
        let jira = service.get_connection("Test Jira").await.unwrap();

        let slots = find_publishable(
            &service.db,
            ticket.profile_id,
            None,
            None,
            [jira.model.id],
            true,
        )
        .await
        .unwrap();

        assert!(slots.is_empty());
    }

    #[tokio::test]
    async fn claim_inserts_pending_entries_once() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "Test Jira", "KAR-1").await;
        let slot = add_slot(&service, &ticket, 1000, Some(2000)).await;
        let select = || {
            find_publishable(
                &service.db,
                ticket.profile_id,
                None,
                None,
                [ticket.connection_id.unwrap()],
                false,
            )
        };
        let slots = select().await.unwrap();
        let concurrent = select().await.unwrap();

        let claimed = claim(&service.db, slots, false).await.unwrap();
        let claimed_again = claim(&service.db, concurrent, false).await.unwrap();

        assert_eq!(ids(&claimed[&ticket.connection_id.unwrap()]), [slot.id]);
        assert!(claimed_again.is_empty());
        let entry = entry(&service, &slot).await.unwrap();
        assert_eq!((entry.published_at, entry.error), (None, None));
    }

    #[tokio::test]
    async fn claim_resets_failed_and_stuck_entries_only_on_retry() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "Test Jira", "KAR-1").await;
        let failed = add_slot(&service, &ticket, 1000, Some(2000)).await;
        let stuck = add_slot(&service, &ticket, 2000, Some(3000)).await;
        let pending = add_slot(&service, &ticket, 3000, Some(4000)).await;
        let now = Utc::now().timestamp();
        add_entry(&service, &failed, &ticket, None, Some("error"), 0).await;
        add_entry(&service, &stuck, &ticket, None, None, now - STUCK_AFTER - 1).await;
        add_entry(&service, &pending, &ticket, None, None, now).await;
        let slots = || {
            let slots = [&failed, &stuck, &pending].map(|model| Slot {
                model: model.clone(),
                ticket: ticket.clone(),
            });
            HashMap::from([(ticket.connection_id.unwrap(), slots.into())])
        };

        assert!(claim(&service.db, slots(), false).await.unwrap().is_empty());
        let claimed = claim(&service.db, slots(), true).await.unwrap();

        assert_eq!(
            ids(&claimed[&ticket.connection_id.unwrap()]),
            [failed.id, stuck.id]
        );
        for slot in [&failed, &stuck] {
            let entry = entry(&service, slot).await.unwrap();
            assert_eq!(entry.error, None);
            assert!(entry.claimed_at >= now);
        }
    }

    #[tokio::test]
    async fn store_result_marks_entry_ok_or_failed() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "Test Jira", "KAR-1").await;
        let slot = add_slot(&service, &ticket, 1000, Some(2000)).await;
        let connection_id = ticket.connection_id.unwrap();
        add_entry(&service, &slot, &ticket, None, None, 0).await;

        let error = "network error".to_string();
        store_result(&service.db, slot.id, connection_id, Some(&error))
            .await
            .unwrap();
        let failed = entry(&service, &slot).await.unwrap();
        assert_eq!((failed.published_at, failed.error), (None, Some(error)));

        store_result(&service.db, slot.id, connection_id, None)
            .await
            .unwrap();
        let ok = entry(&service, &slot).await.unwrap();
        assert!(ok.published_at.is_some());
        assert_eq!(ok.error, None);
    }

    #[tokio::test]
    async fn store_result_fails_without_entry() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "Test Jira", "KAR-1").await;
        let slot = add_slot(&service, &ticket, 1000, Some(2000)).await;

        let err = store_result(&service.db, slot.id, ticket.connection_id.unwrap(), None)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::PublishEntryNotFound(id)) if id == slot.id
        ));
    }

    #[tokio::test]
    async fn publish_slots_stores_failure_of_short_jira_worklog() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "Test Jira", "KAR-1").await;
        let slot = add_slot(&service, &ticket, 1000, Some(1059)).await;

        let result = service
            .publish_slots(None, None, None, false)
            .await
            .unwrap();

        assert!(result.published.is_empty());
        assert_eq!(result.failed.len(), 1);
        let error = "worklog of 59 s is under the 60 s Jira accepts";
        assert_eq!(result.failed[0].error, error);
        let entry = entry(&service, &slot).await.unwrap();
        assert_eq!(
            (entry.published_at, entry.error.as_deref()),
            (None, Some(error))
        );
        let again = service
            .publish_slots(None, None, None, false)
            .await
            .unwrap();
        assert!(again.published.is_empty() && again.failed.is_empty());
    }

    #[tokio::test]
    async fn publish_slots_skips_connections_with_publish_disabled() {
        let service = test_service().await;
        add_linear(&service, false).await;
        let ticket = add_ticket(&service, "Test Linear", "NIR-1").await;
        let slot = add_slot(&service, &ticket, 1000, Some(2000)).await;

        let result = service
            .publish_slots(None, None, None, false)
            .await
            .unwrap();

        assert!(result.published.is_empty() && result.failed.is_empty());
        assert!(entry(&service, &slot).await.is_none());
    }

    #[tokio::test]
    async fn publish_slots_without_connections_publishes_nothing() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "Test Jira", "KAR-1").await;
        add_slot(&service, &ticket, 1000, Some(2000)).await;
        service.delete_connection("Test Jira", false).await.unwrap();

        let result = service
            .publish_slots(None, None, None, false)
            .await
            .unwrap();

        assert!(result.published.is_empty() && result.failed.is_empty());
        assert_eq!(
            publish_entry::Entity::find()
                .count(&service.db)
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn publish_slots_rejects_target_with_publish_disabled() {
        let service = test_service().await;
        add_linear(&service, false).await;

        let err = service
            .publish_slots(None, None, Some("Test Linear"), false)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::PublishDisabled(name)) if name == "Test Linear"
        ));
    }

    #[tokio::test]
    async fn publish_slots_limits_to_target() {
        let service = test_service().await;
        add_linear(&service, true).await;
        let jira = add_ticket(&service, "Test Jira", "KAR-1").await;
        let jira_slot = add_slot(&service, &jira, 1000, Some(1030)).await;
        let linear = add_ticket(&service, "Test Linear", "NIR-1").await;
        let linear_slot = add_slot(&service, &linear, 2000, Some(3000)).await;

        let result = service
            .publish_slots(None, None, Some("Test Jira"), false)
            .await
            .unwrap();

        assert_eq!(result.failed.len(), 1);
        assert_eq!(result.failed[0].slot.model.id, jira_slot.id);
        assert!(entry(&service, &linear_slot).await.is_none());
    }

    #[test]
    fn error_chain_joins_sources() {
        let error = NirvanaError::from(DomainError::PublishDisabled("jira".to_string()));
        assert_eq!(
            error_chain(&error),
            "publishing is disabled for connection 'jira'"
        );

        let io = std::io::Error::other("disk full");
        let error = NirvanaError::from(io);
        assert_eq!(error_chain(&error), "I/O error: disk full");
    }
}
