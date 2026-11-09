use std::str::FromStr;

use crate::errors::DomainError;

#[derive(Clone, Debug, Default)]
pub enum TicketConnection {
    #[default]
    Any,
    Named(String),
    Disconnected,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum TicketSort {
    Key,
    Title,
    Connection,
    #[default]
    LastWorked,
    Synced,
}

impl TicketSort {
    pub const ALL: [Self; 5] = [
        Self::Key,
        Self::Title,
        Self::Connection,
        Self::LastWorked,
        Self::Synced,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Key => "key",
            Self::Title => "title",
            Self::Connection => "connection",
            Self::LastWorked => "last-worked",
            Self::Synced => "synced",
        }
    }
}

impl FromStr for TicketSort {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|sort| sort.as_str() == s)
            .ok_or_else(|| DomainError::UnsupportedTicketSort(s.to_string()))
    }
}
