mod connection;
mod profile;
mod slot;

pub use connection::{Connection, ConnectionData, JiraSettings, Kind, Settings};
pub use profile::{ActiveProfile, Profile};
pub use slot::{PublishFailure, PublishResult, SlotCreate, SlotEdit, SlotSort};

#[derive(Debug, Default)]
pub enum Change<T> {
    #[default]
    Skip,
    Clear,
    Set(T),
}
