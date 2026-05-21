use crate::api::NirvanaApi;
use crate::api::NirvanaError;
use crate::api::domain::Slot;
use crate::api::errors::TrackingError;
use crate::integration;
use crate::storage::slot_repo::{self, SlotSort};

impl NirvanaApi {
    pub fn get_slots(
        &self,
        from: i64,
        to: Option<i64>,
        sort: SlotSort,
    ) -> Result<Vec<Slot>, NirvanaError> {
        let connection_id = self
            .config
            .active_connection
            .ok_or(TrackingError::NoActiveConnection)?;

        let connection = self.get_connection(connection_id)?;
        let integ = integration::Integration::build_for_url(&connection)?;

        let records = slot_repo::get_slots(&self.db, connection_id, from, to, sort)?;
        Ok(records
            .into_iter()
            .map(|r| {
                let issue_url = Some(integ.get_issue_link(&r.ticket_key));
                Slot::from_record(r, issue_url)
            })
            .collect())
    }

    pub fn get_unpublished_slots(
        &self,
        from: i64,
        to: Option<i64>,
    ) -> Result<Vec<Slot>, NirvanaError> {
        let connection_id = self
            .config
            .active_connection
            .ok_or(TrackingError::NoActiveConnection)?;

        let connection = self.get_connection(connection_id)?;
        let integ = integration::Integration::build_for_url(&connection)?;

        let records = slot_repo::get_unpublished(&self.db, connection_id, from, to)?;
        Ok(records
            .into_iter()
            .map(|r| {
                let issue_url = Some(integ.get_issue_link(&r.ticket_key));
                Slot::from_record(r, issue_url)
            })
            .collect())
    }
}
