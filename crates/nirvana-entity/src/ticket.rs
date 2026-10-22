use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "tickets")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub profile_id: i64,
    pub connection_id: Option<i64>,
    #[sea_orm(column_type = "Text")]
    pub ticket_key: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub title: Option<String>,
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
