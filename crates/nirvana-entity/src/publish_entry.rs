use sea_orm::entity::prelude::*;

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

    #[sea_orm(belongs_to, from = "slot_id", to = "id", on_delete = "Cascade")]
    pub slot: BelongsTo<super::slot::Entity>,
    #[sea_orm(belongs_to, from = "connection_id", to = "id")]
    pub connection: BelongsTo<super::connection::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    pub fn find_published() -> Select<Self> {
        Self::find().filter(COLUMN.published_at.is_not_null())
    }
}
