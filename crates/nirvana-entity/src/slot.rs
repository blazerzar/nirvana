use sea_orm::{ExprTrait, SelectTwo, entity::prelude::*};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "slots")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub ticket_id: i64,
    pub connection_id: i64,
    #[sea_orm(column_type = "Text", nullable)]
    pub note: Option<String>,
    pub started_at: i64,
    #[sea_orm(unique)]
    pub stopped_at: Option<i64>,
    pub published_at: Option<i64>,
    #[sea_orm(
        belongs_to,
        from = "connection_id",
        to = "id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    pub connection: BelongsTo<super::connection::Entity>,
    #[sea_orm(
        belongs_to,
        from = "ticket_id",
        to = "id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    pub ticket: BelongsTo<super::ticket::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    pub fn find_running() -> Select<Self> {
        Self::find().filter(COLUMN.stopped_at.is_null())
    }

    pub fn find_overlapping(connection_id: i64, from: i64, to: i64) -> Select<Self> {
        Self::find()
            .filter(COLUMN.connection_id.eq(connection_id))
            .filter(COLUMN.started_at.lt(to))
            .filter(COLUMN.stopped_at.is_null().or(COLUMN.stopped_at.gt(from)))
    }

    pub fn find_with_ticket() -> SelectTwo<Self, super::ticket::Entity> {
        Self::find().find_also_related(super::ticket::Entity)
    }
}
