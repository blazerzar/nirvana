use std::fmt;
use std::str::FromStr;

use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "credentials")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub connection_id: i64,
    pub secret_store: SecretStore,
    #[sea_orm(column_type = "Text")]
    pub credential: String,

    #[sea_orm(belongs_to, from = "connection_id", to = "id", on_delete = "Cascade")]
    pub connection: BelongsTo<super::connection::Entity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum SecretStore {
    #[sea_orm(string_value = "plaintext")]
    Plaintext,
    #[sea_orm(string_value = "keyring")]
    Keyring,
    #[sea_orm(string_value = "env")]
    Env,
}

impl fmt::Display for SecretStore {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.to_value())
    }
}

impl FromStr for SecretStore {
    type Err = DbErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from_value(&s.to_owned())
    }
}

impl ActiveModelBehavior for ActiveModel {}
