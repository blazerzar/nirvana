//! Tracking.
//!
//! A slot is a span of time tracked on a ticket of the active profile. A
//! profile has at most one running slot, which has no stop time, and its slots
//! do not overlap. Starting a slot stops the running one at the same time.
//! Slot times cannot be in the future.

use chrono::Utc;
use nirvana_entity::{connection, publish_entry, slot, ticket};
use sea_orm::sea_query::{Expr, Order, WindowStatement};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseTransaction, EntityTrait, ExprTrait, LoaderTrait,
    ModelTrait, QueryFilter, QueryOrder, QueryTrait, SelectExt, Set, TransactionTrait,
};

use crate::domain::{
    Change, PublishState, PublishTarget, Slot, SlotTarget, StartResult, TargetState,
};
use crate::errors::{DomainError, NirvanaError};
use crate::service::Nirvana;
use crate::service::publish::stuck_before;

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

    /// Returns the ticket that [`Nirvana::resume_slot`] or
    /// [`Nirvana::switch_slot`] starts: the ticket of the last stopped slot,
    /// on another ticket than the running slot.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable or
    /// a database query fails.
    pub async fn previous_ticket(&self) -> Result<Option<ticket::Model>, NirvanaError> {
        let profile = self.find_active_profile().await?;
        let running = find_running_slot(&self.db, profile.id).await?;
        find_previous_ticket(
            &self.db,
            profile.id,
            running.map(|slot| slot.model.ticket_id),
        )
        .await
    }

    /// Returns whether the active profile has any slots.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable or
    /// a database query fails.
    pub async fn has_slots(&self) -> Result<bool, NirvanaError> {
        let profile = self.find_active_profile().await?;
        Ok(slot::Entity::find()
            .filter(slot::COLUMN.profile_id.eq(profile.id))
            .exists(&self.db)
            .await?)
    }

    /// Returns the slots of the active profile started in the range from
    /// `from` to `to`, or unbounded, and the running slot when the range
    /// reaches now, with their tickets and publish states, ordered by start.
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
    ) -> Result<Vec<(Slot, PublishState)>, NirvanaError> {
        let profile = self.find_active_profile().await?;
        let mut query = slot::Entity::find_listed(profile.id, from, to, Utc::now().timestamp())
            .find_also_related(ticket::Entity);
        if by_ticket {
            QueryTrait::query(&mut query)
                .expr_window_as(
                    Expr::col((slot::Entity, slot::Column::StartedAt)).min(),
                    WindowStatement::partition_by((slot::Entity, slot::Column::TicketId)),
                    TICKET_STARTED_AT,
                )
                .order_by(TICKET_STARTED_AT, Order::Asc);
        }
        let rows = query
            .order_by_asc(slot::COLUMN.started_at)
            .all(&self.db)
            .await?;
        with_publish_states(&self.db, rows).await
    }

    /// Returns the stopped slots of the active profile that are not
    /// published, with their tickets and publish states, ordered by start.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable or
    /// a database query fails.
    pub async fn list_unpublished_slots(&self) -> Result<Vec<(Slot, PublishState)>, NirvanaError> {
        let profile = self.find_active_profile().await?;
        let rows = slot::Entity::find()
            .filter(slot::COLUMN.profile_id.eq(profile.id))
            .filter(slot::COLUMN.stopped_at.is_not_null())
            .filter(publish_entry::Entity::not_published())
            .find_also_related(ticket::Entity)
            .order_by_asc(slot::COLUMN.started_at)
            .all(&self.db)
            .await?;
        with_publish_states(&self.db, rows).await
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
        let ticket = find_previous_ticket(&txn, profile.id, None)
            .await?
            .ok_or(DomainError::NoPreviousTicket)?;

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
        let ticket = find_previous_ticket(&txn, profile.id, Some(running.model.ticket_id))
            .await?
            .ok_or(DomainError::NoPreviousTicket)?;

        let result = start_in_txn(&txn, profile.id, Some(running), ticket, at).await?;
        txn.commit().await?;
        Ok(result)
    }

    /// Adds a slot on the ticket `key` of the active profile from
    /// `started_at` to `stopped_at`. The ticket is resolved like in
    /// [`Nirvana::start_slot`].
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

    /// Edits the slot `id` of the active profile and returns it with its
    /// ticket. `key` moves the slot to that ticket, resolved like in
    /// [`Nirvana::start_slot`]. Clearing `stopped_at` reopens the slot.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable,
    /// the connection does not exist, more than one stored ticket matches,
    /// the ticket cannot be loaded, the slot does not exist, is published or
    /// is being published, the ticket changes on a slot that failed to
    /// publish, a time is in the future, the stop is not after the start, another
    /// slot is running when reopening, the slot overlaps another slot or a
    /// database query fails.
    pub async fn edit_slot(
        &self,
        id: i64,
        key: Option<&str>,
        connection: Option<&str>,
        started_at: Option<i64>,
        stopped_at: Change<i64>,
    ) -> Result<Slot, NirvanaError> {
        let profile = self.find_active_profile().await?;
        let ticket = match key {
            Some(key) => Some(
                self.find_or_load_ticket(profile.id, key, connection)
                    .await?,
            ),
            None => None,
        };

        let txn = self.db.begin().await?;
        let Some((model, current_ticket)) = slot::Entity::find_by_id(id)
            .filter(slot::COLUMN.profile_id.eq(profile.id))
            .find_also_related(ticket::Entity)
            .one(&txn)
            .await?
        else {
            return Err(DomainError::SlotNotFound(id).into());
        };
        let entries = publish_entry::Entity::find()
            .filter(publish_entry::COLUMN.slot_id.eq(id))
            .all(&txn)
            .await?;
        if entries.iter().any(|entry| entry.published_at.is_some()) {
            return Err(DomainError::SlotAlreadyPublished.into());
        }
        if entries.iter().any(publish_entry::Model::is_pending) {
            return Err(DomainError::SlotBeingPublished.into());
        }
        if !entries.is_empty()
            && ticket
                .as_ref()
                .is_some_and(|ticket| ticket.id != model.ticket_id)
        {
            return Err(DomainError::SlotTicketLocked.into());
        }
        let ticket = ticket
            .or(current_ticket)
            .ok_or(DomainError::TicketIdNotFound(model.ticket_id))?;

        let started_at = started_at.unwrap_or(model.started_at);
        let stopped_at = match stopped_at {
            Change::Skip => model.stopped_at,
            Change::Clear => None,
            Change::Set(stopped_at) => Some(stopped_at),
        };
        reject_future_time(stopped_at.unwrap_or(started_at))?;
        if stopped_at.is_some_and(|stopped_at| stopped_at <= started_at) {
            return Err(DomainError::InvalidTimeRange.into());
        }
        if model.stopped_at.is_some()
            && stopped_at.is_none()
            && slot::Entity::find_running(profile.id).exists(&txn).await?
        {
            return Err(DomainError::AlreadyTracking.into());
        }
        if slot::Entity::find_in_range(profile.id, started_at, stopped_at)
            .filter(slot::COLUMN.id.ne(id))
            .exists(&txn)
            .await?
        {
            return Err(DomainError::SlotOverlap.into());
        }

        let mut model: slot::ActiveModel = model.into();
        model.ticket_id = Set(ticket.id);
        model.started_at = Set(started_at);
        model.stopped_at = Set(stopped_at);
        let model = model.update(&txn).await?;
        let ticket = mark_worked(&txn, ticket, started_at).await?;
        txn.commit().await?;
        Ok(Slot { model, ticket })
    }

    /// Returns the slot of the active profile with the ID, or tracked at the
    /// time, of `target`, with its ticket and publish state.
    ///
    /// # Errors
    ///
    /// Returns an error if the time is in the future, no profile is active,
    /// the config is unavailable, no slot matches or a database query fails.
    pub async fn get_slot(&self, target: SlotTarget) -> Result<(Slot, PublishState), NirvanaError> {
        let profile = self.find_active_profile().await?;
        let slot = find_slot(&self.db, profile.id, target).await?;
        let entries = slot
            .model
            .find_related(publish_entry::Entity)
            .all(&self.db)
            .await?;
        let state = PublishState::from_entries(&entries, stuck_before());
        Ok((slot, state))
    }

    /// Returns the slot of the active profile with the ID, or tracked at the
    /// time, of `target`, with its ticket and publish targets ordered by
    /// connection name.
    ///
    /// # Errors
    ///
    /// Returns an error if the time is in the future, no profile is active,
    /// the config is unavailable, no slot matches or a database query fails.
    pub async fn get_slot_targets(
        &self,
        target: SlotTarget,
    ) -> Result<(Slot, Vec<PublishTarget>), NirvanaError> {
        let profile = self.find_active_profile().await?;
        let slot = find_slot(&self.db, profile.id, target).await?;
        let stuck_before = stuck_before();
        let targets = slot
            .model
            .find_related(publish_entry::Entity)
            .find_both_related(connection::Entity)
            .order_by_asc(connection::COLUMN.name)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|(entry, connection)| PublishTarget {
                state: TargetState::from_entry(&entry, stuck_before),
                entry,
                connection,
            })
            .collect();
        Ok((slot, targets))
    }

    /// Deletes the slot `id` of the active profile with its publish entries.
    /// A slot with only stuck pending entries can be deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable,
    /// the slot is being published, does not exist or a database query fails.
    pub async fn delete_slot(&self, id: i64) -> Result<(), NirvanaError> {
        let profile = self.find_active_profile().await?;
        let txn = self.db.begin().await?;
        if publish_entry::Entity::find()
            .filter(publish_entry::COLUMN.slot_id.eq(id))
            .filter(publish_entry::Entity::pending())
            .filter(publish_entry::COLUMN.claimed_at.gte(stuck_before()))
            .exists(&txn)
            .await?
        {
            return Err(DomainError::SlotBeingPublished.into());
        }
        let result = slot::Entity::delete_many()
            .filter(slot::COLUMN.id.eq(id))
            .filter(slot::COLUMN.profile_id.eq(profile.id))
            .exec(&txn)
            .await?;
        if result.rows_affected == 0 {
            return Err(DomainError::SlotNotFound(id).into());
        }
        txn.commit().await?;
        Ok(())
    }
}

