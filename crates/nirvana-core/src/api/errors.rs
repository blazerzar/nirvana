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
    #[error("ticket not found: {0}")]
    TicketNotFound(String),
    #[error("slot not found: {0}")]
    SlotNotFound(i64),
    #[error("cannot edit a published slot")]
    SlotAlreadyPublished,
    #[error("stopped_at must be null or after started_at")]
    InvalidTimeRange,
}
