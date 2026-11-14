use sea_orm::entity::prelude::*;
use sea_orm::{ExprTrait, QuerySelect, QueryTrait};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "publish_entries")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub slot_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub connection_id: i64,
    pub published_at: Option<i64>,
    #[sea_orm(column_type = "Text")]
    pub error: Option<String>,
    pub claimed_at: i64,

    #[sea_orm(belongs_to, from = "slot_id", to = "id", on_delete = "Cascade")]
    pub slot: BelongsTo<super::slot::Entity>,
    #[sea_orm(belongs_to, from = "connection_id", to = "id", on_delete = "NoAction")]
    pub connection: BelongsTo<super::connection::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    pub fn find_published() -> Select<Self> {
        Self::find().filter(COLUMN.published_at.is_not_null())
    }

    /// Returns a condition matching slots without publish entries.
    pub fn unpublished() -> Expr {
        super::slot::COLUMN.id.not_in_subquery(
            Self::find()
                .select_only()
                .column(Column::SlotId)
                .into_query(),
        )
    }

    /// Returns a condition matching entries that failed, or that are pending
    /// and were claimed before `stuck_before`.
    pub fn retryable(stuck_before: i64) -> Expr {
        COLUMN.error.is_not_null().or(COLUMN
            .published_at
            .is_null()
            .and(COLUMN.claimed_at.lt(stuck_before)))
    }

    /// Returns a condition matching slots with a [retryable](Self::retryable)
    /// publish entry.
    pub fn failed(stuck_before: i64) -> Expr {
        super::slot::COLUMN.id.in_subquery(
            Self::find()
                .select_only()
                .column(Column::SlotId)
                .filter(Self::retryable(stuck_before))
                .into_query(),
        )
    }
}
