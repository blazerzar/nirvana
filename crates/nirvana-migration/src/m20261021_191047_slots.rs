use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;
use tracing::debug;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    /// Rebuilds `slots` with a NOT NULL `uuid` column, following
    /// https://sqlite.org/lang_altertable.html#otheralter, and drops `note`.
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        manager
            .create_table(
                Table::create()
                    .table("slots_new")
                    .col(ColumnDef::new("id").integer().primary_key())
                    .col(integer("ticket_id"))
                    .col(integer("connection_id"))
                    .col(integer("started_at"))
                    .col(integer_null("stopped_at"))
                    .col(integer_null("published_at"))
                    .col(blob("uuid"))
                    .foreign_key(
                        ForeignKey::create()
                            .from("slots_new", "ticket_id")
                            .to("tickets", "id"),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from("slots_new", "connection_id")
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
        debug!("created slots_new table");

        let columns = [
            "id",
            "ticket_id",
            "connection_id",
            "started_at",
            "stopped_at",
            "published_at",
        ];
        let ids = db
            .query_all(Query::select().column("id").from("slots"))
            .await?;
        for row in &ids {
            let id: i64 = row.try_get_by_index(0)?;
            db.execute(
                Query::insert()
                    .into_table("slots_new")
                    .columns(columns.into_iter().chain(["uuid"]))
                    .select_from(
                        Query::select()
                            .columns(columns)
                            .expr(Expr::val(Uuid::new_v4().as_bytes().to_vec()))
                            .from("slots")
                            .and_where(Expr::col("id").eq(id))
                            .to_owned(),
                    )
                    .map_err(|e| DbErr::Migration(e.to_string()))?,
            )
            .await?;
        }
        debug!("copied {} slots to slots_new with uuids", ids.len());

        manager
            .drop_table(Table::drop().table("slots").to_owned())
            .await?;
        debug!("dropped table slots");

        manager
            .rename_table(Table::rename().table("slots_new", "slots").to_owned())
            .await?;
        debug!("renamed slots_new table to slots");

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
        debug!("created index one_running_slot");

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
        debug!("created index slots_by_ticket");

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
            .await?;
        debug!("created index slots_unpublished");

        manager
            .create_index(
                Index::create()
                    .name("slots_by_uuid")
                    .table("slots")
                    .col("uuid")
                    .unique()
                    .to_owned(),
            )
            .await?;
        debug!("created index slots_by_uuid");

        Ok(())
    }
}
