use nirvana_entity::slot::Entity as Slot;
use nirvana_entity::{slot, ticket};
use sea_orm::{EntityTrait, QueryFilter};

use crate::errors::{NirvanaError, TrackingError};
use crate::service::Nirvana;
use crate::service::slots::require_ticket;

impl Nirvana {
    pub async fn delete_slot(
        &self,
        slot_id: i64,
    ) -> Result<(slot::Model, ticket::Model), NirvanaError> {
        let (slot, ticket) = require_ticket(
            Slot::find_with_ticket()
                .filter(slot::COLUMN.id.eq(slot_id))
                .one(&self.db)
                .await?
                .ok_or(TrackingError::SlotNotFound(slot_id))?,
        )?;

        let deleted = Slot::delete_by_id(slot_id).exec(&self.db).await?;
        if deleted.rows_affected == 0 {
            return Err(TrackingError::SlotNotFound(slot_id).into());
        }

        Ok((slot, ticket))
    }
}
