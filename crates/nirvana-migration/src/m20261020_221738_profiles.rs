use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;
use sea_orm_migration::sea_orm::raw_sql;
use tracing::debug;

const DEFAULT_PROFILE: &str = "default";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// `PRAGMA foreign_keys` is ignored inside a transaction, so the
    /// migration opens its own after turning foreign keys off.
    fn use_transaction(&self) -> Option<bool> {
        Some(false)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;
        let result = add_profiles(manager).await;
        let restored = db.execute_unprepared("PRAGMA foreign_keys = ON").await;
        result?;
        restored?;
        Ok(())
    }
}

/// Rebuilds `connections` and `tickets` with a `profile_id` column, following
/// https://sqlite.org/lang_altertable.html#otheralter. With foreign keys on,
/// dropping `connections` would cascade into `credentials`.
async fn add_profiles(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let txn = manager.begin().await?;
    let db = txn.get_connection();

    txn.create_table(
        Table::create()
            .table("profiles")
            .col(ColumnDef::new("id").integer().primary_key())
            .col(text("name").unique_key())
            .col(integer("created_at"))
            .col(integer("updated_at"))
            .to_owned(),
    )
    .await?;
    debug!("created profiles table");

    txn.create_table(
        Table::create()
            .table("connections_new")
            .col(ColumnDef::new("id").integer().primary_key())
            .col(text("name"))
            .col(text("kind"))
            .col(text("host"))
            .col(text("identity"))
            .col(text("secret_store").default("keyring"))
            .col(integer("created_at"))
            .col(integer("updated_at"))
            .col(integer("profile_id"))
            .foreign_key(
                ForeignKey::create()
                    .from("connections_new", "profile_id")
                    .to("profiles", "id")
                    .on_delete(ForeignKeyAction::Cascade),
            )
            .to_owned(),
    )
    .await?;
    debug!("created connections_new table");

    txn.create_table(
        Table::create()
            .table("tickets_new")
            .col(ColumnDef::new("id").integer().primary_key())
            .col(integer("profile_id"))
            .col(integer_null("connection_id"))
            .col(text("ticket_key"))
            .col(text_null("title"))
            .col(integer("last_worked_at").default(0))
            .col(integer("synced_at"))
            .foreign_key(
                ForeignKey::create()
                    .from("tickets_new", "profile_id")
                    .to("profiles", "id")
                    .on_delete(ForeignKeyAction::Cascade),
            )
            .foreign_key(
                ForeignKey::create()
                    .from("tickets_new", "connection_id")
                    .to("connections", "id"),
            )
            .to_owned(),
    )
    .await?;
    debug!("created tickets_new table");

    let has_connections = db
        .query_one(Query::select().column("id").from("connections").limit(1))
        .await?
        .is_some();
    if has_connections {
        let name = DEFAULT_PROFILE;
        let profile_id = db
            .execute(
                Query::insert()
                    .into_table("profiles")
                    .columns(["name", "created_at", "updated_at"])
                    .values_panic([
                        name.into(),
                        Expr::cust("unixepoch()"),
                        Expr::cust("unixepoch()"),
                    ]),
            )
            .await?
            .last_insert_id();
        debug!("created profile {name}");

        let columns = [
            "id",
            "name",
            "kind",
            "host",
            "identity",
            "secret_store",
            "created_at",
            "updated_at",
        ];
        let assigned = db
            .execute(
                Query::insert()
                    .into_table("connections_new")
                    .columns(columns.into_iter().chain(["profile_id"]))
                    .select_from(
                        Query::select()
                            .columns(columns)
                            .expr(Expr::val(profile_id))
                            .from("connections")
                            .to_owned(),
                    )
                    .map_err(|e| DbErr::Migration(e.to_string()))?,
            )
            .await?
            .rows_affected();
        debug!("assigned {assigned} connections to profile {name}");

        let assigned = db
            .execute(
                Query::insert()
                    .into_table("tickets_new")
                    .columns([
                        "id",
                        "ticket_key",
                        "title",
                        "connection_id",
                        "last_worked_at",
                        "synced_at",
                        "profile_id",
                    ])
                    .select_from(
                        Query::select()
                            .columns([
                                "id",
                                "ticket_key",
                                "summary",
                                "connection_id",
                                "last_worked_at",
                                "last_worked_at",
                            ])
                            .expr(Expr::val(profile_id))
                            .from("tickets")
                            .to_owned(),
                    )
                    .map_err(|e| DbErr::Migration(e.to_string()))?,
            )
            .await?
            .rows_affected();
        debug!("assigned {assigned} tickets to profile {name}");
    }

    txn.drop_table(Table::drop().table("connections").to_owned())
        .await?;
    debug!("dropped table connections");

    txn.rename_table(
        Table::rename()
            .table("connections_new", "connections")
            .to_owned(),
    )
    .await?;
    debug!("renamed connections_new table to connections");

    txn.drop_table(Table::drop().table("tickets").to_owned())
        .await?;
    debug!("dropped table tickets");

    txn.rename_table(Table::rename().table("tickets_new", "tickets").to_owned())
        .await?;
    debug!("renamed tickets_new table to tickets");

    txn.create_index(
        Index::create()
            .name("connections_by_profile_name")
            .table("connections")
            .col("profile_id")
            .col("name")
            .unique()
            .to_owned(),
    )
    .await?;

    txn.create_index(
        Index::create()
            .name("tickets_by_profile_key")
            .table("tickets")
            .col("profile_id")
            .col("ticket_key")
            .to_owned(),
    )
    .await?;

    txn.create_index(
        Index::create()
            .name("tickets_by_last_worked")
            .table("tickets")
            .col("profile_id")
            .col(("last_worked_at", IndexOrder::Desc))
            .to_owned(),
    )
    .await?;

    let violations = db
        .query_all_raw(raw_sql!(Sqlite, "PRAGMA foreign_key_check"))
        .await?;
    if !violations.is_empty() {
        return Err(DbErr::Migration(format!(
            "{} foreign key violations after adding profiles to connections and tickets",
            violations.len()
        )));
    }

    txn.commit().await?;
    Ok(())
}
