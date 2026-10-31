use nirvana_entity::publish_entry::Entity as PublishEntry;
use nirvana_entity::slot::Entity as Slot;
use nirvana_entity::{publish_entry, slot, ticket};
use sea_orm::ActiveValue::{NotSet, Set, Unchanged};
use sea_orm::{ActiveModelTrait, ActiveValue, PaginatorTrait, QueryFilter, Value};

use crate::domain::{Change, SlotEdit};
use crate::errors::{DomainError, NirvanaError};
use crate::service::Nirvana;
use crate::service::slots::require_ticket;

impl Nirvana {
    pub async fn edit_slot(
        &self,
        slot_id: i64,
        edit: SlotEdit,
    ) -> Result<(slot::Model, ticket::Model), NirvanaError> {
        let (current, ticket) = require_ticket(
            Slot::find_with_ticket()
                .filter(slot::COLUMN.id.eq(slot_id))
                .one(&self.db)
                .await?
                .ok_or(DomainError::SlotNotFound(slot_id))?,
        )?;

        let published = PublishEntry::find_published()
            .filter(publish_entry::COLUMN.slot_id.eq(slot_id))
            .count(&self.db)
            .await?;
        if published > 0 {
            return Err(DomainError::SlotAlreadyPublished.into());
        }

        let started_changed = match edit.started_at {
            None => false,
            Some(v) => v != current.started_at,
        };

        let stopped_changed = match &edit.stopped_at {
            Change::Skip => false,
            Change::Clear => current.stopped_at.is_some(),
            Change::Set(v) => current.stopped_at != Some(*v),
        };

        let normalized_ticket_key = edit.ticket_key.as_ref().and_then(|ticket_key| {
            let ticket_key = ticket_key.trim().to_uppercase();
            (!ticket_key.is_empty()).then_some(ticket_key)
        });
        let ticket_changed = normalized_ticket_key
            .as_ref()
            .is_some_and(|ticket_key| ticket_key != &ticket.ticket_key);

        if !ticket_changed && !started_changed && !stopped_changed {
            return Ok((current, ticket));
        }

        let effective_started = edit.started_at.unwrap_or(current.started_at);
        let effective_stopped = match &edit.stopped_at {
            Change::Set(v) => Some(*v),
            Change::Clear => None,
            Change::Skip => current.stopped_at,
        };
        if let Some(stopped) = effective_stopped
            && stopped <= effective_started
        {
            return Err(DomainError::InvalidTimeRange.into());
        }

        let next_ticket = if ticket_changed {
            let connection_id = ticket
                .connection_id
                .ok_or_else(|| DomainError::TicketHasNoConnection(ticket.ticket_key.clone()))?;
            let connection = self.get_connection(connection_id).await?;
            let ticket_key = normalized_ticket_key.as_deref().unwrap_or_default();
            let last_worked_at =
                effective_stopped.unwrap_or_else(|| chrono::Utc::now().timestamp());
            Some(
                self.resolve_ticket_for_work(
                    &connection,
                    connection_id,
                    ticket_key,
                    last_worked_at,
                )
                .await?,
            )
        } else {
            None
        };

        let update = slot::ActiveModel {
            id: Unchanged(slot_id),
            ticket_id: next_ticket.as_ref().map_or(NotSet, |t| Set(t.id)),
            started_at: match edit.started_at {
                Some(v) if started_changed => Set(v),
                _ => NotSet,
            },
            stopped_at: if stopped_changed {
                active_value(edit.stopped_at)
            } else {
                NotSet
            },
            ..Default::default()
        };
        let updated = update.update(&self.db).await?;

        Ok((updated, next_ticket.unwrap_or(ticket)))
    }
}

fn active_value<T>(change: Change<T>) -> ActiveValue<Option<T>>
where
    Option<T>: Into<Value>,
{
    match change {
        Change::Skip => NotSet,
        Change::Clear => Set(None),
        Change::Set(v) => Set(Some(v)),
    }
}
