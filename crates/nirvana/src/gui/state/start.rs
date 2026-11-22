use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use iced::Task;
use iced::widget::{Id, operation};
use nirvana_core::{Nirvana, NirvanaError, TicketConnection, TicketSort};
use tracing::{error, info};

use crate::gui::failure::{Failure, chain};

const LOAD_FAILED: &str = "couldn't load recent tickets";
const START_FAILED: &str = "couldn't start tracking";
const RECENT_MAX: usize = 9;
pub(in crate::gui) const TICKET_INPUT: Id = Id::new("start-ticket");

/// The start form: the ticket key, the connection to search in, the tickets
/// by when they were last worked on and the one picked from them.
#[derive(Default)]
pub(in crate::gui) struct State {
    open: bool,
    starting: bool,
    key: String,
    connection: ConnectionChoice,
    connection_open: bool,
    connections: Vec<String>,
    recent: Vec<Recent>,
    picked: Option<Recent>,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::gui) struct Recent {
    pub(in crate::gui) id: i64,
    pub(in crate::gui) key: String,
    pub(in crate::gui) title: String,
    pub(in crate::gui) connection: Option<String>,
    /// The connection, shown only when another ticket has the same key.
    pub(in crate::gui) tag: Option<String>,
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
    /// Picks the shown recent ticket at the index.
    RecentShortcut(usize),
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

    /// Returns the most recent tickets that fuzzily match the key.
    pub(in crate::gui) fn recent(&self) -> Vec<&Recent> {
        let query = self.key.trim().to_lowercase();
        self.recent
            .iter()
            .filter(|recent| recent.matches(&query))
            .take(RECENT_MAX)
            .collect()
    }

    pub(in crate::gui) fn is_picked(&self, recent: &Recent) -> bool {
        self.picked.as_ref() == Some(recent)
    }

    /// Sets the typed key, which no longer is a picked recent ticket.
    fn set_key(&mut self, key: String) {
        self.key = key;
        self.picked = None;
    }

    /// Fills in the key of `recent` and, when the connection choices are
    /// shown, its connection.
    fn pick(&mut self, recent: Recent) {
        self.key.clone_from(&recent.key);
        if self.connection_choices().is_some() {
            self.connection = recent
                .connection
                .clone()
                .map_or(ConnectionChoice::All, ConnectionChoice::Named);
        }
        self.picked = Some(recent);
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
            Message::KeyChanged(key) => self.set_key(key),
            Message::ToggleConnection => self.connection_open = !self.connection_open,
            Message::CloseConnection => self.connection_open = false,
            Message::Connection(connection) => {
                self.connection = connection;
                self.connection_open = false;
                self.picked = None;
            }
            Message::Recent(recent) => {
                self.pick(recent);
                return operation::focus(TICKET_INPUT);
            }
            Message::RecentShortcut(index) => {
                if let Some(recent) = self.recent().get(index).copied().cloned() {
                    self.pick(recent);
                    return operation::focus(TICKET_INPUT);
                }
            }
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
        let connections = nirvana
            .list_connections()
            .await?
            .into_iter()
            .map(|connection| connection.model.name)
            .collect();
        let recent = recent_tickets(&nirvana).await?;
        Ok::<_, NirvanaError>(Message::Loaded(connections, recent))
    })
    .map(|loaded| loaded.unwrap_or_else(|e| Message::Failed(failure(LOAD_FAILED, &e))))
}

/// Returns the tickets of the active profile by when they were last worked
/// on.
pub(in crate::gui) async fn recent_tickets(nirvana: &Nirvana) -> Result<Vec<Recent>, NirvanaError> {
    let mut recent: Vec<Recent> = nirvana
        .list_tickets(TicketConnection::Any, TicketSort::LastWorked)
        .await?
        .into_iter()
        .map(|(ticket, connection)| Recent {
            id: ticket.id,
            key: ticket.ticket_key,
            title: ticket.title,
            connection: connection.map(|connection| connection.name),
            tag: None,
        })
        .collect();
    tag_shared_keys(&mut recent);
    Ok(recent)
}

