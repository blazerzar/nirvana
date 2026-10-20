use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;
use tracing::info;

use crate::user_version;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if user_version(manager).await? >= 2 {
            info!("legacy migration tickets_slots applied from 0.3, skipping");
            return Ok(());
        }

        manager
            .create_table(
                Table::create()
                    .table("tickets")
                    .col(ColumnDef::new("id").integer().primary_key())
                    .col(text("ticket_key"))
                    .col(text_null("summary"))
                    .col(integer("connection_id"))
                    .col(integer("last_worked_at").default(0))
                    .foreign_key(
                        ForeignKey::create()
                            .from("tickets", "connection_id")
                            .to("connections", "id"),
                    )
                    .index(
                        Index::create()
                            .col("ticket_key")
                            .col("connection_id")
                            .unique(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("tickets_by_key")
                    .table("tickets")
                    .col("ticket_key")
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("tickets_by_last_worked")
                    .table("tickets")
                    .col("connection_id")
                    .col(("last_worked_at", IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table("slots")
                    .col(ColumnDef::new("id").integer().primary_key())
                    .col(integer("ticket_id"))
                    .col(integer("connection_id"))
                    .col(text_null("note"))
                    .col(integer("started_at"))
                    .col(integer_null("stopped_at"))
                    .col(integer_null("published_at"))
                    .foreign_key(
                        ForeignKey::create()
                            .from("slots", "ticket_id")
                            .to("tickets", "id"),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from("slots", "connection_id")
                            .to("connections", "id"),
                    )
                    .check(
                        Expr::col("stopped_at")
                            .is_null()
                            .or(Expr::col("stopped_at").gt(Expr::col("started_at"))),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("one_running_slot")
                    .table("slots")
                    .col("stopped_at")
                    .and_where(Expr::col("stopped_at").is_null())
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("slots_by_ticket")
                    .table("slots")
                    .col("ticket_id")
                    .col("started_at")
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("slots_unpublished")
                    .table("slots")
                    .col("published_at")
                    .col("started_at")
                    .and_where(Expr::col("published_at").is_null())
                    .to_owned(),
            )
            .await
    }
}
