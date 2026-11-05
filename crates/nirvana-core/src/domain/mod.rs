mod connection;
mod credential;
mod profile;
mod slot;

pub use connection::{Connection, ConnectionData, JiraSettings, Kind, Settings};
pub use credential::{Credential, SecretLocation};
pub use profile::{ActiveProfile, Profile};
pub use slot::{PublishFailure, PublishResult, SlotCreate, SlotEdit, SlotSort};

#[derive(Debug, Default)]
pub enum Change<T> {
    #[default]
    Skip,
    Clear,
    Set(T),
}
