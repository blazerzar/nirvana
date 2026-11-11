mod config;
mod connection;
mod credential;
mod info;
mod logs;
mod profile;
mod slot;
mod ticket;

pub use config::ConfigEntry;
pub use connection::{
    Connection, ConnectionData, ConnectionEdit, JiraSettings, JiraSettingsEdit, Kind, Settings,
    SettingsEdit,
};
pub use credential::{Credential, SecretLocation, SecretStore};
pub use info::AppInfo;
pub use logs::LogReader;
pub use profile::{ActiveProfile, Profile};
pub use slot::{Slot, SlotSort, StartResult};
pub use ticket::{TicketConnection, TicketSort, TicketTarget};

#[derive(Debug, Default)]
pub enum Change<T> {
    #[default]
    Skip,
    Clear,
    Set(T),
}
