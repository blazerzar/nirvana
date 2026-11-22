use std::fmt;
use std::sync::Arc;

use iced::Task;
use iced::widget::{Id, operation};
use nirvana_core::{Nirvana, NirvanaError, TicketConnection, TicketSort};
use tracing::{error, info};

use crate::gui::failure::{Failure, chain};

const LOAD_FAILED: &str = "couldn't load recent tickets";
const START_FAILED: &str = "couldn't start tracking";
const RECENT_LIMIT: usize = 5;
pub(in crate::gui) const TICKET_INPUT: Id = Id::new("start-ticket");

/// The start form: the ticket key, the connection to search in and the
/// recently worked on tickets.
#[derive(Default)]
pub(in crate::gui) struct State {
    open: bool,
    starting: bool,
    key: String,
    connection: ConnectionChoice,
    connection_open: bool,
    connections: Vec<String>,
    recent: Vec<Recent>,
}

#[derive(Clone)]
pub(in crate::gui) struct Recent {
    pub(in crate::gui) key: String,
    pub(in crate::gui) title: String,
    connection: Option<String>,
}

/// The connection the ticket is searched in.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::gui) enum ConnectionChoice {
    #[default]
    All,
    Named(String),
}

impl fmt::Display for ConnectionChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => f.write_str("all connections"),
            Self::Named(name) => f.write_str(name),
        }
    }
}

#[derive(Clone)]
pub(in crate::gui) enum Message {
    Open,
    Cancel,
    Loaded(Vec<String>, Vec<Recent>),
    KeyChanged(String),
    ToggleConnection,
    CloseConnection,
    Connection(ConnectionChoice),
    Recent(Recent),
    Start,
    Started(Result<(), Failure>),
    Succeeded,
    Failed(Failure),
}

impl State {
    pub(in crate::gui) fn is_open(&self) -> bool {
        self.open
    }

    pub(in crate::gui) fn is_starting(&self) -> bool {
        self.starting
    }

    pub(in crate::gui) fn key(&self) -> &str {
        &self.key
    }

    pub(in crate::gui) fn connection(&self) -> &ConnectionChoice {
        &self.connection
    }

    pub(in crate::gui) fn is_connection_open(&self) -> bool {
        self.connection_open
    }

    /// Returns the connection choices, or `None` when the profile has at
    /// most one connection.
    pub(in crate::gui) fn connection_choices(&self) -> Option<Vec<ConnectionChoice>> {
        (self.connections.len() > 1).then(|| {
            let named = self
                .connections
                .iter()
                .cloned()
                .map(ConnectionChoice::Named);
            [ConnectionChoice::All].into_iter().chain(named).collect()
        })
    }

    pub(in crate::gui) fn recent(&self) -> &[Recent] {
        &self.recent
    }

    /// Fills in the key of `recent` and, when the connection choices are
    /// shown, its connection.
    fn pick(&mut self, recent: Recent) {
        self.key = recent.key;
        if self.connection_choices().is_some() {
            self.connection = recent
                .connection
                .map_or(ConnectionChoice::All, ConnectionChoice::Named);
        }
    }

    pub(in crate::gui) fn update(
        &mut self,
        message: Message,
        nirvana: Option<&Result<Arc<Nirvana>, Failure>>,
    ) -> Task<Message> {
        let Some(Ok(nirvana)) = nirvana else {
            return Task::none();
        };
        match message {
            Message::Open => {
                *self = Self {
                    open: true,
                    ..Self::default()
                };
                return Task::batch([load(nirvana.clone()), operation::focus(TICKET_INPUT)]);
            }
            Message::Cancel => *self = Self::default(),
            Message::Loaded(connections, recent) => {
                self.connections = connections;
                self.recent = recent;
            }
            Message::KeyChanged(key) => self.key = key,
            Message::ToggleConnection => self.connection_open = !self.connection_open,
            Message::CloseConnection => self.connection_open = false,
            Message::Connection(connection) => {
                self.connection = connection;
                self.connection_open = false;
            }
            Message::Recent(recent) => self.pick(recent),
            Message::Start => {
                let key = self.key.trim().to_owned();
                if key.is_empty() || self.starting {
                    return Task::none();
                }
                self.starting = true;
                let connection = match &self.connection {
                    ConnectionChoice::All => None,
                    ConnectionChoice::Named(name) => Some(name.clone()),
                };
                return start(nirvana.clone(), key, connection);
            }
            Message::Started(Ok(())) => {
                *self = Self::default();
                return Task::done(Message::Succeeded);
            }
            Message::Started(Err(failure)) => {
                self.starting = false;
                return Task::done(Message::Failed(failure));
            }
            Message::Succeeded | Message::Failed(_) => {}
        }
        Task::none()
    }
}

