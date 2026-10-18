pub use crate::config::ConfigError;
pub use crate::integration::IntegrationError;
pub use crate::storage::DbError;
use thiserror::Error;

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
}

#[derive(Debug, Error)]
pub enum TrackingError {
    #[error("no active connection")]
    NoActiveConnection,
    #[error("connection not found: {0}")]
    ConnectionNotFound(i64),
    #[error("cannot delete a connection with local history")]
    ConnectionHasHistory,
    #[error("ticket not found: {0}")]
    TicketNotFound(String),
    #[error("slot not found: {0}")]
    SlotNotFound(i64),
    #[error("cannot edit a published slot")]
    SlotAlreadyPublished,
    #[error("cannot delete a published slot")]
    CannotDeletePublished,
    #[error("stopped_at must be null or after started_at")]
    InvalidTimeRange,
    #[error("slot time cannot be in the future")]
    FutureTime,
    #[error("slot overlaps an existing slot")]
    SlotOverlap,
}