/// Returns the slot of the profile `profile_id` with the ID, or tracked at
/// the time, of `target`, with its ticket.
async fn find_slot(
    db: &impl ConnectionTrait,
    profile_id: i64,
    target: SlotTarget,
) -> Result<Slot, NirvanaError> {
    if let SlotTarget::At(at) = target {
        reject_future_time(at)?;
    }
    let query = match target {
        SlotTarget::Id(id) => {
            slot::Entity::find_by_id(id).filter(slot::COLUMN.profile_id.eq(profile_id))
        }
        SlotTarget::At(at) => slot::Entity::find_at(profile_id, at),
    };
    let Some((model, ticket)) = query.find_also_related(ticket::Entity).one(db).await? else {
        return Err(match target {
            SlotTarget::Id(id) => DomainError::SlotNotFound(id),
            SlotTarget::At(_) => DomainError::NoSlotAt,
        }
        .into());
    };
    let ticket = ticket.ok_or(DomainError::TicketIdNotFound(model.ticket_id))?;
    Ok(Slot { model, ticket })
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
) -> Result<Option<ticket::Model>, NirvanaError> {
    let Some((slot, ticket)) = slot::Entity::find_last_stopped(profile_id)
        .apply_if(skipped_ticket_id, |query, id| {
            query.filter(slot::COLUMN.ticket_id.ne(id))
        })
        .find_also_related(ticket::Entity)
        .one(db)
        .await?
    else {
        return Ok(None);
    };
    let ticket = ticket.ok_or(DomainError::TicketIdNotFound(slot.ticket_id))?;
    Ok(Some(ticket))
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

/// Pairs each slot with its ticket and its publish state.
async fn with_publish_states(
    db: &impl ConnectionTrait,
    rows: Vec<(slot::Model, Option<ticket::Model>)>,
) -> Result<Vec<(Slot, PublishState)>, NirvanaError> {
    let (models, tickets): (Vec<_>, Vec<_>) = rows.into_iter().unzip();
    let entries = models.load_many(publish_entry::Entity, db).await?;
    let stuck_before = stuck_before();
    models
        .into_iter()
        .zip(tickets)
        .zip(entries)
        .map(|((model, ticket), entries)| {
            let ticket = ticket.ok_or(DomainError::TicketIdNotFound(model.ticket_id))?;
            let state = PublishState::from_entries(&entries, stuck_before);
            Ok((Slot { model, ticket }, state))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use sea_orm::PaginatorTrait;

    use super::*;
    use crate::service::test_service::TestConnection::{Jira, Linear};
    use crate::service::test_service::TestService;

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

    /// Returns a service with the connection "Test Linear" and its tickets
    /// "NIR-1" and "NIR-2".
    async fn setup() -> (TestService, [ticket::Model; 2]) {
        let mut service = TestService::new(&[Linear]).await;
        service.use_connection(Some(Linear));
        let tickets = [
            service.seed_ticket("NIR-1", "Add export", 0, 0).await,
            service.seed_ticket("NIR-2", "Fix login", 0, 0).await,
        ];
        (service, tickets)
    }

    #[tokio::test]
    async fn start_slot_stops_running_slot() {
        let (service, [previous, _]) = setup().await;
        let now = Utc::now().timestamp();
        let running = service.seed_slot(&previous, now - 600, None).await;

        let result = service.start_slot("NIR-2", None, None).await.unwrap();

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
        let (service, [stored, _]) = setup().await;
        let tickets = ticket::Entity::find().count(&service.db).await.unwrap();

        let result = service.start_slot("NIR-1", None, None).await.unwrap();

        let Slot {
            model: slot,
            ticket,
        } = result.started;
        assert_eq!(slot.ticket_id, stored.id);
        assert_eq!(ticket.id, stored.id);
        assert!(result.stopped.is_none());
        assert_eq!(
            ticket::Entity::find().count(&service.db).await.unwrap(),
            tickets
        );
    }

    #[tokio::test]
    async fn start_slot_rejects_ambiguous_key_without_connection() {
        let (mut service, [connected, _]) = setup().await;
        service.use_connection(None);
        service.seed_ticket("NIR-1", "Add export", 0, 0).await;

        let err = service.start_slot("NIR-1", None, None).await.unwrap_err();
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::AmbiguousTicket(key, _)) if key == "NIR-1"
        ));

        let result = service
            .start_slot("NIR-1", None, Some(Linear.name()))
            .await
            .unwrap();
        assert_eq!(result.started.ticket.id, connected.id);
    }

    #[tokio::test]
    async fn start_slot_rejects_future_time() {
        let (service, _) = setup().await;
        let now = Utc::now().timestamp();

        let err = service
            .start_slot("NIR-1", Some(now + 60), None)
            .await
            .unwrap_err();

        assert!(matches!(err, NirvanaError::Domain(DomainError::FutureTime)));
    }

    #[tokio::test]
    async fn start_slot_rejects_backdated_overlap_with_closed_slot() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        service
            .seed_slot(&ticket, now - 3600, Some(now - 1800))
            .await;

        let err = service
            .start_slot("NIR-1", Some(now - 2700), None)
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
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let running = service.seed_slot(&ticket, now - 600, None).await;

        for at in [now - 1200, now - 600] {
            let err = service
                .start_slot("NIR-1", Some(at), None)
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
        let (service, [ticket, _]) = setup().await;
        let running = service
            .seed_slot(&ticket, Utc::now().timestamp() + 60, None)
            .await;

        let err = service.start_slot("NIR-1", None, None).await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::RunningSlotJustStarted)
        ));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [running]);
    }

    #[tokio::test]
    async fn start_slot_backdated_stops_running_slot_at_start() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        service
            .seed_slot(&ticket, now - 3600, Some(now - 1800))
            .await;
        let running = service.seed_slot(&ticket, now - 1200, None).await;

        let result = service
            .start_slot("NIR-1", Some(now - 600), None)
            .await
            .unwrap();

        let stopped = result.stopped.unwrap().model;
        assert_eq!(stopped.id, running.id);
        assert_eq!(stopped.stopped_at, Some(now - 600));
        assert_eq!(result.started.model.started_at, now - 600);
        assert_eq!(result.started.model.stopped_at, None);
    }

    #[tokio::test]
    async fn has_slots_is_false_without_slots() {
        let (service, _) = setup().await;

        assert!(!service.has_slots().await.unwrap());
    }

    #[tokio::test]
    async fn has_slots_is_true_with_a_slot() {
        let (service, [ticket, _]) = setup().await;
        service
            .seed_slot(&ticket, Utc::now().timestamp() - 600, None)
            .await;

        assert!(service.has_slots().await.unwrap());
    }

    #[tokio::test]
    async fn previous_ticket_is_none_without_slots() {
        let (service, _) = setup().await;

        assert_eq!(service.previous_ticket().await.unwrap(), None);
    }

    #[tokio::test]
    async fn previous_ticket_is_none_with_only_a_running_slot() {
        let (service, [ticket, _]) = setup().await;
        service
            .seed_slot(&ticket, Utc::now().timestamp() - 600, None)
            .await;

        assert_eq!(service.previous_ticket().await.unwrap(), None);
    }

    #[tokio::test]
    async fn previous_ticket_returns_ticket_of_last_stopped_slot() {
        let (service, [first, last]) = setup().await;
        let now = Utc::now().timestamp();
        service.seed_slot(&last, now - 3600, Some(now - 3000)).await;
        service
            .seed_slot(&first, now - 2400, Some(now - 1800))
            .await;
        service.seed_slot(&last, now - 1200, Some(now - 600)).await;

        assert_eq!(service.previous_ticket().await.unwrap(), Some(last));
    }

    #[tokio::test]
    async fn previous_ticket_skips_running_ticket() {
        let (service, [previous, current]) = setup().await;
        let now = Utc::now().timestamp();
        service
            .seed_slot(&previous, now - 3600, Some(now - 3000))
            .await;
        service
            .seed_slot(&current, now - 2400, Some(now - 1800))
            .await;
        service.seed_slot(&current, now - 1200, None).await;

        assert_eq!(service.previous_ticket().await.unwrap(), Some(previous));
    }

    #[tokio::test]
    async fn previous_ticket_is_none_when_only_running_ticket_was_stopped() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;
        service.seed_slot(&ticket, now - 300, None).await;

        assert_eq!(service.previous_ticket().await.unwrap(), None);
    }

    #[tokio::test]
    async fn previous_ticket_ignores_slots_of_other_profiles() {
        let (mut service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;
        service.use_profile("work").await;

        assert_eq!(service.previous_ticket().await.unwrap(), None);
    }

    #[tokio::test]
    async fn stop_slot_stops_running_slot() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let running = service.seed_slot(&ticket, now - 600, None).await;

        let stopped = service.stop_slot(Some(now - 60)).await.unwrap().unwrap();

        assert_eq!(stopped.model.id, running.id);
        assert_eq!(stopped.model.stopped_at, Some(now - 60));
        assert_eq!(stopped.ticket, ticket);
        assert!(service.running_slot().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn stop_slot_stops_running_slot_now() {
        let (service, [ticket, _]) = setup().await;
        let running = service
            .seed_slot(&ticket, Utc::now().timestamp() - 600, None)
            .await;

        let before = Utc::now().timestamp();
        let stopped = service.stop_slot(None).await.unwrap().unwrap();
        let after = Utc::now().timestamp();

        assert_eq!(stopped.model.id, running.id);
        let stopped_at = stopped.model.stopped_at.unwrap();
        assert!((before..=after).contains(&stopped_at));
    }

    #[tokio::test]
    async fn stop_slot_rejects_running_slot_started_in_same_second() {
        let (service, [ticket, _]) = setup().await;
        let running = service
            .seed_slot(&ticket, Utc::now().timestamp() + 60, None)
            .await;

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
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        service.seed_slot(&ticket, now - 600, Some(now - 300)).await;

        assert!(service.stop_slot(None).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn stop_slot_rejects_future_time() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let running = service.seed_slot(&ticket, now - 600, None).await;

        let err = service.stop_slot(Some(now + 60)).await.unwrap_err();

        assert!(matches!(err, NirvanaError::Domain(DomainError::FutureTime)));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [running]);
    }

    #[tokio::test]
    async fn stop_slot_rejects_time_not_after_start() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let running = service.seed_slot(&ticket, now - 600, None).await;

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
        let (service, [first, last]) = setup().await;
        let now = Utc::now().timestamp();
        service.seed_slot(&last, now - 3600, Some(now - 3000)).await;
        service
            .seed_slot(&first, now - 2400, Some(now - 1800))
            .await;
        service.seed_slot(&last, now - 1200, Some(now - 600)).await;

        let resumed = service.resume_slot(None).await.unwrap();

        assert_eq!(resumed.ticket.id, last.id);
        assert_eq!(resumed.model.stopped_at, None);
        let running = service.running_slot().await.unwrap().unwrap();
        assert_eq!(running.model, resumed.model);
    }

    #[tokio::test]
    async fn resume_slot_starts_at_time() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;

        let resumed = service.resume_slot(Some(now - 300)).await.unwrap();

        assert_eq!(resumed.ticket.id, ticket.id);
        assert_eq!(resumed.model.started_at, now - 300);
        assert_eq!(resumed.model.stopped_at, None);
    }

    #[tokio::test]
    async fn resume_slot_rejects_future_time() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let stopped = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;

        let err = service.resume_slot(Some(now + 60)).await.unwrap_err();

        assert!(matches!(err, NirvanaError::Domain(DomainError::FutureTime)));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [stopped]);
    }

    #[tokio::test]
    async fn resume_slot_rejects_overlap_with_last_slot() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let stopped = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;

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
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let stopped = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;
        let running = service.seed_slot(&ticket, now - 300, None).await;

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
        let (service, _) = setup().await;

        let err = service.resume_slot(None).await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::NoPreviousTicket)
        ));
        assert_eq!(slot::Entity::find().count(&service.db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn resume_slot_ignores_slots_of_other_profiles() {
        let (mut service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let stopped = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;
        service.use_profile("work").await;

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
        let (service, [previous, current]) = setup().await;
        let now = Utc::now().timestamp();
        service
            .seed_slot(&previous, now - 1200, Some(now - 600))
            .await;
        let running = service.seed_slot(&current, now - 600, None).await;

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
        let (service, [previous, current]) = setup().await;
        let now = Utc::now().timestamp();
        service
            .seed_slot(&previous, now - 1200, Some(now - 600))
            .await;
        let running = service.seed_slot(&current, now - 600, None).await;

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
        let (service, [previous, current]) = setup().await;
        let now = Utc::now().timestamp();
        let stopped = service
            .seed_slot(&previous, now - 1200, Some(now - 600))
            .await;
        let running = service.seed_slot(&current, now - 600, None).await;

        let err = service.switch_slot(Some(now + 60)).await.unwrap_err();

        assert!(matches!(err, NirvanaError::Domain(DomainError::FutureTime)));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [stopped, running]);
    }

    #[tokio::test]
    async fn switch_slot_rejects_time_before_running_slot_start() {
        let (service, [previous, current]) = setup().await;
        let now = Utc::now().timestamp();
        let stopped = service
            .seed_slot(&previous, now - 1200, Some(now - 900))
            .await;
        let running = service.seed_slot(&current, now - 600, None).await;

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
        let (service, [previous, current]) = setup().await;
        let now = Utc::now().timestamp();
        service
            .seed_slot(&previous, now - 3600, Some(now - 3000))
            .await;
        service
            .seed_slot(&current, now - 2400, Some(now - 1800))
            .await;
        service.seed_slot(&current, now - 1200, None).await;

        let result = service.switch_slot(None).await.unwrap();

        assert_eq!(result.started.ticket.id, previous.id);
    }

    #[tokio::test]
    async fn switch_slot_rejects_when_not_tracking() {
        let (service, [first, second]) = setup().await;
        let now = Utc::now().timestamp();
        let a = service.seed_slot(&first, now - 1200, Some(now - 900)).await;
        let b = service.seed_slot(&second, now - 600, Some(now - 300)).await;

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
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let stopped = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;
        let running = service.seed_slot(&ticket, now - 300, None).await;

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
        let (service, [previous, current]) = setup().await;
        let now = Utc::now().timestamp();
        let stopped = service
            .seed_slot(&previous, now - 1200, Some(now - 600))
            .await;
        let running = service.seed_slot(&current, now + 60, None).await;

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
        let (service, [ticket, _]) = setup().await;
        let running = service
            .seed_slot(&ticket, Utc::now().timestamp() - 300, None)
            .await;

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
            .map(|(slot, _)| slot.model.id)
            .collect()
    }

    #[tokio::test]
    async fn list_slots_returns_slots_started_in_range() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let crossing_start = service
            .seed_slot(&ticket, now - 5000, Some(now - 3000))
            .await;
        let at_start = service
            .seed_slot(&ticket, now - 2500, Some(now - 2000))
            .await;
        let crossing_stop = service
            .seed_slot(&ticket, now - 1500, Some(now - 500))
            .await;
        let running = service.seed_slot(&ticket, now - 300, None).await;

        assert_eq!(
            listed_slot_ids(&service, now - 2500, Some(now - 1000), false).await,
            [at_start.id, crossing_stop.id]
        );
        assert_eq!(
            listed_slot_ids(&service, now - 4000, Some(now - 1500), false).await,
            [at_start.id]
        );
        assert_eq!(
            listed_slot_ids(&service, now - 4000, None, false).await,
            [at_start.id, crossing_stop.id, running.id]
        );
        assert_eq!(
            listed_slot_ids(&service, now - 6000, Some(now - 4000), false).await,
            [crossing_start.id]
        );
    }

    #[tokio::test]
    async fn list_slots_returns_running_slot_when_range_reaches_now() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let running = service.seed_slot(&ticket, now - 3000, None).await;

        assert_eq!(
            listed_slot_ids(&service, now - 1000, None, false).await,
            [running.id]
        );
        assert_eq!(
            listed_slot_ids(&service, now - 1000, Some(now + 60), false).await,
            [running.id]
        );
        assert!(
            listed_slot_ids(&service, now - 1000, Some(now - 500), false)
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn list_slots_returns_slots_with_tickets() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let slot = service.seed_slot(&ticket, now - 600, Some(now - 300)).await;

        let slots = service.list_slots(now - 3600, None, false).await.unwrap();

        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].0.model, slot);
        assert_eq!(slots[0].0.ticket, ticket);
    }

    #[tokio::test]
    async fn list_slots_returns_publish_states() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let unpublished = service
            .seed_slot(&ticket, now - 3000, Some(now - 2700))
            .await;
        let published = service
            .seed_slot(&ticket, now - 2400, Some(now - 2100))
            .await;
        let pending = service
            .seed_slot(&ticket, now - 1800, Some(now - 1500))
            .await;
        let stuck = service
            .seed_slot(&ticket, now - 1200, Some(now - 900))
            .await;
        let failed = service.seed_slot(&ticket, now - 600, Some(now - 300)).await;
        service
            .seed_publish_entry(&published, Linear, Some(now), None, now)
            .await;
        service
            .seed_publish_entry(&pending, Linear, None, None, now)
            .await;
        service
            .seed_publish_entry(&stuck, Linear, None, None, now - 3600)
            .await;
        service
            .seed_publish_entry(&failed, Linear, None, Some("error"), now)
            .await;

        let states: Vec<_> = service
            .list_slots(now - 3600, None, false)
            .await
            .unwrap()
            .into_iter()
            .map(|(slot, state)| (slot.model.id, state))
            .collect();

        assert_eq!(
            states,
            [
                (unpublished.id, PublishState::Unpublished),
                (published.id, PublishState::Published),
                (pending.id, PublishState::Pending),
                (stuck.id, PublishState::Failed),
                (failed.id, PublishState::Failed),
            ]
        );
    }

    #[tokio::test]
    async fn list_unpublished_slots_returns_stopped_slots_not_published() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let old = service
            .seed_slot(&ticket, now - 90_000, Some(now - 89_000))
            .await;
        let published = service
            .seed_slot(&ticket, now - 2400, Some(now - 2100))
            .await;
        let pending = service
            .seed_slot(&ticket, now - 1800, Some(now - 1500))
            .await;
        let failed = service
            .seed_slot(&ticket, now - 1200, Some(now - 900))
            .await;
        service.seed_slot(&ticket, now - 300, None).await;
        service
            .seed_publish_entry(&published, Linear, Some(now), None, now)
            .await;
        service
            .seed_publish_entry(&pending, Linear, None, None, now)
            .await;
        service
            .seed_publish_entry(&failed, Linear, None, Some("error"), now)
            .await;

        let states: Vec<_> = service
            .list_unpublished_slots()
            .await
            .unwrap()
            .into_iter()
            .map(|(slot, state)| (slot.model.id, state))
            .collect();

        assert_eq!(
            states,
            [
                (old.id, PublishState::Unpublished),
                (pending.id, PublishState::Pending),
                (failed.id, PublishState::Failed),
            ]
        );
    }

    #[tokio::test]
    async fn list_slots_sorts_by_start_or_ticket() {
        let (service, [first, second]) = setup().await;
        let now = Utc::now().timestamp();
        let a = service
            .seed_slot(&second, now - 3600, Some(now - 3000))
            .await;
        let b = service
            .seed_slot(&first, now - 2400, Some(now - 1800))
            .await;
        let c = service
            .seed_slot(&second, now - 1200, Some(now - 600))
            .await;
        let d = service.seed_slot(&first, now - 300, None).await;

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
        let (service, [stored, _]) = setup().await;
        let at = Utc::now().timestamp() - 60;

        let result = service.start_slot("NIR-1", Some(at), None).await.unwrap();

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
        let (service, [stored, _]) = setup().await;
        let now = Utc::now().timestamp();

        let Slot { model, ticket } = service
            .add_slot("NIR-1", now - 3600, now - 1800, None)
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
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let closed = service
            .seed_slot(&ticket, now - 3600, Some(now - 1800))
            .await;

        for (started_at, stopped_at) in [
            (now - 4000, now - 3000),
            (now - 3000, now - 2000),
            (now - 2000, now - 1000),
            (now - 4000, now - 1000),
        ] {
            let err = service
                .add_slot("NIR-1", started_at, stopped_at, None)
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
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let running = service.seed_slot(&ticket, now - 1800, None).await;

        let err = service
            .add_slot("NIR-1", now - 900, now - 600, None)
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
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        service
            .seed_slot(&ticket, now - 3600, Some(now - 2400))
            .await;
        service.seed_slot(&ticket, now - 1200, None).await;

        service
            .add_slot("NIR-1", now - 2400, now - 1200, None)
            .await
            .unwrap();

        assert_eq!(slot::Entity::find().count(&service.db).await.unwrap(), 3);
    }

    #[tokio::test]
    async fn add_slot_rejects_future_stop() {
        let (service, _) = setup().await;
        let now = Utc::now().timestamp();

        let err = service
            .add_slot("NIR-1", now - 600, now + 60, None)
            .await
            .unwrap_err();

        assert!(matches!(err, NirvanaError::Domain(DomainError::FutureTime)));
        assert_eq!(slot::Entity::find().count(&service.db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn add_slot_rejects_stop_not_after_start() {
        let (service, _) = setup().await;
        let now = Utc::now().timestamp();

        for stopped_at in [now - 1200, now - 600] {
            let err = service
                .add_slot("NIR-1", now - 600, stopped_at, None)
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
        let (mut service, [connected, _]) = setup().await;
        service.use_connection(None);
        service.seed_ticket("NIR-1", "Add export", 0, 0).await;
        let now = Utc::now().timestamp();

        let err = service
            .add_slot("NIR-1", now - 600, now - 300, None)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::AmbiguousTicket(key, _)) if key == "NIR-1"
        ));

        let slot = service
            .add_slot("NIR-1", now - 600, now - 300, Some(Linear.name()))
            .await
            .unwrap();
        assert_eq!(slot.ticket.id, connected.id);
    }

    #[tokio::test]
    async fn add_slot_keeps_later_last_worked_at() {
        let (service, _) = setup().await;
        let now = Utc::now().timestamp();
        service
            .add_slot("NIR-1", now - 600, now - 300, None)
            .await
            .unwrap();

        let slot = service
            .add_slot("NIR-1", now - 3600, now - 1800, None)
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
        let (service, [stored, _]) = setup().await;
        let now = Utc::now().timestamp();
        let mut stored: ticket::ActiveModel = stored.into();
        stored.last_worked_at = Set(now);
        stored.update(&service.db).await.unwrap();

        let result = service
            .start_slot("NIR-1", Some(now - 600), None)
            .await
            .unwrap();

        assert_eq!(result.started.ticket.last_worked_at, now);
    }

    #[tokio::test]
    async fn edit_slot_edits_times_and_ticket() {
        let (service, [previous, next]) = setup().await;
        let now = Utc::now().timestamp();
        let slot = service
            .seed_slot(&previous, now - 3600, Some(now - 1800))
            .await;

        let Slot { model, ticket } = service
            .edit_slot(
                slot.id,
                Some("NIR-2"),
                None,
                Some(now - 1200),
                Change::Set(now - 600),
            )
            .await
            .unwrap();

        assert_eq!(model.id, slot.id);
        assert_eq!(model.ticket_id, next.id);
        assert_eq!(model.started_at, now - 1200);
        assert_eq!(model.stopped_at, Some(now - 600));
        assert_eq!(ticket.id, next.id);
        assert_eq!(ticket.last_worked_at, now - 1200);
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [model]);
    }

    #[tokio::test]
    async fn edit_slot_keeps_fields_that_are_not_edited() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let slot = service
            .seed_slot(&ticket, now - 3600, Some(now - 1800))
            .await;

        let edited = service
            .edit_slot(slot.id, None, None, Some(now - 2400), Change::Skip)
            .await
            .unwrap();

        assert_eq!(edited.model.ticket_id, ticket.id);
        assert_eq!(edited.model.started_at, now - 2400);
        assert_eq!(edited.model.stopped_at, Some(now - 1800));
    }

    #[tokio::test]
    async fn edit_slot_rejects_slot_being_published() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let slot = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;
        service
            .seed_publish_entry(&slot, Linear, None, None, now)
            .await;

        let err = service
            .edit_slot(slot.id, None, None, Some(now - 900), Change::Skip)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::SlotBeingPublished)
        ));
    }

    #[tokio::test]
    async fn edit_slot_keeps_ticket_of_failed_slot() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let slot = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;
        service
            .seed_publish_entry(&slot, Linear, None, Some("error"), now)
            .await;

        let err = service
            .edit_slot(slot.id, Some("NIR-2"), None, None, Change::Skip)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::SlotTicketLocked)
        ));

        let edited = service
            .edit_slot(slot.id, Some("NIR-1"), None, Some(now - 900), Change::Skip)
            .await
            .unwrap();
        assert_eq!(edited.model.ticket_id, ticket.id);
        assert_eq!(edited.model.started_at, now - 900);
    }

    #[tokio::test]
    async fn edit_slot_rejects_published_slot() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let slot = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;
        service
            .seed_publish_entry(&slot, Linear, Some(now), None, now)
            .await;

        let err = service
            .edit_slot(slot.id, None, None, Some(now - 900), Change::Skip)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::SlotAlreadyPublished)
        ));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [slot]);
    }

    #[tokio::test]
    async fn edit_slot_rejects_unknown_slot_and_slot_of_other_profile() {
        let (mut service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let slot = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;

        let err = service
            .edit_slot(slot.id + 1, None, None, None, Change::Clear)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::SlotNotFound(id)) if id == slot.id + 1
        ));

        service.use_profile("work").await;
        let err = service
            .edit_slot(slot.id, None, None, None, Change::Clear)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::SlotNotFound(id)) if id == slot.id
        ));
    }

    #[tokio::test]
    async fn edit_slot_rejects_invalid_and_future_range() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let slot = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;

        for stopped_at in [Change::Set(now - 1200), Change::Set(now - 1800)] {
            let err = service
                .edit_slot(slot.id, None, None, None, stopped_at)
                .await
                .unwrap_err();
            assert!(matches!(
                err,
                NirvanaError::Domain(DomainError::InvalidTimeRange)
            ));
        }
        for (started_at, stopped_at) in [
            (None, Change::Set(now + 60)),
            (Some(now + 60), Change::Clear),
        ] {
            let err = service
                .edit_slot(slot.id, None, None, started_at, stopped_at)
                .await
                .unwrap_err();
            assert!(matches!(err, NirvanaError::Domain(DomainError::FutureTime)));
        }
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [slot]);
    }

    #[tokio::test]
    async fn edit_slot_rejects_overlap() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let first = service
            .seed_slot(&ticket, now - 3600, Some(now - 2400))
            .await;
        let second = service
            .seed_slot(&ticket, now - 1800, Some(now - 1200))
            .await;
        let running = service.seed_slot(&ticket, now - 600, None).await;

        for (started_at, stopped_at) in [
            (Some(now - 3000), Change::Skip),
            (None, Change::Set(now - 300)),
        ] {
            let err = service
                .edit_slot(second.id, None, None, started_at, stopped_at)
                .await
                .unwrap_err();
            assert!(matches!(
                err,
                NirvanaError::Domain(DomainError::SlotOverlap)
            ));
        }
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [first, second, running]);
    }

    #[tokio::test]
    async fn edit_slot_moves_its_own_edges() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        service
            .seed_slot(&ticket, now - 3600, Some(now - 2400))
            .await;
        let slot = service
            .seed_slot(&ticket, now - 1800, Some(now - 1200))
            .await;
        service.seed_slot(&ticket, now - 600, None).await;

        let edited = service
            .edit_slot(
                slot.id,
                None,
                None,
                Some(now - 2400),
                Change::Set(now - 600),
            )
            .await
            .unwrap();

        assert_eq!(edited.model.started_at, now - 2400);
        assert_eq!(edited.model.stopped_at, Some(now - 600));
    }

    #[tokio::test]
    async fn edit_slot_keeps_running_slot_running() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let running = service.seed_slot(&ticket, now - 600, None).await;

        let edited = service
            .edit_slot(running.id, None, None, Some(now - 1200), Change::Skip)
            .await
            .unwrap();

        assert_eq!(edited.model.started_at, now - 1200);
        assert_eq!(edited.model.stopped_at, None);
        assert_eq!(
            service.running_slot().await.unwrap().unwrap().model,
            edited.model
        );
    }

    #[tokio::test]
    async fn edit_slot_reopens_slot() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let slot = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;
        assert!(service.running_slot().await.unwrap().is_none());

        let edited = service
            .edit_slot(slot.id, None, None, None, Change::Clear)
            .await
            .unwrap();

        assert_eq!(edited.model.stopped_at, None);
        assert_eq!(
            service.running_slot().await.unwrap().unwrap().model,
            edited.model
        );
    }

    #[tokio::test]
    async fn edit_slot_rejects_reopen_while_tracking() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let slot = service
            .seed_slot(&ticket, now - 1200, Some(now - 900))
            .await;
        let running = service.seed_slot(&ticket, now - 600, None).await;

        let err = service
            .edit_slot(slot.id, None, None, None, Change::Clear)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::AlreadyTracking)
        ));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [slot, running]);
    }

    #[tokio::test]
    async fn edit_slot_rejects_reopen_before_later_slot() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let slot = service
            .seed_slot(&ticket, now - 1200, Some(now - 900))
            .await;
        let later = service.seed_slot(&ticket, now - 600, Some(now - 300)).await;

        let err = service
            .edit_slot(slot.id, None, None, None, Change::Clear)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::SlotOverlap)
        ));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [slot, later]);
    }

    #[tokio::test]
    async fn get_slot_finds_slot_by_id() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let slot = service.seed_slot(&ticket, now - 600, Some(now - 300)).await;

        let (found, state) = service.get_slot(SlotTarget::Id(slot.id)).await.unwrap();

        assert_eq!(found.model, slot);
        assert_eq!(found.ticket, ticket);
        assert_eq!(state, PublishState::Unpublished);
    }

    #[tokio::test]
    async fn get_slot_finds_slot_at_time() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let stopped = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;
        let running = service.seed_slot(&ticket, now - 600, None).await;

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
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;

        for at in [now - 1800, now - 600, now - 300] {
            let err = service.get_slot(SlotTarget::At(at)).await.unwrap_err();
            assert!(matches!(err, NirvanaError::Domain(DomainError::NoSlotAt)));
        }
    }

    #[tokio::test]
    async fn get_slot_rejects_future_time() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        service.seed_slot(&ticket, now - 600, None).await;

        let err = service
            .get_slot(SlotTarget::At(now + 60))
            .await
            .unwrap_err();

        assert!(matches!(err, NirvanaError::Domain(DomainError::FutureTime)));
    }

    #[tokio::test]
    async fn get_slot_rejects_slot_of_other_profile() {
        let (mut service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let slot = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;
        service.use_profile("work").await;

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
    async fn get_slot_reports_publish_state() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let published = service
            .seed_slot(&ticket, now - 1800, Some(now - 1500))
            .await;
        let pending = service
            .seed_slot(&ticket, now - 1200, Some(now - 900))
            .await;
        let failed = service.seed_slot(&ticket, now - 600, Some(now - 300)).await;
        service
            .seed_publish_entry(&published, Linear, Some(now), None, now)
            .await;
        service
            .seed_publish_entry(&pending, Linear, None, None, now)
            .await;
        service
            .seed_publish_entry(&failed, Linear, None, Some("error"), now)
            .await;

        for (slot, expected) in [
            (published, PublishState::Published),
            (pending, PublishState::Pending),
            (failed, PublishState::Failed),
        ] {
            let (_, state) = service.get_slot(SlotTarget::Id(slot.id)).await.unwrap();
            assert_eq!(state, expected);
        }
    }

    #[tokio::test]
    async fn get_slot_targets_returns_entries_of_slot_with_states() {
        let (service, [ticket, _]) = setup().await;
        service.seed(Jira).await;
        let now = Utc::now().timestamp();
        let slot = service
            .seed_slot(&ticket, now - 1200, Some(now - 900))
            .await;
        let other = service.seed_slot(&ticket, now - 600, Some(now - 300)).await;
        service
            .seed_publish_entry(&slot, Linear, Some(now), None, now)
            .await;
        service
            .seed_publish_entry(&other, Linear, None, Some("error"), now)
            .await;
        service
            .seed_publish_entry(&slot, Jira, None, None, stuck_before() - 1)
            .await;

        let (found, targets) = service
            .get_slot_targets(SlotTarget::Id(slot.id))
            .await
            .unwrap();

        assert_eq!(found.model, slot);
        let targets: Vec<_> = targets
            .iter()
            .map(|target| (target.connection.name.as_str(), target.state))
            .collect();
        assert_eq!(
            targets,
            [
                (Jira.name(), TargetState::Stuck),
                (Linear.name(), TargetState::Published),
            ]
        );
    }

    #[tokio::test]
    async fn get_slot_targets_rejects_slot_of_other_profile() {
        let (mut service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let slot = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;
        service
            .seed_publish_entry(&slot, Linear, Some(now), None, now)
            .await;
        service.use_profile("work").await;

        let err = service
            .get_slot_targets(SlotTarget::Id(slot.id))
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::SlotNotFound(id)) if id == slot.id
        ));
    }

    #[tokio::test]
    async fn delete_slot_rejects_slot_being_published() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let slot = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;
        service
            .seed_publish_entry(&slot, Linear, None, None, now)
            .await;

        let err = service.delete_slot(slot.id).await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::SlotBeingPublished)
        ));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [slot]);
    }

    #[tokio::test]
    async fn delete_slot_deletes_stuck_slot() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let slot = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;
        service
            .seed_publish_entry(&slot, Linear, None, None, stuck_before() - 1)
            .await;

        service.delete_slot(slot.id).await.unwrap();

        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert!(slots.is_empty());
        let entries = publish_entry::Entity::find()
            .all(&service.db)
            .await
            .unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn delete_slot_deletes_slot() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let deleted = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;
        let kept = service.seed_slot(&ticket, now - 600, None).await;

        service.delete_slot(deleted.id).await.unwrap();

        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [kept]);
    }

    #[tokio::test]
    async fn delete_slot_deletes_running_slot_found_at_time() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let stopped = service
            .seed_slot(&ticket, now - 1200, Some(now - 600))
            .await;
        service.seed_slot(&ticket, now - 600, None).await;

        let (found, _) = service.get_slot(SlotTarget::At(now - 60)).await.unwrap();
        service.delete_slot(found.model.id).await.unwrap();

        assert!(service.running_slot().await.unwrap().is_none());
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [stopped]);
    }

    #[tokio::test]
    async fn delete_slot_deletes_publish_entries() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let deleted = service
            .seed_slot(&ticket, now - 1200, Some(now - 900))
            .await;
        let kept = service.seed_slot(&ticket, now - 600, Some(now - 300)).await;
        service
            .seed_publish_entry(&deleted, Linear, Some(now), None, now)
            .await;
        let kept_entry = service
            .seed_publish_entry(&kept, Linear, Some(now), None, now)
            .await;

        service.delete_slot(deleted.id).await.unwrap();

        let entries = publish_entry::Entity::find()
            .all(&service.db)
            .await
            .unwrap();
        assert_eq!(entries, [kept_entry]);
    }

    #[tokio::test]
    async fn delete_slot_rejects_unknown_slot() {
        let (service, [ticket, _]) = setup().await;
        let now = Utc::now().timestamp();
        let slot = service.seed_slot(&ticket, now - 600, Some(now - 300)).await;

        let err = service.delete_slot(slot.id + 1).await.unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::SlotNotFound(id)) if id == slot.id + 1
        ));
        let slots = slot::Entity::find().all(&service.db).await.unwrap();
        assert_eq!(slots, [slot]);
    }
}
