mod connection;
mod credential;
mod info;
mod profile;
mod slot;

pub use connection::{Connection, ConnectionData, ConnectionEdit, JiraSettings, Kind, Settings};
pub use credential::{Credential, SecretLocation, SecretStore};
pub use info::AppInfo;
pub use profile::{ActiveProfile, Profile};
pub use slot::{PublishFailure, PublishResult, SlotCreate, SlotEdit, SlotSort};

#[derive(Debug, Default)]
pub enum Change<T> {
    #[default]
    Skip,
    Clear,
    Set(T),
}
