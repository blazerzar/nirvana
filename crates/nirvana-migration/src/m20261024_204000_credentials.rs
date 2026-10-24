use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;
use tracing::debug;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261024_204000_credentials"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    /// Rebuilds `credentials` with a NOT NULL `secret_store` column copied from
    /// `connections`, following https://sqlite.org/lang_altertable.html#otheralter,
    /// then drops `connections.secret_store`.
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let stmt = Table::create()
            .table("credentials_new")
            .col(integer("connection_id"))
            .col(text("secret_store"))
            .col(text("credential"))
            .primary_key(Index::create().col("connection_id"))
            .foreign_key(
                ForeignKey::create()
                    .from("credentials_new", "connection_id")
                    .to("connections", "id")
                    .on_delete(ForeignKeyAction::Cascade),
            )
            .check(Expr::col("secret_store").is_in(["plaintext", "keyring", "env"]))
            .to_owned();
        manager.create_table(stmt).await?;
        debug!("created credentials_new table");

        let select = Query::select()
            .column(("credentials", "connection_id"))
            .column(("connections", "secret_store"))
            .column(("credentials", "credential"))
            .from("credentials")
            .inner_join(
                "connections",
                Expr::col(("connections", "id")).equals(("credentials", "connection_id")),
            )
            .to_owned();
        let stmt = Query::insert()
            .into_table("credentials_new")
            .columns(["connection_id", "secret_store", "credential"])
            .select_from(select)
            .map_err(|e| DbErr::Migration(e.to_string()))?
            .to_owned();
        manager.exec_stmt(stmt).await?;
        debug!("copied credentials with secret_store from connections to credentials_new");

        let stmt = Table::drop().table("credentials").to_owned();
        manager.drop_table(stmt).await?;
        debug!("dropped table credentials");

        let stmt = Table::rename()
            .table("credentials_new", "credentials")
            .to_owned();
        manager.rename_table(stmt).await?;
        debug!("renamed credentials_new table to credentials");

        let stmt = Table::alter()
            .table("connections")
            .drop_column("secret_store")
            .to_owned();
        manager.alter_table(stmt).await?;
        debug!("removed secret_store column from connections");

        Ok(())
    }
}
