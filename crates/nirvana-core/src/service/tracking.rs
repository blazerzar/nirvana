//! Tracking.
//!
//! A slot is a span of time tracked on a ticket of the active profile. A
//! profile has at most one running slot, which has no stop time, and its slots
//! do not overlap. Starting a slot stops the running one at the same time.
//! Slot times cannot be in the future.

use chrono::Utc;
use nirvana_entity::{publish_entry, slot, ticket};
use sea_orm::sea_query::{Expr, Order, WindowStatement};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseTransaction, EntityTrait, ExprTrait, QueryFilter,
    QueryOrder, QueryTrait, SelectExt, Set, TransactionTrait,
};

use crate::domain::{Slot, SlotTarget, StartResult};
use crate::errors::{DomainError, NirvanaError};
use crate::service::Nirvana;

const TICKET_STARTED_AT: &str = "ticket_started_at";

impl Nirvana {
    /// Returns the running slot of the active profile with its ticket.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable or
    /// a database query fails.
    pub async fn running_slot(&self) -> Result<Option<Slot>, NirvanaError> {
        let profile = self.find_active_profile().await?;
        find_running_slot(&self.db, profile.id).await
    }

    /// Returns the slots of the active profile that overlap the range from
    /// `from` to `to`, or unbounded, with their tickets, ordered by start.
    /// With `by_ticket` the slots of a ticket are grouped, in the order of
    /// each ticket's first slot.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable or
    /// a database query fails.
    pub async fn list_slots(
        &self,
        from: i64,
        to: Option<i64>,
        by_ticket: bool,
    ) -> Result<Vec<Slot>, NirvanaError> {
        let profile = self.find_active_profile().await?;
        let mut query =
            slot::Entity::find_in_range(profile.id, from, to).find_also_related(ticket::Entity);
        if by_ticket {
            QueryTrait::query(&mut query)
                .expr_window_as(
                    Expr::col((slot::Entity, slot::Column::StartedAt)).min(),
                    WindowStatement::partition_by((slot::Entity, slot::Column::TicketId)),
                    TICKET_STARTED_AT,
                )
                .order_by(TICKET_STARTED_AT, Order::Asc);
        }
        query
            .order_by_asc(slot::COLUMN.started_at)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|(model, ticket)| {
                let ticket = ticket.ok_or(DomainError::TicketIdNotFound(model.ticket_id))?;
                Ok(Slot { model, ticket })
            })
            .collect()
    }

    /// Starts tracking the ticket `key` of the active profile at `at`, or
    /// now, and stops the running slot at the same time. A ticket that is
    /// not stored is loaded from the connections. `connection` selects the
    /// ticket of the connection with that name and limits loading to it.
    ///
    /// # Errors
    ///
    /// Returns an error if `at` is in the future, no profile is active, the
    /// config is unavailable, the connection does not exist, more than one
    /// stored ticket matches, the ticket cannot be loaded, the start is not
    /// after the start of the running slot, a backdated slot overlaps another
    /// slot or a database query fails.
    pub async fn start_slot(
        &self,
        key: &str,
        at: Option<i64>,
        connection: Option<&str>,
    ) -> Result<StartResult, NirvanaError> {
        if let Some(at) = at {
            reject_future_time(at)?;
        }
        let profile = self.find_active_profile().await?;
        let ticket = self
            .find_or_load_ticket(profile.id, key, connection)
            .await?;

        let txn = self.db.begin().await?;
        let running = find_running_slot(&txn, profile.id).await?;
        let result = start_in_txn(&txn, profile.id, running, ticket, at).await?;
        txn.commit().await?;
        Ok(result)
    }

    /// Stops the running slot of the active profile at `at`, or now, and
    /// returns it with its ticket. Returns `None` when nothing is running.
    ///
    /// # Errors
    ///
    /// Returns an error if `at` is in the future, no profile is active, the
    /// config is unavailable, the stop is not after the start of the running
    /// slot or a database query fails.
    pub async fn stop_slot(&self, at: Option<i64>) -> Result<Option<Slot>, NirvanaError> {
        if let Some(at) = at {
            reject_future_time(at)?;
        }
        let profile = self.find_active_profile().await?;
        let txn = self.db.begin().await?;
        let Some(running) = find_running_slot(&txn, profile.id).await? else {
            return Ok(None);
        };
        let stopped_at = at.unwrap_or_else(|| Utc::now().timestamp());
        if stopped_at <= running.model.started_at {
            return Err(match at {
                Some(_) => DomainError::InvalidTimeRange,
                None => DomainError::RunningSlotJustStarted,
            }
            .into());
        }

        let mut model: slot::ActiveModel = running.model.into();
        model.stopped_at = Set(Some(stopped_at));
        let model = model.update(&txn).await?;
        txn.commit().await?;
        Ok(Some(Slot {
            model,
            ticket: running.ticket,
        }))
    }

    /// Starts tracking the ticket of the last stopped slot of the active
    /// profile at `at`, or now, and returns the started slot with its ticket.
    ///
    /// # Errors
    ///
    /// Returns an error if `at` is in the future, no profile is active, the
    /// config is unavailable, a slot is running, there is no stopped slot, a
    /// backdated slot overlaps another slot or a database query fails.
    pub async fn resume_slot(&self, at: Option<i64>) -> Result<Slot, NirvanaError> {
        if let Some(at) = at {
            reject_future_time(at)?;
        }
        let profile = self.find_active_profile().await?;
        let txn = self.db.begin().await?;
        if find_running_slot(&txn, profile.id).await?.is_some() {
            return Err(DomainError::AlreadyTracking.into());
        }
        let ticket = find_previous_ticket(&txn, profile.id, None).await?;

        let result = start_in_txn(&txn, profile.id, None, ticket, at).await?;
        txn.commit().await?;
        Ok(result.started)
    }

    /// Stops the running slot of the active profile at `at`, or now, and
    /// starts tracking the ticket of the last stopped slot on another ticket
    /// at the same time.
    ///
    /// # Errors
    ///
    /// Returns an error if `at` is in the future, no profile is active, the
    /// config is unavailable, no slot is running, there is no stopped slot on
    /// another ticket, the start is not after the start of the running slot
    /// or a database query fails.
    pub async fn switch_slot(&self, at: Option<i64>) -> Result<StartResult, NirvanaError> {
        if let Some(at) = at {
            reject_future_time(at)?;
        }
        let profile = self.find_active_profile().await?;
        let txn = self.db.begin().await?;
        let Some(running) = find_running_slot(&txn, profile.id).await? else {
            return Err(DomainError::NotTracking.into());
        };
        let ticket = find_previous_ticket(&txn, profile.id, Some(running.model.ticket_id)).await?;

        let result = start_in_txn(&txn, profile.id, Some(running), ticket, at).await?;
        txn.commit().await?;
        Ok(result)
    }

    /// Adds a slot on the ticket `key` of the active profile from
    /// `started_at` to `stopped_at`. The ticket is resolved like in
    /// [`Nirvana::start_slot`]. The ticket's last worked time only moves
    /// forward.
    ///
    /// # Errors
    ///
    /// Returns an error if `stopped_at` is in the future or not after
    /// `started_at`, no profile is active, the config is unavailable, the
    /// connection does not exist, more than one stored ticket matches, the
    /// ticket cannot be loaded, the slot overlaps another slot or a database
    /// query fails.
    pub async fn add_slot(
        &self,
        key: &str,
        started_at: i64,
        stopped_at: i64,
        connection: Option<&str>,
    ) -> Result<Slot, NirvanaError> {
        reject_future_time(stopped_at)?;
        if stopped_at <= started_at {
            return Err(DomainError::InvalidTimeRange.into());
        }
        let profile = self.find_active_profile().await?;
        let ticket = self
            .find_or_load_ticket(profile.id, key, connection)
            .await?;

        let txn = self.db.begin().await?;
        if slot::Entity::find_in_range(profile.id, started_at, Some(stopped_at))
            .exists(&txn)
            .await?
        {
            return Err(DomainError::SlotOverlap.into());
        }
        let model = slot::ActiveModel {
            ticket_id: Set(ticket.id),
            profile_id: Set(profile.id),
            started_at: Set(started_at),
            stopped_at: Set(Some(stopped_at)),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
        let ticket = mark_worked(&txn, ticket, started_at).await?;
        txn.commit().await?;
        Ok(Slot { model, ticket })
    }

    /// Returns the slot of the active profile with the ID, or tracked at the
    /// time, of `target`, with its ticket and whether it is published to any
    /// connection.
    ///
    /// # Errors
    ///
    /// Returns an error if the time is in the future, no profile is active,
    /// the config is unavailable, no slot matches or a database query fails.
    pub async fn get_slot(&self, target: SlotTarget) -> Result<(Slot, bool), NirvanaError> {
        if let SlotTarget::At(at) = target {
            reject_future_time(at)?;
        }
        let profile = self.find_active_profile().await?;
        let query = match target {
            SlotTarget::Id(id) => {
                slot::Entity::find_by_id(id).filter(slot::COLUMN.profile_id.eq(profile.id))
            }
            SlotTarget::At(at) => slot::Entity::find_at(profile.id, at),
        };
        let Some((model, ticket)) = query
            .find_also_related(ticket::Entity)
            .one(&self.db)
            .await?
        else {
            return Err(match target {
                SlotTarget::Id(id) => DomainError::SlotNotFound(id),
                SlotTarget::At(_) => DomainError::NoSlotAt,
            }
            .into());
        };
        let ticket = ticket.ok_or(DomainError::TicketIdNotFound(model.ticket_id))?;
        let published = publish_entry::Entity::find_published()
            .filter(publish_entry::COLUMN.slot_id.eq(model.id))
            .exists(&self.db)
            .await?;
        Ok((Slot { model, ticket }, published))
    }

    /// Deletes the slot `id` of the active profile with its publish entries.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable,
    /// the slot does not exist or a database query fails.
    pub async fn delete_slot(&self, id: i64) -> Result<(), NirvanaError> {
        let profile = self.find_active_profile().await?;
        let result = slot::Entity::delete_many()
            .filter(slot::COLUMN.id.eq(id))
            .filter(slot::COLUMN.profile_id.eq(profile.id))
            .exec(&self.db)
            .await?;
        if result.rows_affected == 0 {
            return Err(DomainError::SlotNotFound(id).into());
        }
        Ok(())
    }
}

