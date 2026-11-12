use std::sync::Arc;

use thiserror::Error;

use crate::config::{ConfigError, KEYS};
use crate::domain::{Kind, SecretStore};
use crate::integration::IntegrationError;
use crate::secrets::SecretError;
use crate::storage::DbError;

#[derive(Debug, Error)]
pub enum NirvanaError {
    #[error("config error")]
    Config(#[from] ConfigError),
    #[error("config unavailable")]
    ConfigUnavailable(#[from] Arc<ConfigError>),
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error(transparent)]
    Integration(#[from] IntegrationError),
    #[error(transparent)]
    Secret(#[from] SecretError),
}

#[derive(Debug, Error)]
pub enum DomainError {
    // profiles
    #[error("no active profile")]
    NoActiveProfile,
    #[error("profile '{0}' not found")]
    ProfileNotFound(String),
    #[error("profile '{0}' already exists")]
    ProfileExists(String),
    #[error("profile name must not be empty")]
    EmptyProfileName,

    // connections
    #[error("connection '{0}' already exists")]
    ConnectionExists(String),
    #[error("connection name must not be empty")]
    EmptyConnectionName,
    #[error("{0} connection requires an identity")]
    MissingIdentity(Kind),
    #[error("{0} connection does not use an identity")]
    UnexpectedIdentity(Kind),
    #[error("connection '{0}' not found")]
    ConnectionNotFound(String),
    #[error("connection with ID {0} not found")]
    ConnectionIdNotFound(i64),
    #[error("unsupported connection kind: {0}")]
    UnsupportedConnectionKind(String),
    #[error("connection {0} has no settings")]
    MissingConnectionSettings(i64),
    #[error("{0} connection does not have these settings")]
    UnexpectedSettings(Kind),
    #[error("invalid host: {0}")]
    InvalidHost(String),
    #[error("connection {0} has no credential")]
    MissingCredential(i64),
    #[error("credential of connection '{0}' is already in {1}")]
    CredentialAlreadyIn(String, SecretStore),

    // publishing
    #[error("publishing is disabled for connection '{0}'")]
    PublishDisabled(String),

    // tickets
    #[error("ticket not found: {0}")]
    TicketNotFound(String),
    #[error("ticket with ID {0} not found")]
    TicketIdNotFound(i64),
    #[error("ticket {0} found in more than one connection: {connections}", connections = .1.join(", "))]
    AmbiguousTicket(String, Vec<String>),
    #[error("ticket {0} has {1} slots")]
    TicketHasSlots(String, u64),
    #[error("ticket {0} has no connection")]
    TicketHasNoConnection(String),
    #[error("unsupported ticket sort: {0}")]
    UnsupportedTicketSort(String),

    // slots
    #[error("slot not found: {0}")]
    SlotNotFound(i64),
    #[error("cannot edit a published slot")]
    SlotAlreadyPublished,
    #[error("stopped_at must be null or after started_at")]
    InvalidTimeRange,
    #[error("slot time cannot be in the future")]
    FutureTime,
    #[error("slot overlaps an existing slot")]
    SlotOverlap,
    #[error("start time must be after the start of the running slot")]
    StartBeforeRunningSlot,
    #[error("running slot started less than a second ago")]
    RunningSlotJustStarted,
    #[error("already tracking, use switch")]
    AlreadyTracking,
    #[error("not tracking, use resume")]
    NotTracking,
    #[error("no previous ticket")]
    NoPreviousTicket,

    // config
    #[error("unknown config key '{0}', expected a section or one of: {keys}", keys = KEYS.join(", "))]
    UnknownConfigKey(String),
    #[error("invalid value for config key '{0}'")]
    InvalidConfigValue(String, #[source] toml::de::Error),
    #[error("config key '{0}' cannot be unset")]
    ConfigKeyRequired(String),
}

impl From<sea_orm::DbErr> for NirvanaError {
    fn from(e: sea_orm::DbErr) -> Self {
        Self::Db(e.into())
    }
}
