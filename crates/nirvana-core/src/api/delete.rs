use crate::api::NirvanaApi;
use crate::api::NirvanaError;
use crate::api::domain::Slot;
use crate::api::errors::TrackingError;
use crate::integration;
use crate::storage::slot_repo;

impl NirvanaApi {
    pub fn delete_slot(&self, slot_id: i64) -> Result<Slot, NirvanaError> {
        let current = slot_repo::find_by_id_with_ticket(&self.db, slot_id)?
            .ok_or(TrackingError::SlotNotFound(slot_id))?;

        if current.published_at.is_some() {
            return Err(TrackingError::CannotDeletePublished.into());
        }

        let connection = self.get_connection(current.connection_id)?;
        let integ = integration::Integration::build_for_url(&connection)?;
        let issue_url = Some(integ.get_issue_link(&current.ticket_key));
        let slot = Slot::from_record(current, issue_url);

        let deleted = slot_repo::delete(&self.db, slot_id)?;
        if !deleted {
            return Err(TrackingError::SlotNotFound(slot_id).into());
        }

        Ok(slot)
    }
}