async fn find_running_slot(
    db: &impl ConnectionTrait,
    profile_id: i64,
) -> Result<Option<Slot>, NirvanaError> {
    let Some((model, ticket)) = slot::Entity::find_running(profile_id)
        .find_also_related(ticket::Entity)
        .one(db)
        .await?
    else {
        return Ok(None);
    };
    let ticket = ticket.ok_or(DomainError::TicketIdNotFound(model.ticket_id))?;
    Ok(Some(Slot { model, ticket }))
}

/// Returns the ticket of the last stopped slot of the profile, skipping the
/// slots of `skipped_ticket_id`.
async fn find_previous_ticket(
    db: &impl ConnectionTrait,
    profile_id: i64,
    skipped_ticket_id: Option<i64>,
) -> Result<ticket::Model, NirvanaError> {
    let Some((slot, ticket)) = slot::Entity::find_last_stopped(profile_id)
        .apply_if(skipped_ticket_id, |query, id| {
            query.filter(slot::COLUMN.ticket_id.ne(id))
        })
        .find_also_related(ticket::Entity)
        .one(db)
        .await?
    else {
        return Err(DomainError::NoPreviousTicket.into());
    };
    Ok(ticket.ok_or(DomainError::TicketIdNotFound(slot.ticket_id))?)
}

