use sea_orm::IntoSimpleExpr;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::{Expr, ExprTrait, Func};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "tickets")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub profile_id: i64,
    #[sea_orm(unique_key = "key_in_connection")]
    pub connection_id: Option<i64>,
    #[sea_orm(column_type = "Text", unique_key = "key_in_connection")]
    pub ticket_key: String,
    #[sea_orm(column_type = "Text")]
    pub title: Option<String>,
    #[sea_orm(default_value = 0)]
    pub last_worked_at: i64,
    pub synced_at: i64,

    #[sea_orm(belongs_to, from = "profile_id", to = "id", on_delete = "Cascade")]
    pub profile: BelongsTo<super::profile::Entity>,
    #[sea_orm(belongs_to, from = "connection_id", to = "id", on_delete = "NoAction")]
    pub connection: BelongsTo<Option<super::connection::Entity>>,
    #[sea_orm(has_many)]
    pub slots: HasMany<super::slot::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    pub fn find_by_profile_key(profile_id: i64, key: &str) -> Select<Self> {
        Self::find()
            .filter(COLUMN.profile_id.eq(profile_id))
            .filter(
                Expr::expr(Func::lower(COLUMN.ticket_key.into_simple_expr()))
                    .eq(key.to_lowercase()),
            )
    }
}
