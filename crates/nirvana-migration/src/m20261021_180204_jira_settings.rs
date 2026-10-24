use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;
use tracing::debug;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let stmt = Table::create()
            .table("jira_settings")
            .col(integer("connection_id"))
            .col(boolean("publish").default(true))
            .col(boolean("squash").default(false))
            .primary_key(Index::create().col("connection_id"))
            .foreign_key(
                ForeignKey::create()
                    .from("jira_settings", "connection_id")
                    .to("connections", "id")
                    .on_delete(ForeignKeyAction::Cascade),
            )
            .to_owned();
        manager.create_table(stmt).await?;
        debug!("created jira_settings table");

        let select = Query::select()
            .column("id")
            .expr(Expr::val(true))
            .expr(Expr::val(true))
            .from("connections")
            .and_where(Expr::col("kind").is_in(["jira-cloud", "jira-dc"]))
            .to_owned();
        let stmt = Query::insert()
            .into_table("jira_settings")
            .columns(["connection_id", "publish", "squash"])
            .select_from(select)
            .map_err(|e| DbErr::Migration(e.to_string()))?
            .to_owned();
        manager.exec_stmt(stmt).await?;

        Ok(())
    }
}