fn load(nirvana: Arc<Nirvana>) -> Task<Message> {
    Task::future(async move {
        let connections = nirvana.list_connections().await?;
        let tickets = nirvana
            .list_tickets(TicketConnection::Any, TicketSort::LastWorked)
            .await?;
        let connections = connections
            .into_iter()
            .map(|connection| connection.model.name)
            .collect();
        let recent = tickets
            .into_iter()
            .take(RECENT_LIMIT)
            .map(|(ticket, connection)| Recent {
                key: ticket.ticket_key,
                title: ticket.title,
                connection: connection.map(|connection| connection.name),
            })
            .collect();
        Ok::<_, NirvanaError>(Message::Loaded(connections, recent))
    })
    .map(|loaded| loaded.unwrap_or_else(|e| Message::Failed(failure(LOAD_FAILED, &e))))
}

fn start(nirvana: Arc<Nirvana>, key: String, connection: Option<String>) -> Task<Message> {
    Task::future(async move {
        let started = nirvana
            .start_slot(&key, None, connection.as_deref())
            .await?;
        let slot = &started.started;
        info!(
            key = %slot.ticket.ticket_key,
            slot_id = slot.model.id,
            started_at = slot.model.started_at,
            "slot started");
        Ok(())
    })
    .map(|started: Result<(), NirvanaError>| {
        Message::Started(started.map_err(|e| failure(START_FAILED, &e)))
    })
}

fn failure(text: &'static str, error: &NirvanaError) -> Failure {
    error!("{}", chain(error));
    Failure::from_error(text, error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(connections: &[&str]) -> State {
        State {
            connections: connections.iter().map(|&name| name.to_owned()).collect(),
            ..State::default()
        }
    }

    fn recent(connection: Option<&str>) -> Recent {
        Recent {
            key: "NIR-1".into(),
            title: "title".into(),
            connection: connection.map(str::to_owned),
        }
    }

    #[test]
    fn connection_choices_are_hidden_with_one_connection() {
        assert_eq!(state(&["jira"]).connection_choices(), None);
    }

    #[test]
    fn connection_choices_start_with_all_connections() {
        assert_eq!(
            state(&["jira", "linear"]).connection_choices(),
            Some(vec![
                ConnectionChoice::All,
                ConnectionChoice::Named("jira".into()),
                ConnectionChoice::Named("linear".into()),
            ])
        );
    }

    #[test]
    fn picking_a_recent_ticket_selects_its_connection() {
        let mut state = state(&["jira", "linear"]);
        state.pick(recent(Some("linear")));
        assert_eq!(state.key, "NIR-1");
        assert_eq!(state.connection, ConnectionChoice::Named("linear".into()));
    }

    #[test]
    fn picking_a_recent_ticket_without_connection_searches_all() {
        let mut state = state(&["jira", "linear"]);
        state.connection = ConnectionChoice::Named("jira".into());
        state.pick(recent(None));
        assert_eq!(state.connection, ConnectionChoice::All);
    }

    #[test]
    fn picking_a_recent_ticket_keeps_all_with_one_connection() {
        let mut state = state(&["jira"]);
        state.pick(recent(Some("jira")));
        assert_eq!(state.connection, ConnectionChoice::All);
    }
}
