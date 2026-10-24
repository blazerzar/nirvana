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

    /// Rebuilds `slots` with NOT NULL `uuid` and `profile_id` columns, following
    /// https://sqlite.org/lang_altertable.html#otheralter, and drops `note`.
    /// `profile_id` comes from the slot's ticket and replaces `connection_id`,
    /// which must match the ticket's. Moves `published_at` into a new
    /// `publish_entries` table with one row per published slot, then drops it
    /// with the `slots_unpublished` index. Allows one running slot per profile.
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let stmt = Query::select()
            .column(("slots", "id"))
            .from("slots")
            .inner_join(
                "tickets",
                Expr::col(("tickets", "id")).equals(("slots", "ticket_id")),
            )
            .and_where(
                Expr::col(("slots", "connection_id"))
                    .is_not(Expr::col(("tickets", "connection_id"))),
            )
            .limit(1)
            .to_owned();
        if let Some(row) = db.query_one(&stmt).await? {
            let id: i64 = row.try_get_by_index(0)?;
            return Err(DbErr::Migration(format!(
                "slot {id} has a different connection than its ticket"
            )));
        }

        let stmt = Table::create()
            .table("slots_new")
            .col(ColumnDef::new("id").integer().primary_key())
            .col(integer("ticket_id"))
            .col(integer("profile_id"))
            .col(integer("started_at"))
            .col(integer_null("stopped_at"))
            .col(blob("uuid"))
            .foreign_key(
                ForeignKey::create()
                    .from("slots_new", "ticket_id")
                    .to("tickets", "id"),
            )
            .foreign_key(
                ForeignKey::create()
                    .from("slots_new", "profile_id")
                    .to("profiles", "id")
                    .on_delete(ForeignKeyAction::Cascade),
            )
            .check(
                Expr::col("stopped_at")
                    .is_null()
                    .or(Expr::col("stopped_at").gt(Expr::col("started_at"))),
            )
            .to_owned();
        manager.create_table(stmt).await?;
        debug!("created slots_new table");

        let stmt = Table::create()
            .table("publish_entries")
            .col(integer("slot_id"))
            .col(integer("connection_id"))
            .col(integer_null("published_at"))
            .col(text_null("error"))
            .primary_key(Index::create().col("slot_id").col("connection_id"))
            .foreign_key(
                ForeignKey::create()
                    .from("publish_entries", "slot_id")
                    .to("slots_new", "id")
                    .on_delete(ForeignKeyAction::Cascade),
            )
            .foreign_key(
                ForeignKey::create()
                    .from("publish_entries", "connection_id")
                    .to("connections", "id"),
            )
            .to_owned();
        manager.create_table(stmt).await?;
        debug!("created publish_entries table");

        let columns = ["id", "ticket_id", "started_at", "stopped_at"];
        let stmt = Query::select()
            .columns(columns.map(|c| ("slots", c)))
            .column(("tickets", "profile_id"))
            .from("slots")
            .inner_join(
                "tickets",
                Expr::col(("slots", "ticket_id")).equals(("tickets", "id")),
            )
            .to_owned();
        let rows = db.query_all(&stmt).await?;
        for chunk in rows.chunks(1000) {
            let mut stmt = Query::insert()
                .into_table("slots_new")
                .columns(columns.into_iter().chain(["profile_id", "uuid"]))
                .to_owned();
            for row in chunk {
                let (id, ticket_id, started_at, stopped_at, profile_id): (
                    i64,
                    i64,
                    i64,
                    Option<i64>,
                    i64,
                ) = row.try_get_many_by_index()?;
                stmt.values_panic([
                    id.into(),
                    ticket_id.into(),
                    started_at.into(),
                    stopped_at.into(),
                    profile_id.into(),
                    Uuid::new_v4().as_bytes().to_vec().into(),
                ]);
            }
            db.execute(&stmt).await?;
        }
        debug!("copied {} slots to slots_new with uuids", rows.len());

        let select = Query::select()
            .columns(["id", "connection_id", "published_at"])
            .from("slots")
            .and_where(Expr::col("published_at").is_not_null())
            .to_owned();
        let stmt = Query::insert()
            .into_table("publish_entries")
            .columns(["slot_id", "connection_id", "published_at"])
            .select_from(select)
            .map_err(|e| DbErr::Migration(e.to_string()))?
            .to_owned();
        let res = db.execute(&stmt).await?;
        debug!("created {} publish_entries entries", res.rows_affected());

        let stmt = Table::drop().table("slots").to_owned();
        manager.drop_table(stmt).await?;
        debug!("dropped table slots");

        let stmt = Table::rename().table("slots_new", "slots").to_owned();
        manager.rename_table(stmt).await?;
        debug!("renamed slots_new table to slots");

        let stmt = Index::create()
            .name("one_running_slot")
            .table("slots")
            .col("profile_id")
            .and_where(Expr::col("stopped_at").is_null())
            .unique()
            .to_owned();
        manager.create_index(stmt).await?;
        debug!("created index one_running_slot");

        let stmt = Index::create()
            .name("slots_by_ticket")
            .table("slots")
            .col("ticket_id")
            .col("started_at")
            .to_owned();
        manager.create_index(stmt).await?;
        debug!("created index slots_by_ticket");

        let stmt = Index::create()
            .name("slots_by_uuid")
            .table("slots")
            .col("uuid")
            .unique()
            .to_owned();
        manager.create_index(stmt).await?;
        debug!("created index slots_by_uuid");

        Ok(())
    }
}
