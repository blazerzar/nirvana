use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "tickets")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    #[sea_orm(column_type = "Text")]
    pub ticket_key: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub summary: Option<String>,
    pub connection_id: i64,
    pub last_worked_at: i64,
    #[sea_orm(
        belongs_to,
        from = "connection_id",
        to = "id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    pub connection: BelongsTo<super::connection::Entity>,
    #[sea_orm(has_many)]
    pub slots: HasMany<super::slot::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
