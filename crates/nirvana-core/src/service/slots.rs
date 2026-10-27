use nirvana_entity::slot::Entity as Slot;
use nirvana_entity::{slot, ticket};
use sea_orm::{DbErr, ExprTrait, QueryFilter, QueryOrder, QueryTrait};

use crate::domain::SlotSort;
use crate::errors::NirvanaError;
use crate::service::Nirvana;

impl Nirvana {
    pub async fn get_running_slot(
        &self,
    ) -> Result<Option<(slot::Model, ticket::Model)>, NirvanaError> {
        let Some(connection) = self.get_active_connection().await? else {
            return Ok(None);
        };

        let running = Slot::find_running(connection.model.profile_id)
            .find_also_related(ticket::Entity)
            .one(&self.db)
            .await?;

        Ok(running.map(require_ticket).transpose()?)
    }

    pub async fn get_slots(
        &self,
        from: i64,
        to: Option<i64>,
        sort: SlotSort,
    ) -> Result<Vec<(slot::Model, ticket::Model)>, NirvanaError> {
        let connection_id = self.active_connection_id().await?;

        let query = Slot::find_with_ticket()
            .filter(ticket::COLUMN.connection_id.eq(connection_id))
            .filter(
                slot::COLUMN
                    .stopped_at
                    .is_null()
                    .or(slot::COLUMN.stopped_at.gt(from)),
            )
            .apply_if(to, |q, to| q.filter(slot::COLUMN.started_at.lt(to)));
        let query = match sort {
            SlotSort::StartedAt => query.order_by_asc(slot::COLUMN.started_at),
            SlotSort::TicketId => query.order_by_asc(slot::COLUMN.ticket_id),
        };

        let records = query.all(&self.db).await?;
        Ok(records
            .into_iter()
            .map(require_ticket)
            .collect::<Result<_, _>>()?)
    }

    pub async fn get_unpublished_slots(
        &self,
        from: i64,
        to: Option<i64>,
    ) -> Result<Vec<(slot::Model, ticket::Model)>, NirvanaError> {
        let connection_id = self.active_connection_id().await?;

        let records = Slot::find_unpublished(connection_id)
            .find_also_related(ticket::Entity)
            .filter(ticket::COLUMN.connection_id.eq(connection_id))
            .filter(slot::COLUMN.started_at.gte(from))
            .apply_if(to, |q, to| q.filter(slot::COLUMN.started_at.lt(to)))
            .filter(slot::COLUMN.stopped_at.is_not_null())
            .order_by_asc(slot::COLUMN.started_at)
            .all(&self.db)
            .await?;
        Ok(records
            .into_iter()
            .map(require_ticket)
            .collect::<Result<_, _>>()?)
    }
}

pub(crate) fn require_ticket(
    (slot, ticket): (slot::Model, Option<ticket::Model>),
) -> Result<(slot::Model, ticket::Model), DbErr> {
    let ticket = ticket.ok_or(DbErr::RecordNotFound(format!(
        "ticket for slot {}",
        slot.id
    )))?;
    Ok((slot, ticket))
}
