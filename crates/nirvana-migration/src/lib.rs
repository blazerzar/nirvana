pub use sea_orm_migration::prelude::*;

mod m20261018_204621_init;
mod m20261018_204643_tickets_slots;
mod m20261020_221738_profiles;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20261018_204621_init::Migration),
            Box::new(m20261018_204643_tickets_slots::Migration),
            Box::new(m20261020_221738_profiles::Migration),
        ]
    }
}

async fn user_version(manager: &SchemaManager<'_>) -> Result<i32, DbErr> {
    let db = manager.get_connection();
    let row = db
        .query_one_raw(sea_orm_migration::sea_orm::raw_sql!(
            Sqlite,
            "pragma user_version"
        ))
        .await?
        .ok_or_else(|| DbErr::Custom("pragma user_version returned no row".into()))?;

    let version: i32 = row.try_get_by_index(0)?;
    Ok(version)
}