/// Tags the tickets whose key another ticket has with their connection.
fn tag_shared_keys(recent: &mut [Recent]) {
    let mut counts = HashMap::<String, usize>::new();
    for ticket in recent.iter() {
        *counts.entry(ticket.key.clone()).or_default() += 1;
    }
    for ticket in recent.iter_mut() {
        if counts[&ticket.key] > 1 {
            ticket.tag.clone_from(&ticket.connection);
        }
    }
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

impl Recent {
    /// Returns whether the lowercase `query` is empty, its characters appear
    /// in order in the key or it is part of the title, ignoring case.
    fn matches(&self, query: &str) -> bool {
        query.is_empty()
            || is_subsequence(query, &self.key.to_lowercase())
            || self.title.to_lowercase().contains(query)
    }
}

fn is_subsequence(query: &str, text: &str) -> bool {
    let mut text = text.chars();
    query.chars().all(|q| text.any(|t| t == q))
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
        ticket("NIR-1", "title", connection)
    }

    fn ticket(key: &str, title: &str, connection: Option<&str>) -> Recent {
        Recent {
            id: 0,
            key: key.into(),
            title: title.into(),
            connection: connection.map(str::to_owned),
            tag: None,
        }
    }

    fn tags(recent: &[Recent]) -> Vec<Option<&str>> {
        recent.iter().map(|recent| recent.tag.as_deref()).collect()
    }

    fn keys(state: &State) -> Vec<&str> {
        state
            .recent()
            .into_iter()
            .map(|recent| recent.key.as_str())
            .collect()
    }

    #[test]
    fn recent_shows_the_most_recent_without_a_key() {
        let state = State {
            recent: (1..=11)
                .map(|n| ticket(&format!("NIR-{n}"), "title", None))
                .collect(),
            ..State::default()
        };
        assert_eq!(
            keys(&state),
            [
                "NIR-1", "NIR-2", "NIR-3", "NIR-4", "NIR-5", "NIR-6", "NIR-7", "NIR-8", "NIR-9"
            ]
        );
    }

    #[test]
    fn recent_matches_the_key_fuzzily_and_the_title() {
        let state = State {
            key: " nr1 ".into(),
            recent: vec![
                ticket("NIR-12", "timer", None),
                ticket("DES-50", "status bar", None),
                ticket("DES-51", "nr1 notes", None),
                ticket("NIR-3", "connection list", None),
            ],
            ..State::default()
        };
        assert_eq!(keys(&state), ["NIR-12", "DES-51"]);
    }

    #[test]
    fn shared_keys_are_tagged_with_their_connection() {
        let mut recent = vec![
            ticket("OPS-241", "staging", Some("acme")),
            ticket("NIR-12", "timer", Some("acme")),
            ticket("OPS-241", "on-call", Some("acme-dc")),
            ticket("OPS-241", "old", None),
        ];
        tag_shared_keys(&mut recent);
        assert_eq!(tags(&recent), [Some("acme"), None, Some("acme-dc"), None]);
    }

    #[test]
    fn unique_keys_are_not_tagged() {
        let mut recent = vec![
            ticket("NIR-12", "timer", Some("acme")),
            ticket("NIR-9", "review", Some("linear")),
        ];
        tag_shared_keys(&mut recent);
        assert_eq!(tags(&recent), [None, None]);
    }

    #[test]
    fn picking_marks_only_the_picked_ticket() {
        let mut state = state(&["jira", "linear"]);
        state.pick(recent(Some("jira")));
        assert!(state.is_picked(&recent(Some("jira"))));
        assert!(!state.is_picked(&recent(Some("linear"))));
    }

    #[test]
    fn typing_the_key_of_a_recent_ticket_does_not_pick_it() {
        let mut state = state(&["jira"]);
        state.set_key("NIR-1".into());
        assert!(!state.is_picked(&recent(Some("jira"))));
    }

    #[test]
    fn typing_after_picking_clears_the_pick() {
        let mut state = state(&["jira"]);
        state.pick(recent(Some("jira")));
        state.set_key("NIR-12".into());
        assert!(!state.is_picked(&recent(Some("jira"))));
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