/// Stops the `running` slot of the profile and starts a slot on `ticket` at
/// `at`, or now. A backdated slot must not overlap other slots.
async fn start_in_txn(
    txn: &DatabaseTransaction,
    profile_id: i64,
    running: Option<Slot>,
    ticket: ticket::Model,
    at: Option<i64>,
) -> Result<StartResult, NirvanaError> {
    let now = Utc::now().timestamp();
    let started_at = at.unwrap_or(now);

    let stopped = match running {
        Some(running) => {
            if started_at <= running.model.started_at {
                return Err(match at {
                    Some(_) => DomainError::StartBeforeRunningSlot,
                    None => DomainError::RunningSlotJustStarted,
                }
                .into());
            }
            let mut model: slot::ActiveModel = running.model.into();
            model.stopped_at = Set(Some(started_at));
            Some(Slot {
                model: model.update(txn).await?,
                ticket: running.ticket,
            })
        }
        None => None,
    };

    if at.is_some()
        && slot::Entity::find_in_range(profile_id, started_at, None)
            .exists(txn)
            .await?
    {
        return Err(DomainError::SlotOverlap.into());
    }

    let slot = slot::ActiveModel {
        ticket_id: Set(ticket.id),
        profile_id: Set(profile_id),
        started_at: Set(started_at),
        ..Default::default()
    }
    .insert(txn)
    .await?;

    let ticket = mark_worked(txn, ticket, started_at).await?;

    Ok(StartResult {
        started: Slot {
            model: slot,
            ticket,
        },
        stopped,
    })
}

/// Moves the last worked time of `ticket` forward to `worked_at`.
async fn mark_worked(
    txn: &DatabaseTransaction,
    ticket: ticket::Model,
    worked_at: i64,
) -> Result<ticket::Model, NirvanaError> {
    if worked_at <= ticket.last_worked_at {
        return Ok(ticket);
    }
    let mut ticket: ticket::ActiveModel = ticket.into();
    ticket.last_worked_at = Set(worked_at);
    Ok(ticket.update(txn).await?)
}

