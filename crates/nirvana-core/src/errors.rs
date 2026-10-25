use thiserror::Error;

pub use crate::config::ConfigError;
pub use crate::integration::IntegrationError;
pub use crate::secrets::SecretError;
pub use crate::storage::DbError;

#[derive(Debug, Error)]
pub enum NirvanaError {
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Tracking(#[from] TrackingError),
    #[error(transparent)]
    Integration(#[from] IntegrationError),
    #[error(transparent)]
    Secret(#[from] SecretError),
}

#[derive(Debug, Error)]
pub enum TrackingError {
    #[error("no active connection")]
    NoActiveConnection,
    #[error("no active profile")]
    NoActiveProfile,
    #[error("connection not found: {0}")]
    ConnectionNotFound(i64),
    #[error("cannot delete a connection with local history")]
    ConnectionHasHistory,
    #[error("unsupported connection kind: {0}")]
    UnsupportedConnectionKind(String),
    #[error("connection {0} has no settings")]
    MissingConnectionSettings(i64),
    #[error("publishing is disabled for connection '{0}'")]
    PublishDisabled(String),
    #[error("ticket not found: {0}")]
    TicketNotFound(String),
    #[error("ticket {0} has no connection")]
    TicketHasNoConnection(String),
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
}

impl From<sea_orm::DbErr> for NirvanaError {
    fn from(e: sea_orm::DbErr) -> Self {
        Self::Db(e.into())
    }
}
