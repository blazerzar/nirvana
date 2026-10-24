use sea_orm::Set;
use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "connections")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    #[sea_orm(column_type = "Text")]
    pub name: String,
    #[sea_orm(column_type = "Text")]
    pub kind: String,
    #[sea_orm(column_type = "Text")]
    pub host: String,
    #[sea_orm(column_type = "Text")]
    pub identity: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub profile_id: i64,

    #[sea_orm(belongs_to, from = "profile_id", to = "id", on_delete = "Cascade")]
    pub profile: BelongsTo<super::profile::Entity>,
    #[sea_orm(has_one)]
    pub credential: HasOne<super::credential::Entity>,
    #[sea_orm(has_one)]
    pub jira_settings: HasOne<super::jira_settings::Entity>,
    #[sea_orm(has_many)]
    pub tickets: HasMany<super::ticket::Entity>,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if let sea_orm::ActiveValue::Set(ref name) = self.name
            && name.trim().is_empty()
        {
            return Err(DbErr::Custom("name must not be empty".into()));
        }

        let now = chrono::Utc::now().timestamp();
        if insert {
            self.created_at = Set(now);
        }
        self.updated_at = Set(now);

        Ok(self)
    }
}
