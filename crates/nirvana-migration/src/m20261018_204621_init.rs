use crate::user_version;
use sea_orm_migration::{prelude::*, schema::*};
use tracing::info;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if user_version(manager).await? >= 1 {
            info!("legacy migration init applied from 0.3, skipping");
            return Ok(());
        }

        manager
            .create_table(
                Table::create()
                    .table("connections")
                    .col(ColumnDef::new("id").integer().primary_key())
                    .col(text("name"))
                    .col(text("kind"))
                    .col(text("host"))
                    .col(text("identity"))
                    .col(text("secret_store").default("keyring"))
                    .col(integer("created_at"))
                    .col(integer("updated_at"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("connections_by_name")
                    .table("connections")
                    .col("name")
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table("credentials")
                    .col(integer("connection_id"))
                    .col(text("credential"))
                    .primary_key(Index::create().col("connection_id"))
                    .foreign_key(
                        ForeignKey::create()
                            .from("credentials", "connection_id")
                            .to("connections", "id")
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("credentials").to_owned())
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("connections_by_name")
                    .table("connections")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table("connections").to_owned())
            .await
    }
}
