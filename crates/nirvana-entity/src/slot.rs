use sea_orm::entity::prelude::*;
use sea_orm::{ExprTrait, QueryTrait, SelectTwo, Set};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "slots")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub ticket_id: i64,
    pub connection_id: i64,
    pub started_at: i64,
    #[sea_orm(unique)]
    pub stopped_at: Option<i64>,
    #[sea_orm(unique)]
    pub uuid: Uuid,

    #[sea_orm(belongs_to, from = "connection_id", to = "id", on_delete = "NoAction")]
    pub connection: BelongsTo<super::connection::Entity>,
    #[sea_orm(belongs_to, from = "ticket_id", to = "id", on_delete = "NoAction")]
    pub ticket: BelongsTo<super::ticket::Entity>,
    #[sea_orm(has_many)]
    pub publish_entries: HasMany<super::publish_entry::Entity>,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert && self.uuid.is_set() {
            return Err(DbErr::Custom("uuid must not change".into()));
        }
        if insert && self.uuid.is_not_set() {
            self.uuid = Set(Uuid::new_v4());
        }

        Ok(self)
    }
}

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

    pub fn find_unpublished(connection_id: i64) -> Select<Self> {
        use super::publish_entry;

        Self::find().filter(
            Expr::exists(
                publish_entry::Entity::find_published()
                    .filter(publish_entry::COLUMN.connection_id.eq(connection_id))
                    .filter(
                        Expr::col((publish_entry::Entity, publish_entry::Column::SlotId))
                            .equals((Entity, Column::Id)),
                    )
                    .into_query(),
            )
            .not(),
        )
    }

    pub fn find_with_ticket() -> SelectTwo<Self, super::ticket::Entity> {
        Self::find().find_also_related(super::ticket::Entity)
    }
}