fn reject_future_time(timestamp: i64) -> Result<(), DomainError> {
    if timestamp > Utc::now().timestamp() {
        return Err(DomainError::FutureTime);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use sea_orm::PaginatorTrait;

    use super::*;
    use crate::service::test_support::test_service;

    #[test]
    fn reject_future_time_accepts_past_and_present() {
        let now = Utc::now().timestamp();
        assert!(reject_future_time(now - 60).is_ok());
        assert!(reject_future_time(now).is_ok());
    }

    #[test]
    fn reject_future_time_rejects_future() {
        let now = Utc::now().timestamp();
        assert!(matches!(
            reject_future_time(now + 60),
            Err(DomainError::FutureTime)
        ));
    }

    /// Adds the ticket `key` to the active profile, in "Test Jira" when
    /// `connected` is set.
    async fn add_ticket(service: &Nirvana, key: &str, connected: bool) -> ticket::Model {
        let connection_id = match connected {
            true => Some(service.list_connections().await.unwrap()[0].model.id),
            false => None,
        };
        ticket::ActiveModel {
            profile_id: Set(service.find_active_profile().await.unwrap().id),
            connection_id: Set(connection_id),
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

    async fn add_publish_entry(
        service: &Nirvana,
        ticket: &ticket::Model,
        slot: &slot::Model,
        published_at: Option<i64>,
    ) -> publish_entry::Model {
        publish_entry::ActiveModel {
            slot_id: Set(slot.id),
            connection_id: Set(ticket.connection_id.unwrap()),
            published_at: Set(published_at),
            error: Set(None),
        }
        .insert(&service.db)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn start_slot_stops_running_slot() {
        let service = test_service().await;
        let now = Utc::now().timestamp();
        let previous = add_ticket(&service, "JIRA-1", true).await;
        let running = add_slot(&service, &previous, now - 600, None).await;
        add_ticket(&service, "JIRA-2", true).await;

        let result = service.start_slot("JIRA-2", None, None).await.unwrap();

        let Slot {
            model: stopped,
            ticket: stopped_ticket,
        } = result.stopped.unwrap();
        let started = result.started.model;
        assert_eq!(stopped.id, running.id);
        assert_eq!(stopped_ticket.id, previous.id);
        assert_eq!(stopped.stopped_at, Some(started.started_at));
        assert_eq!(started.stopped_at, None);
        let running = slot::Entity::find_running(previous.profile_id)
            .all(&service.db)
            .await
            .unwrap();
        assert_eq!(running, [started]);
    }

    #[tokio::test]
    async fn start_slot_uses_stored_ticket() {
        let service = test_service().await;
        let stored = add_ticket(&service, "JIRA-1", true).await;

        let result = service.start_slot("JIRA-1", None, None).await.unwrap();

        let Slot {
            model: slot,
            ticket,
        } = result.started;
        assert_eq!(slot.ticket_id, stored.id);
        assert_eq!(ticket.id, stored.id);
        assert!(result.stopped.is_none());
        assert_eq!(ticket::Entity::find().count(&service.db).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn start_slot_rejects_ambiguous_key_without_connection() {
        let service = test_service().await;
        let connected = add_ticket(&service, "JIRA-1", true).await;
        add_ticket(&service, "JIRA-1", false).await;

        let err = service.start_slot("JIRA-1", None, None).await.unwrap_err();
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::AmbiguousTicket(key, _)) if key == "JIRA-1"
        ));

        let result = service
            .start_slot("JIRA-1", None, Some("Test Jira"))
            .await
            .unwrap();
        assert_eq!(result.started.ticket.id, connected.id);
    }

    #[tokio::test]
    async fn start_slot_rejects_future_time() {
        let service = test_service().await;
        add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();

        let err = service
            .start_slot("JIRA-1", Some(now + 60), None)
            .await
            .unwrap_err();

        assert!(matches!(err, NirvanaError::Domain(DomainError::FutureTime)));
    }

    #[tokio::test]
    async fn start_slot_rejects_backdated_overlap_with_closed_slot() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        add_slot(&service, &ticket, now - 3600, Some(now - 1800)).await;

        let err = service
            .start_slot("JIRA-1", Some(now - 2700), None)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::SlotOverlap)
        ));
        assert_eq!(slot::Entity::find().count(&service.db).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn start_slot_rejects_time_before_running_slot_start() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let running = add_slot(&service, &ticket, now - 600, None).await;

        for at in [now - 1200, now - 600] {
            let err = service
                .start_slot("JIRA-1", Some(at), None)
                .await
                .unwrap_err();
            assert!(matches!(
                err,
                NirvanaError::Domain(DomainError::StartBeforeRunningSlot)
            ));
        }
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [running]);
    }

    #[tokio::test]
    async fn start_slot_rejects_running_slot_started_in_same_second() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let running = add_slot(&service, &ticket, Utc::now().timestamp() + 60, None).await;

        let err = service.start_slot("JIRA-1", None, None).await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::RunningSlotJustStarted)
        ));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [running]);
    }

    #[tokio::test]
    async fn start_slot_backdated_stops_running_slot_at_start() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        add_slot(&service, &ticket, now - 3600, Some(now - 1800)).await;
        let running = add_slot(&service, &ticket, now - 1200, None).await;

        let result = service
            .start_slot("JIRA-1", Some(now - 600), None)
            .await
            .unwrap();

        let stopped = result.stopped.unwrap().model;
        assert_eq!(stopped.id, running.id);
        assert_eq!(stopped.stopped_at, Some(now - 600));
        assert_eq!(result.started.model.started_at, now - 600);
        assert_eq!(result.started.model.stopped_at, None);
    }

    #[tokio::test]
    async fn stop_slot_stops_running_slot() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let running = add_slot(&service, &ticket, now - 600, None).await;

        let stopped = service.stop_slot(Some(now - 60)).await.unwrap().unwrap();

        assert_eq!(stopped.model.id, running.id);
        assert_eq!(stopped.model.stopped_at, Some(now - 60));
        assert_eq!(stopped.ticket, ticket);
        assert!(service.running_slot().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn stop_slot_stops_running_slot_now() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let running = add_slot(&service, &ticket, Utc::now().timestamp() - 600, None).await;

        let before = Utc::now().timestamp();
        let stopped = service.stop_slot(None).await.unwrap().unwrap();
        let after = Utc::now().timestamp();

        assert_eq!(stopped.model.id, running.id);
        let stopped_at = stopped.model.stopped_at.unwrap();
        assert!((before..=after).contains(&stopped_at));
    }

    #[tokio::test]
    async fn stop_slot_rejects_running_slot_started_in_same_second() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let running = add_slot(&service, &ticket, Utc::now().timestamp() + 60, None).await;

        let err = service.stop_slot(None).await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::RunningSlotJustStarted)
        ));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [running]);
    }

    #[tokio::test]
    async fn stop_slot_returns_none_when_not_tracking() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        add_slot(&service, &ticket, now - 600, Some(now - 300)).await;

        assert!(service.stop_slot(None).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn stop_slot_rejects_future_time() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let running = add_slot(&service, &ticket, now - 600, None).await;

        let err = service.stop_slot(Some(now + 60)).await.unwrap_err();

        assert!(matches!(err, NirvanaError::Domain(DomainError::FutureTime)));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [running]);
    }

    #[tokio::test]
    async fn stop_slot_rejects_time_not_after_start() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let running = add_slot(&service, &ticket, now - 600, None).await;

        for at in [now - 1200, now - 600] {
            let err = service.stop_slot(Some(at)).await.unwrap_err();
            assert!(matches!(
                err,
                NirvanaError::Domain(DomainError::InvalidTimeRange)
            ));
        }
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [running]);
    }

    #[tokio::test]
    async fn resume_slot_starts_ticket_of_last_stopped_slot() {
        let service = test_service().await;
        let first = add_ticket(&service, "JIRA-1", true).await;
        let last = add_ticket(&service, "JIRA-2", true).await;
        let now = Utc::now().timestamp();
        add_slot(&service, &last, now - 3600, Some(now - 3000)).await;
        add_slot(&service, &first, now - 2400, Some(now - 1800)).await;
        add_slot(&service, &last, now - 1200, Some(now - 600)).await;

        let resumed = service.resume_slot(None).await.unwrap();

        assert_eq!(resumed.ticket.id, last.id);
        assert_eq!(resumed.model.stopped_at, None);
        let running = service.running_slot().await.unwrap().unwrap();
        assert_eq!(running.model, resumed.model);
    }

    #[tokio::test]
    async fn resume_slot_starts_at_time() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        add_slot(&service, &ticket, now - 1200, Some(now - 600)).await;

        let resumed = service.resume_slot(Some(now - 300)).await.unwrap();

        assert_eq!(resumed.ticket.id, ticket.id);
        assert_eq!(resumed.model.started_at, now - 300);
        assert_eq!(resumed.model.stopped_at, None);
    }

    #[tokio::test]
    async fn resume_slot_rejects_future_time() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let stopped = add_slot(&service, &ticket, now - 1200, Some(now - 600)).await;

        let err = service.resume_slot(Some(now + 60)).await.unwrap_err();

        assert!(matches!(err, NirvanaError::Domain(DomainError::FutureTime)));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [stopped]);
    }

    #[tokio::test]
    async fn resume_slot_rejects_overlap_with_last_slot() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let stopped = add_slot(&service, &ticket, now - 1200, Some(now - 600)).await;

        let err = service.resume_slot(Some(now - 900)).await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::SlotOverlap)
        ));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [stopped]);
    }

    #[tokio::test]
    async fn resume_slot_rejects_while_tracking() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let stopped = add_slot(&service, &ticket, now - 1200, Some(now - 600)).await;
        let running = add_slot(&service, &ticket, now - 300, None).await;

        let err = service.resume_slot(None).await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::AlreadyTracking)
        ));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [stopped, running]);
    }

    #[tokio::test]
    async fn resume_slot_rejects_without_previous_slot() {
        let service = test_service().await;
        add_ticket(&service, "JIRA-1", true).await;

        let err = service.resume_slot(None).await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::NoPreviousTicket)
        ));
        assert_eq!(slot::Entity::find().count(&service.db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn resume_slot_ignores_slots_of_other_profiles() {
        let mut service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let stopped = add_slot(&service, &ticket, now - 1200, Some(now - 600)).await;
        service.create_profile("work".to_string()).await.unwrap();
        service.switch_profile("work").await.unwrap();

        let err = service.resume_slot(None).await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::NoPreviousTicket)
        ));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [stopped]);
    }

    #[tokio::test]
    async fn switch_slot_switches_to_previous_ticket() {
        let service = test_service().await;
        let previous = add_ticket(&service, "JIRA-1", true).await;
        let current = add_ticket(&service, "JIRA-2", true).await;
        let now = Utc::now().timestamp();
        add_slot(&service, &previous, now - 1200, Some(now - 600)).await;
        let running = add_slot(&service, &current, now - 600, None).await;

        let result = service.switch_slot(None).await.unwrap();

        let stopped = result.stopped.unwrap();
        let started = result.started;
        assert_eq!(stopped.model.id, running.id);
        assert_eq!(stopped.ticket.id, current.id);
        assert_eq!(stopped.model.stopped_at, Some(started.model.started_at));
        assert_eq!(started.ticket.id, previous.id);
        assert_eq!(started.model.stopped_at, None);
        let running = service.running_slot().await.unwrap().unwrap();
        assert_eq!(running.model, started.model);
    }

    #[tokio::test]
    async fn switch_slot_switches_at_time() {
        let service = test_service().await;
        let previous = add_ticket(&service, "JIRA-1", true).await;
        let current = add_ticket(&service, "JIRA-2", true).await;
        let now = Utc::now().timestamp();
        add_slot(&service, &previous, now - 1200, Some(now - 600)).await;
        let running = add_slot(&service, &current, now - 600, None).await;

        let result = service.switch_slot(Some(now - 300)).await.unwrap();

        let stopped = result.stopped.unwrap().model;
        assert_eq!(stopped.id, running.id);
        assert_eq!(stopped.stopped_at, Some(now - 300));
        assert_eq!(result.started.ticket.id, previous.id);
        assert_eq!(result.started.model.started_at, now - 300);
        assert_eq!(result.started.model.stopped_at, None);
    }

    #[tokio::test]
    async fn switch_slot_rejects_future_time() {
        let service = test_service().await;
        let previous = add_ticket(&service, "JIRA-1", true).await;
        let current = add_ticket(&service, "JIRA-2", true).await;
        let now = Utc::now().timestamp();
        let stopped = add_slot(&service, &previous, now - 1200, Some(now - 600)).await;
        let running = add_slot(&service, &current, now - 600, None).await;

        let err = service.switch_slot(Some(now + 60)).await.unwrap_err();

        assert!(matches!(err, NirvanaError::Domain(DomainError::FutureTime)));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [stopped, running]);
    }

    #[tokio::test]
    async fn switch_slot_rejects_time_before_running_slot_start() {
        let service = test_service().await;
        let previous = add_ticket(&service, "JIRA-1", true).await;
        let current = add_ticket(&service, "JIRA-2", true).await;
        let now = Utc::now().timestamp();
        let stopped = add_slot(&service, &previous, now - 1200, Some(now - 900)).await;
        let running = add_slot(&service, &current, now - 600, None).await;

        for at in [now - 800, now - 600] {
            let err = service.switch_slot(Some(at)).await.unwrap_err();
            assert!(matches!(
                err,
                NirvanaError::Domain(DomainError::StartBeforeRunningSlot)
            ));
        }
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [stopped, running]);
    }

    #[tokio::test]
    async fn switch_slot_skips_slots_of_running_ticket() {
        let service = test_service().await;
        let previous = add_ticket(&service, "JIRA-1", true).await;
        let current = add_ticket(&service, "JIRA-2", true).await;
        let now = Utc::now().timestamp();
        add_slot(&service, &previous, now - 3600, Some(now - 3000)).await;
        add_slot(&service, &current, now - 2400, Some(now - 1800)).await;
        add_slot(&service, &current, now - 1200, None).await;

        let result = service.switch_slot(None).await.unwrap();

        assert_eq!(result.started.ticket.id, previous.id);
    }

    #[tokio::test]
    async fn switch_slot_rejects_when_not_tracking() {
        let service = test_service().await;
        let first = add_ticket(&service, "JIRA-1", true).await;
        let second = add_ticket(&service, "JIRA-2", true).await;
        let now = Utc::now().timestamp();
        let a = add_slot(&service, &first, now - 1200, Some(now - 900)).await;
        let b = add_slot(&service, &second, now - 600, Some(now - 300)).await;

        let err = service.switch_slot(None).await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::NotTracking)
        ));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [a, b]);
    }

    #[tokio::test]
    async fn switch_slot_rejects_without_previous_ticket() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let stopped = add_slot(&service, &ticket, now - 1200, Some(now - 600)).await;
        let running = add_slot(&service, &ticket, now - 300, None).await;

        let err = service.switch_slot(None).await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::NoPreviousTicket)
        ));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [stopped, running]);
    }

    #[tokio::test]
    async fn switch_slot_rejects_running_slot_started_in_same_second() {
        let service = test_service().await;
        let previous = add_ticket(&service, "JIRA-1", true).await;
        let current = add_ticket(&service, "JIRA-2", true).await;
        let now = Utc::now().timestamp();
        let stopped = add_slot(&service, &previous, now - 1200, Some(now - 600)).await;
        let running = add_slot(&service, &current, now + 60, None).await;

        let err = service.switch_slot(None).await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::RunningSlotJustStarted)
        ));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [stopped, running]);
    }

    #[tokio::test]
    async fn running_slot_returns_slot_with_ticket() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let running = add_slot(&service, &ticket, Utc::now().timestamp() - 300, None).await;

        let slot = service.running_slot().await.unwrap().unwrap();

        assert_eq!(slot.model, running);
        assert_eq!(slot.ticket, ticket);
    }

    async fn listed_slot_ids(
        service: &Nirvana,
        from: i64,
        to: Option<i64>,
        by_ticket: bool,
    ) -> Vec<i64> {
        service
            .list_slots(from, to, by_ticket)
            .await
            .unwrap()
            .into_iter()
            .map(|slot| slot.model.id)
            .collect()
    }

    #[tokio::test]
    async fn list_slots_returns_slots_overlapping_range() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        add_slot(&service, &ticket, now - 7200, Some(now - 5400)).await;
        let crossing_start = add_slot(&service, &ticket, now - 5000, Some(now - 3000)).await;
        let inside = add_slot(&service, &ticket, now - 2500, Some(now - 2000)).await;
        let crossing_stop = add_slot(&service, &ticket, now - 1500, Some(now - 500)).await;
        let running = add_slot(&service, &ticket, now - 300, None).await;

        assert_eq!(
            listed_slot_ids(&service, now - 4000, Some(now - 1000), false).await,
            [crossing_start.id, inside.id, crossing_stop.id]
        );
        assert_eq!(
            listed_slot_ids(&service, now - 4000, None, false).await,
            [crossing_start.id, inside.id, crossing_stop.id, running.id]
        );
        assert_eq!(
            listed_slot_ids(&service, now - 600, Some(now - 100), false).await,
            [crossing_stop.id, running.id]
        );
    }

    #[tokio::test]
    async fn list_slots_returns_slots_with_tickets() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let slot = add_slot(&service, &ticket, now - 600, Some(now - 300)).await;

        let slots = service.list_slots(now - 3600, None, false).await.unwrap();

        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].model, slot);
        assert_eq!(slots[0].ticket, ticket);
    }

    #[tokio::test]
    async fn list_slots_sorts_by_start_or_ticket() {
        let service = test_service().await;
        let first = add_ticket(&service, "JIRA-1", true).await;
        let second = add_ticket(&service, "JIRA-2", true).await;
        let now = Utc::now().timestamp();
        let a = add_slot(&service, &second, now - 3600, Some(now - 3000)).await;
        let b = add_slot(&service, &first, now - 2400, Some(now - 1800)).await;
        let c = add_slot(&service, &second, now - 1200, Some(now - 600)).await;
        let d = add_slot(&service, &first, now - 300, None).await;

        assert_eq!(
            listed_slot_ids(&service, now - 7200, None, false).await,
            [a.id, b.id, c.id, d.id]
        );
        assert_eq!(
            listed_slot_ids(&service, now - 7200, None, true).await,
            [a.id, c.id, b.id, d.id]
        );
    }

    #[tokio::test]
    async fn start_slot_updates_last_worked_at() {
        let service = test_service().await;
        let stored = add_ticket(&service, "JIRA-1", true).await;
        let at = Utc::now().timestamp() - 60;

        let result = service.start_slot("JIRA-1", Some(at), None).await.unwrap();

        assert_eq!(result.started.ticket.last_worked_at, at);
        let ticket = ticket::Entity::find_by_id(stored.id)
            .one(&service.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ticket.last_worked_at, at);
    }

    #[tokio::test]
    async fn add_slot_adds_closed_slot() {
        let service = test_service().await;
        let stored = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();

        let Slot { model, ticket } = service
            .add_slot("JIRA-1", now - 3600, now - 1800, None)
            .await
            .unwrap();

        assert_eq!(model.ticket_id, stored.id);
        assert_eq!(model.started_at, now - 3600);
        assert_eq!(model.stopped_at, Some(now - 1800));
        assert_eq!(ticket.last_worked_at, now - 3600);
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [model]);
    }

    #[tokio::test]
    async fn add_slot_rejects_overlap_with_closed_slot() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let closed = add_slot(&service, &ticket, now - 3600, Some(now - 1800)).await;

        for (started_at, stopped_at) in [
            (now - 4000, now - 3000),
            (now - 3000, now - 2000),
            (now - 2000, now - 1000),
            (now - 4000, now - 1000),
        ] {
            let err = service
                .add_slot("JIRA-1", started_at, stopped_at, None)
                .await
                .unwrap_err();
            assert!(matches!(
                err,
                NirvanaError::Domain(DomainError::SlotOverlap)
            ));
        }
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [closed]);
    }

    #[tokio::test]
    async fn add_slot_rejects_overlap_with_running_slot() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let running = add_slot(&service, &ticket, now - 1800, None).await;

        let err = service
            .add_slot("JIRA-1", now - 900, now - 600, None)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::SlotOverlap)
        ));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [running]);
    }

    #[tokio::test]
    async fn add_slot_accepts_adjacent_slots() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        add_slot(&service, &ticket, now - 3600, Some(now - 2400)).await;
        add_slot(&service, &ticket, now - 1200, None).await;

        service
            .add_slot("JIRA-1", now - 2400, now - 1200, None)
            .await
            .unwrap();

        assert_eq!(slot::Entity::find().count(&service.db).await.unwrap(), 3);
    }

    #[tokio::test]
    async fn add_slot_rejects_future_stop() {
        let service = test_service().await;
        add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();

        let err = service
            .add_slot("JIRA-1", now - 600, now + 60, None)
            .await
            .unwrap_err();

        assert!(matches!(err, NirvanaError::Domain(DomainError::FutureTime)));
        assert_eq!(slot::Entity::find().count(&service.db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn add_slot_rejects_stop_not_after_start() {
        let service = test_service().await;
        add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();

        for stopped_at in [now - 1200, now - 600] {
            let err = service
                .add_slot("JIRA-1", now - 600, stopped_at, None)
                .await
                .unwrap_err();
            assert!(matches!(
                err,
                NirvanaError::Domain(DomainError::InvalidTimeRange)
            ));
        }
        assert_eq!(slot::Entity::find().count(&service.db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn add_slot_rejects_ambiguous_key_without_connection() {
        let service = test_service().await;
        let connected = add_ticket(&service, "JIRA-1", true).await;
        add_ticket(&service, "JIRA-1", false).await;
        let now = Utc::now().timestamp();

        let err = service
            .add_slot("JIRA-1", now - 600, now - 300, None)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::AmbiguousTicket(key, _)) if key == "JIRA-1"
        ));

        let slot = service
            .add_slot("JIRA-1", now - 600, now - 300, Some("Test Jira"))
            .await
            .unwrap();
        assert_eq!(slot.ticket.id, connected.id);
    }

    #[tokio::test]
    async fn add_slot_keeps_later_last_worked_at() {
        let service = test_service().await;
        add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        service
            .add_slot("JIRA-1", now - 600, now - 300, None)
            .await
            .unwrap();

        let slot = service
            .add_slot("JIRA-1", now - 3600, now - 1800, None)
            .await
            .unwrap();

        assert_eq!(slot.ticket.last_worked_at, now - 600);
        let ticket = ticket::Entity::find_by_id(slot.ticket.id)
            .one(&service.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ticket.last_worked_at, now - 600);
    }

    #[tokio::test]
    async fn start_slot_keeps_later_last_worked_at() {
        let service = test_service().await;
        let now = Utc::now().timestamp();
        let mut stored: ticket::ActiveModel = add_ticket(&service, "JIRA-1", true).await.into();
        stored.last_worked_at = Set(now);
        stored.update(&service.db).await.unwrap();

        let result = service
            .start_slot("JIRA-1", Some(now - 600), None)
            .await
            .unwrap();

        assert_eq!(result.started.ticket.last_worked_at, now);
    }

    #[tokio::test]
    async fn get_slot_finds_slot_by_id() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let slot = add_slot(&service, &ticket, now - 600, Some(now - 300)).await;

        let (found, published) = service.get_slot(SlotTarget::Id(slot.id)).await.unwrap();

        assert_eq!(found.model, slot);
        assert_eq!(found.ticket, ticket);
        assert!(!published);
    }

    #[tokio::test]
    async fn get_slot_finds_slot_at_time() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let stopped = add_slot(&service, &ticket, now - 1200, Some(now - 600)).await;
        let running = add_slot(&service, &ticket, now - 600, None).await;

        for (at, expected) in [
            (now - 1200, &stopped),
            (now - 900, &stopped),
            (now - 600, &running),
            (now, &running),
        ] {
            let (found, _) = service.get_slot(SlotTarget::At(at)).await.unwrap();
            assert_eq!(&found.model, expected);
        }
    }

    #[tokio::test]
    async fn get_slot_rejects_time_without_slot() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        add_slot(&service, &ticket, now - 1200, Some(now - 600)).await;

        for at in [now - 1800, now - 600, now - 300] {
            let err = service.get_slot(SlotTarget::At(at)).await.unwrap_err();
            assert!(matches!(err, NirvanaError::Domain(DomainError::NoSlotAt)));
        }
    }

    #[tokio::test]
    async fn get_slot_rejects_future_time() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        add_slot(&service, &ticket, now - 600, None).await;

        let err = service
            .get_slot(SlotTarget::At(now + 60))
            .await
            .unwrap_err();

        assert!(matches!(err, NirvanaError::Domain(DomainError::FutureTime)));
    }

    #[tokio::test]
    async fn get_slot_rejects_slot_of_other_profile() {
        let mut service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let slot = add_slot(&service, &ticket, now - 1200, Some(now - 600)).await;
        service.create_profile("work".to_string()).await.unwrap();
        service.switch_profile("work").await.unwrap();

        let err = service.get_slot(SlotTarget::Id(slot.id)).await.unwrap_err();
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::SlotNotFound(id)) if id == slot.id
        ));
        let err = service
            .get_slot(SlotTarget::At(now - 900))
            .await
            .unwrap_err();
        assert!(matches!(err, NirvanaError::Domain(DomainError::NoSlotAt)));
    }

    #[tokio::test]
    async fn get_slot_reports_published_slot() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let published = add_slot(&service, &ticket, now - 1200, Some(now - 900)).await;
        let failed = add_slot(&service, &ticket, now - 600, Some(now - 300)).await;
        add_publish_entry(&service, &ticket, &published, Some(now)).await;
        add_publish_entry(&service, &ticket, &failed, None).await;

        let (_, is_published) = service
            .get_slot(SlotTarget::Id(published.id))
            .await
            .unwrap();
        assert!(is_published);
        let (_, is_published) = service.get_slot(SlotTarget::Id(failed.id)).await.unwrap();
        assert!(!is_published);
    }

    #[tokio::test]
    async fn delete_slot_deletes_slot() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let deleted = add_slot(&service, &ticket, now - 1200, Some(now - 600)).await;
        let kept = add_slot(&service, &ticket, now - 600, None).await;

        service.delete_slot(deleted.id).await.unwrap();

        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [kept]);
    }

    #[tokio::test]
    async fn delete_slot_deletes_running_slot_found_at_time() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let stopped = add_slot(&service, &ticket, now - 1200, Some(now - 600)).await;
        add_slot(&service, &ticket, now - 600, None).await;

        let (found, _) = service.get_slot(SlotTarget::At(now - 60)).await.unwrap();
        service.delete_slot(found.model.id).await.unwrap();

        assert!(service.running_slot().await.unwrap().is_none());
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [stopped]);
    }

    #[tokio::test]
    async fn delete_slot_deletes_publish_entries() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let deleted = add_slot(&service, &ticket, now - 1200, Some(now - 900)).await;
        let kept = add_slot(&service, &ticket, now - 600, Some(now - 300)).await;
        add_publish_entry(&service, &ticket, &deleted, Some(now)).await;
        let kept_entry = add_publish_entry(&service, &ticket, &kept, Some(now)).await;

        service.delete_slot(deleted.id).await.unwrap();

        let entries = publish_entry::Entity::find()
            .all(&service.db)
            .await
            .unwrap();
        assert_eq!(entries, [kept_entry]);
    }

    #[tokio::test]
    async fn delete_slot_rejects_unknown_slot() {
        let service = test_service().await;
        let ticket = add_ticket(&service, "JIRA-1", true).await;
        let now = Utc::now().timestamp();
        let slot = add_slot(&service, &ticket, now - 600, Some(now - 300)).await;

        let err = service.delete_slot(slot.id + 1).await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::SlotNotFound(id)) if id == slot.id + 1
        ));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [slot]);
    }
}
