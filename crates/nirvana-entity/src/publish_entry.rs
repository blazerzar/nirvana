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

impl Model {
    /// Returns whether the entry is claimed and has no result yet.
    pub fn is_pending(&self) -> bool {
        self.published_at.is_none() && self.error.is_none()
    }

    /// Returns whether the entry is pending and was claimed before
    /// `stuck_before`.
    pub fn is_stuck(&self, stuck_before: i64) -> bool {
        self.is_pending() && self.claimed_at < stuck_before
    }
}

impl Entity {
    /// Returns a condition matching slots without publish entries.
    pub fn unpublished() -> Expr {
        super::slot::COLUMN.id.not_in_subquery(
            Self::find()
                .select_only()
                .column(Column::SlotId)
                .into_query(),
        )
    }

    /// Returns a condition matching slots without publish entries or with an
    /// entry that is not published.
    pub fn not_published() -> Expr {
        Self::unpublished().or(super::slot::COLUMN.id.in_subquery(
            Self::find()
                .select_only()
                .column(Column::SlotId)
                .filter(COLUMN.published_at.is_null())
                .into_query(),
        ))
    }

    /// Returns a condition matching [pending](Model::is_pending) entries.
    pub fn pending() -> Expr {
        COLUMN.published_at.is_null().and(COLUMN.error.is_null())
    }

    /// Returns a condition matching [stuck](Model::is_stuck) entries.
    pub fn stuck(stuck_before: i64) -> Expr {
        Self::pending().and(COLUMN.claimed_at.lt(stuck_before))
    }

    /// Returns a condition matching entries that failed or are
    /// [stuck](Self::stuck).
    pub fn retryable(stuck_before: i64) -> Expr {
        COLUMN.error.is_not_null().or(Self::stuck(stuck_before))
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
