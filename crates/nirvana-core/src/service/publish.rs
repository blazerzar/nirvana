//! Publishing.
//!
//! A stopped slot is published to its ticket's connection when that
//! connection has `publish` enabled: Jira as worklogs, Linear as comments. A
//! publish first claims its slots with pending entries in one transaction and
//! sends only the claimed ones, so concurrent publishes do not send a slot
//! twice. The result of each slot is stored in its entry, as ok or as failed
//! with the error. A pending entry claimed more than [`STUCK_AFTER`] seconds
//! ago was left by an interrupted publish. With `squash`, the claimed slots of
//! one ticket and local day are sent as one worklog or comment.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Local, NaiveDate, Utc};
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
            let groups = match connection.settings.squash() {
                true => squash(slots),
                false => slots.into_iter().map(|slot| vec![slot]).collect(),
            };
            publishes.spawn(publish_to_connection(
                self.db.clone(),
                connection.model.id,
                integration,
                groups,
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

/// Sends each group of slots of one ticket to the connection as one worklog
/// or comment, starting at the group's earliest slot start and lasting the
/// summed duration, and stores the result on every slot of the group. When
/// `integration` could not be built, its error is stored for every slot.
async fn publish_to_connection(
    db: DatabaseConnection,
    connection_id: i64,
    integration: Result<Integration, String>,
    groups: Vec<Vec<Slot>>,
) -> PublishResult {
    let mut result = PublishResult::default();
    for group in groups {
        let first = &group[0];
        let seconds = group.iter().map(Slot::elapsed).sum();
        let sent = match &integration {
            Ok(integration) => integration
                .publish_slot(&first.ticket.ticket_key, first.model.started_at, seconds)
                .await
                .map_err(|e| error_chain(&e)),
            Err(error) => Err(error.clone()),
        };
        for slot in group {
            let stored = store_result(&db, slot.model.id, connection_id, sent.as_ref().err()).await;
            let error = match (&sent, stored) {
                (Ok(()), Ok(())) => {
                    result.published.push(slot);
                    continue;
                }
                (Ok(()), Err(e)) => format!(
                    "published, but storing the result failed: {}",
                    error_chain(&e)
                ),
                (Err(error), Ok(())) => error.clone(),
                (Err(error), Err(e)) => {
                    format!("{error}, and storing the error failed: {}", error_chain(&e))
                }
            };
            debug!(slot_id = slot.model.id, %error, "slot not published");
            result.failed.push(PublishFailure { slot, error });
        }
    }
    result
}

/// Groups the slots by ticket and the local day they started on, ordered by
/// their first start.
fn squash(slots: Vec<Slot>) -> Vec<Vec<Slot>> {
    let mut groups: HashMap<(i64, Option<NaiveDate>), Vec<Slot>> = HashMap::new();
    for slot in slots {
        let day = DateTime::from_timestamp(slot.model.started_at, 0)
            .map(|started_at| started_at.with_timezone(&Local).date_naive());
        groups.entry((slot.ticket.id, day)).or_default().push(slot);
    }
    let mut groups: Vec<Vec<Slot>> = groups.into_values().collect();
    groups.sort_by_key(|group| group[0].model.started_at);
    groups
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

pub(super) fn stuck_before() -> i64 {
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
    use chrono::TimeZone;
    use sea_orm::PaginatorTrait;

    use super::*;
    use crate::service::test_service::TestConnection::{Jira, KeyringJira, Linear};
    use crate::service::test_service::TestService;

    async fn entry(service: &Nirvana, slot: &slot::Model) -> Option<publish_entry::Model> {
        publish_entry::Entity::find()
            .filter(publish_entry::COLUMN.slot_id.eq(slot.id))
            .one(&service.db)
            .await
            .unwrap()
    }

    fn local(day: u32, hour: u32) -> i64 {
        Local
            .with_ymd_and_hms(2026, 11, day, hour, 0, 0)
            .unwrap()
            .timestamp()
    }

    fn ids(slots: &[Slot]) -> Vec<i64> {
        slots.iter().map(|slot| slot.model.id).collect()
    }

    fn failures(result: &PublishResult) -> Vec<(i64, &str)> {
        result
            .failed
            .iter()
            .map(|failure| (failure.slot.model.id, failure.error.as_str()))
            .collect()
    }

    async fn assert_published(service: &Nirvana, slots: &[&slot::Model]) {
        for slot in slots {
            let entry = entry(service, slot).await.unwrap();
            assert!(entry.published_at.is_some());
            assert_eq!(entry.error, None);
        }
    }

    #[tokio::test]
    async fn find_publishable_selects_stopped_unpublished_slots_in_range() {
        let mut service = TestService::new(&[Jira]).await;
        service.use_connection(Some(Jira));
        let ticket = service.seed_ticket("KAR-1", "Title", 0, 0).await;
        let connection_id = ticket.connection_id.unwrap();
        let before = service.seed_slot(&ticket, 1000, Some(2000)).await;
        let overlapping = service.seed_slot(&ticket, 2500, Some(3500)).await;
        let published = service.seed_slot(&ticket, 3500, Some(4000)).await;
        let failed = service.seed_slot(&ticket, 4000, Some(4500)).await;
        let stuck = service.seed_slot(&ticket, 4500, Some(4600)).await;
        let pending = service.seed_slot(&ticket, 4600, Some(4700)).await;
        service.seed_slot(&ticket, 5000, None).await;
        let now = Utc::now().timestamp();
        service
            .seed_publish_entry(&published, connection_id, Some(4000), None, 4000)
            .await;
        service
            .seed_publish_entry(&failed, connection_id, None, Some("error"), now)
            .await;
        service
            .seed_publish_entry(&stuck, connection_id, None, None, now - STUCK_AFTER - 1)
            .await;
        service
            .seed_publish_entry(&pending, connection_id, None, None, now)
            .await;

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
        let mut service = TestService::new(&[Jira, Linear]).await;
        service.use_connection(Some(Linear));
        let ticket = service.seed_ticket("NIR-1", "Title", 0, 0).await;
        service.seed_slot(&ticket, 1000, Some(2000)).await;
        let jira = service.connection(Jira).await;

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
        let mut service = TestService::new(&[Jira]).await;
        service.use_connection(Some(Jira));
        let ticket = service.seed_ticket("KAR-1", "Title", 0, 0).await;
        let slot = service.seed_slot(&ticket, 1000, Some(2000)).await;
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
        let mut service = TestService::new(&[Jira]).await;
        service.use_connection(Some(Jira));
        let ticket = service.seed_ticket("KAR-1", "Title", 0, 0).await;
        let connection_id = ticket.connection_id.unwrap();
        let failed = service.seed_slot(&ticket, 1000, Some(2000)).await;
        let stuck = service.seed_slot(&ticket, 2000, Some(3000)).await;
        let pending = service.seed_slot(&ticket, 3000, Some(4000)).await;
        let now = Utc::now().timestamp();
        service
            .seed_publish_entry(&failed, connection_id, None, Some("error"), 0)
            .await;
        service
            .seed_publish_entry(&stuck, connection_id, None, None, now - STUCK_AFTER - 1)
            .await;
        service
            .seed_publish_entry(&pending, connection_id, None, None, now)
            .await;
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
        let mut service = TestService::new(&[Jira]).await;
        service.use_connection(Some(Jira));
        let ticket = service.seed_ticket("KAR-1", "Title", 0, 0).await;
        let connection_id = ticket.connection_id.unwrap();
        let slot = service.seed_slot(&ticket, 1000, Some(2000)).await;
        service
            .seed_publish_entry(&slot, connection_id, None, None, 0)
            .await;

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
        let mut service = TestService::new(&[Jira]).await;
        service.use_connection(Some(Jira));
        let ticket = service.seed_ticket("KAR-1", "Title", 0, 0).await;
        let slot = service.seed_slot(&ticket, 1000, Some(2000)).await;

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
        let mut service = TestService::new(&[Jira]).await;
        service.use_connection(Some(Jira));
        let ticket = service.seed_ticket("KAR-1", "Title", 0, 0).await;
        let slot = service.seed_slot(&ticket, 1000, Some(1059)).await;

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
    async fn publish_slots_publishes_to_jira_and_linear() {
        let mut service = TestService::new(&[Jira, Linear]).await;
        service
            .edit_settings(Linear, Some(true), None)
            .await
            .unwrap();
        service.start_mock().await;
        service.use_connection(Some(Jira));
        let jira = service.seed_ticket("KAR-1", "Title", 0, 0).await;
        let jira_slot = service.seed_slot(&jira, 1000, Some(4600)).await;
        service.mock_worklog("KAR-1", 3600).await;
        service.use_connection(Some(Linear));
        let linear = service.seed_ticket("NIR-1", "Title", 0, 0).await;
        let linear_slot = service.seed_slot(&linear, 5000, Some(5600)).await;
        service.mock_comment("NIR-1", "10m").await;

        let result = service
            .publish_slots(None, None, None, false)
            .await
            .unwrap();

        assert!(result.failed.is_empty());
        assert_eq!(ids(&result.published), [jira_slot.id, linear_slot.id]);
        assert_published(&service, &[&jira_slot, &linear_slot]).await;
    }

    #[tokio::test]
    async fn publish_slots_stores_failure_of_missing_ticket() {
        let mut service = TestService::new(&[Jira]).await;
        service.start_mock().await;
        service.use_connection(Some(Jira));
        let ticket = service.seed_ticket("KAR-1", "Title", 0, 0).await;
        let slot = service.seed_slot(&ticket, 1000, Some(4600)).await;

        let result = service
            .publish_slots(None, None, None, false)
            .await
            .unwrap();

        assert!(result.published.is_empty());
        assert_eq!(result.failed.len(), 1);
        let error = "ticket not found: KAR-1";
        assert_eq!(result.failed[0].error, error);
        let entry = entry(&service, &slot).await.unwrap();
        assert_eq!(
            (entry.published_at, entry.error.as_deref()),
            (None, Some(error))
        );
    }

    #[tokio::test]
    async fn publish_slots_stores_result_of_each_squashed_group() {
        let mut service = TestService::new(&[Jira]).await;
        service.edit_settings(Jira, None, Some(true)).await.unwrap();
        service.start_mock().await;
        service.use_connection(Some(Jira));
        let found = service.seed_ticket("KAR-1", "Title", 0, 0).await;
        let missing = service.seed_ticket("KAR-2", "Title", 0, 0).await;
        let found_first = service
            .seed_slot(&found, local(10, 9), Some(local(10, 9) + 600))
            .await;
        let found_second = service
            .seed_slot(&found, local(10, 10), Some(local(10, 10) + 600))
            .await;
        let missing_first = service
            .seed_slot(&missing, local(10, 11), Some(local(10, 11) + 600))
            .await;
        let missing_second = service
            .seed_slot(&missing, local(10, 12), Some(local(10, 12) + 600))
            .await;
        service.mock_worklog("KAR-1", 1200).await;

        let result = service
            .publish_slots(None, None, None, false)
            .await
            .unwrap();

        assert_eq!(ids(&result.published), [found_first.id, found_second.id]);
        let error = "ticket not found: KAR-2";
        assert_eq!(
            failures(&result),
            [(missing_first.id, error), (missing_second.id, error)]
        );
        assert_published(&service, &[&found_first, &found_second]).await;
        for slot in [&missing_first, &missing_second] {
            let entry = entry(&service, slot).await.unwrap();
            assert_eq!(
                (entry.published_at, entry.error.as_deref()),
                (None, Some(error))
            );
        }
    }

    #[tokio::test]
    async fn publish_slots_retry_publishes_failed_slot() {
        let mut service = TestService::new(&[Jira]).await;
        service.start_mock().await;
        service.use_connection(Some(Jira));
        let ticket = service.seed_ticket("KAR-1", "Title", 0, 0).await;
        let slot = service.seed_slot(&ticket, 1000, Some(4600)).await;
        let failed = service
            .publish_slots(None, None, None, false)
            .await
            .unwrap();
        assert_eq!(failed.failed.len(), 1);
        service.mock_worklog("KAR-1", 3600).await;

        let result = service.publish_slots(None, None, None, true).await.unwrap();

        assert!(result.failed.is_empty());
        assert_eq!(ids(&result.published), [slot.id]);
        let entry = entry(&service, &slot).await.unwrap();
        assert!(entry.published_at.is_some());
        assert_eq!(entry.error, None);
    }

    #[tokio::test]
    async fn publish_slots_stores_failure_of_missing_credential_on_every_slot() {
        let mut service = TestService::new(&[KeyringJira]).await;
        service.clear_keyring();
        service.use_connection(Some(KeyringJira));
        let ticket = service.seed_ticket("KAR-1", "Title", 0, 0).await;
        let first = service.seed_slot(&ticket, 1000, Some(4600)).await;
        let second = service.seed_slot(&ticket, 5000, Some(8600)).await;

        let result = service
            .publish_slots(None, None, None, false)
            .await
            .unwrap();

        assert!(result.published.is_empty());
        let error = format!(
            "no keyring entry for connection {}",
            ticket.connection_id.unwrap()
        );
        assert_eq!(
            failures(&result),
            [(first.id, error.as_str()), (second.id, error.as_str())]
        );
        for slot in [&first, &second] {
            let entry = entry(&service, slot).await.unwrap();
            assert_eq!(
                (entry.published_at, entry.error.as_deref()),
                (None, Some(error.as_str()))
            );
        }
    }

    #[tokio::test]
    async fn publish_slots_skips_connections_with_publish_disabled() {
        let mut service = TestService::new(&[Jira, Linear]).await;
        service.use_connection(Some(Linear));
        let ticket = service.seed_ticket("NIR-1", "Title", 0, 0).await;
        let slot = service.seed_slot(&ticket, 1000, Some(2000)).await;

        let result = service
            .publish_slots(None, None, None, false)
            .await
            .unwrap();

        assert!(result.published.is_empty() && result.failed.is_empty());
        assert!(entry(&service, &slot).await.is_none());
    }

    #[tokio::test]
    async fn publish_slots_without_connections_publishes_nothing() {
        let mut service = TestService::new(&[Jira]).await;
        service.use_connection(Some(Jira));
        let ticket = service.seed_ticket("KAR-1", "Title", 0, 0).await;
        service.seed_slot(&ticket, 1000, Some(2000)).await;
        service.delete_connection(Jira.name(), false).await.unwrap();

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
        let service = TestService::new(&[Jira, Linear]).await;

        let err = service
            .publish_slots(None, None, Some(Linear.name()), false)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::PublishDisabled(name)) if name == Linear.name()
        ));
    }

    #[tokio::test]
    async fn publish_slots_limits_to_target() {
        let mut service = TestService::new(&[Jira, Linear]).await;
        service
            .edit_settings(Linear, Some(true), None)
            .await
            .unwrap();
        service.use_connection(Some(Jira));
        let jira = service.seed_ticket("KAR-1", "Title", 0, 0).await;
        let jira_slot = service.seed_slot(&jira, 1000, Some(1030)).await;
        service.use_connection(Some(Linear));
        let linear = service.seed_ticket("NIR-1", "Title", 0, 0).await;
        let linear_slot = service.seed_slot(&linear, 2000, Some(3000)).await;

        let result = service
            .publish_slots(None, None, Some(Jira.name()), false)
            .await
            .unwrap();

        assert_eq!(result.failed.len(), 1);
        assert_eq!(result.failed[0].slot.model.id, jira_slot.id);
        assert!(entry(&service, &linear_slot).await.is_none());
    }

    #[tokio::test]
    async fn squash_groups_slots_by_ticket_and_local_day() {
        let mut service = TestService::new(&[Jira]).await;
        service.use_connection(Some(Jira));
        let first = service.seed_ticket("KAR-1", "Title", 0, 0).await;
        let second = service.seed_ticket("KAR-2", "Title", 0, 0).await;
        let morning = service
            .seed_slot(&first, local(10, 0), Some(local(10, 1)))
            .await;
        let evening = service
            .seed_slot(&first, local(10, 23), Some(local(11, 1)))
            .await;
        let next_day = service
            .seed_slot(&first, local(11, 9), Some(local(11, 10)))
            .await;
        let other = service
            .seed_slot(&second, local(10, 9), Some(local(10, 10)))
            .await;
        let slots = [
            (&morning, &first),
            (&evening, &first),
            (&next_day, &first),
            (&other, &second),
        ]
        .map(|(model, ticket)| Slot {
            model: model.clone(),
            ticket: ticket.clone(),
        });

        let groups: Vec<Vec<i64>> = squash(slots.into()).iter().map(|g| ids(g)).collect();

        assert_eq!(
            groups,
            [
                vec![morning.id, evening.id],
                vec![other.id],
                vec![next_day.id]
            ]
        );
    }

    #[tokio::test]
    async fn publish_slots_squashes_and_stores_result_on_every_slot() {
        let mut service = TestService::new(&[Jira]).await;
        service.edit_settings(Jira, None, Some(true)).await.unwrap();
        service.start_mock().await;
        service.use_connection(Some(Jira));
        let ticket = service.seed_ticket("KAR-1", "Title", 0, 0).await;
        let first = service
            .seed_slot(&ticket, local(10, 9), Some(local(10, 9) + 1200))
            .await;
        let second = service
            .seed_slot(&ticket, local(10, 10), Some(local(10, 10) + 1200))
            .await;
        let next_day = service
            .seed_slot(&ticket, local(11, 9), Some(local(11, 9) + 1800))
            .await;
        service.mock_worklog("KAR-1", 2400).await;
        service.mock_worklog("KAR-1", 1800).await;

        let result = service
            .publish_slots(None, None, None, false)
            .await
            .unwrap();

        assert!(result.failed.is_empty());
        assert_eq!(ids(&result.published), [first.id, second.id, next_day.id]);
        assert_published(&service, &[&first, &second, &next_day]).await;
    }

    #[tokio::test]
    async fn publish_slots_squash_skips_published_slots_on_retry() {
        let mut service = TestService::new(&[Jira]).await;
        service.edit_settings(Jira, None, Some(true)).await.unwrap();
        service.start_mock().await;
        service.use_connection(Some(Jira));
        let ticket = service.seed_ticket("KAR-1", "Title", 0, 0).await;
        let connection_id = ticket.connection_id.unwrap();
        let published = service
            .seed_slot(&ticket, local(10, 9), Some(local(10, 9) + 3000))
            .await;
        let failed = service
            .seed_slot(&ticket, local(10, 10), Some(local(10, 10) + 1800))
            .await;
        service
            .seed_publish_entry(&published, connection_id, Some(1000), None, 1000)
            .await;
        service
            .seed_publish_entry(&failed, connection_id, None, Some("error"), 1000)
            .await;
        service.mock_worklog("KAR-1", 1800).await;

        let result = service.publish_slots(None, None, None, true).await.unwrap();

        assert!(result.failed.is_empty());
        assert_eq!(ids(&result.published), [failed.id]);
        let entry = entry(&service, &published).await.unwrap();
        assert_eq!((entry.published_at, entry.error), (Some(1000), None));
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
