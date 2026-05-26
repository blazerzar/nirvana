use crate::api::NirvanaApi;
use crate::api::NirvanaError;
use crate::api::domain::{Slot, SlotEdit};
use crate::api::errors::TrackingError;
use crate::integration;
use crate::storage::slot_repo;
use crate::storage::slot_repo::Change;

impl NirvanaApi {
    pub fn edit_slot(&self, slot_id: i64, edit: SlotEdit) -> Result<Slot, NirvanaError> {
        let current = slot_repo::find_by_id_with_ticket(&self.db, slot_id)?
            .ok_or(TrackingError::SlotNotFound(slot_id))?;

        if current.published_at.is_some() {
            return Err(TrackingError::SlotAlreadyPublished.into());
        }

        let note_changed = match &edit.note {
            Change::Skip => false,
            Change::Clear => current.note.is_some(),
            Change::Set(v) => current.note.as_ref() != Some(v),
        };

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
            .is_some_and(|ticket_key| ticket_key != &current.ticket_key);

        if !ticket_changed && !note_changed && !started_changed && !stopped_changed {
            let connection = self.get_connection(current.connection_id)?;
            let integ = integration::Integration::build_for_url(&connection)?;
            let issue_url = Some(integ.get_issue_link(&current.ticket_key));
            return Ok(Slot::from_record(current, issue_url));
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
            return Err(TrackingError::InvalidTimeRange.into());
        }

        let connection = self.get_connection(current.connection_id)?;
        let next_ticket_id = if ticket_changed {
            let ticket_key = normalized_ticket_key.as_deref().unwrap_or_default();
            let last_worked_at =
                effective_stopped.unwrap_or_else(|| chrono::Utc::now().timestamp());
            Some(
                self.resolve_ticket_for_work(
                    &connection,
                    current.connection_id,
                    ticket_key,
                    last_worked_at,
                )?
                .id,
            )
        } else {
            None
        };

        let update = slot_repo::SlotUpdate {
            ticket_id: next_ticket_id,
            note: if note_changed {
                edit.note
            } else {
                Change::Skip
            },
            started_at: if started_changed {
                edit.started_at
            } else {
                None
            },
            stopped_at: if stopped_changed {
                edit.stopped_at
            } else {
                Change::Skip
            },
        };
        slot_repo::update(&self.db, slot_id, &update)?;

        let updated = slot_repo::find_by_id_with_ticket(&self.db, slot_id)?
            .ok_or(TrackingError::SlotNotFound(slot_id))?;

        let integ = integration::Integration::build_for_url(&connection)?;
        let issue_url = Some(integ.get_issue_link(&updated.ticket_key));
        Ok(Slot::from_record(updated, issue_url))
    }
}
