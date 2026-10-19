use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "credentials")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub connection_id: i64,
    #[sea_orm(column_type = "Text")]
    pub credential: String,
    #[sea_orm(
        belongs_to,
        from = "connection_id",
        to = "id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    pub connection: BelongsTo<super::connection::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
