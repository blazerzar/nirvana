use std::sync::Arc;

use iced::Task;
use nirvana_core::{DomainError, Nirvana, NirvanaError};
use tracing::{error, warn};

pub(super) enum ProfileState {
    Missing,
    Active(Profile),
}

pub(super) struct Profile {
    pub(super) name: String,
    connections: usize,
    tested: usize,
    failed: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Health {
    Healthy,
    Degraded,
    Down,
}

impl Profile {
    /// Returns `None` until every connection is tested, and for a profile
    /// without connections.
    pub(super) fn health(&self) -> Option<Health> {
        (self.connections > 0 && self.tested == self.connections).then_some(
            if self.failed == self.connections {
                Health::Down
            } else if self.failed == 0 {
                Health::Healthy
            } else {
                Health::Degraded
            },
        )
    }
}

#[derive(Clone)]
pub(super) enum Message {
    Missing,
    Loaded(String, usize),
    ConnectionTested(bool),
}

pub(super) fn update(profile: &mut Option<ProfileState>, message: Message) {
    match message {
        Message::Missing => *profile = Some(ProfileState::Missing),
        Message::Loaded(name, connections) => {
            *profile = Some(ProfileState::Active(Profile {
                name,
                connections,
                tested: 0,
                failed: 0,
            }));
        }
        Message::ConnectionTested(passed) => {
            if let Some(ProfileState::Active(profile)) = profile {
                profile.tested += 1;
                profile.failed += usize::from(!passed);
            }
        }
    }
}

pub(super) fn load(nirvana: Arc<Nirvana>) -> Task<Message> {
    let service = nirvana.clone();
    Task::future(async move { service.active_profile().await }).then(move |active| {
        let active = match active {
            Ok(active) => active,
            Err(NirvanaError::Domain(DomainError::NoActiveProfile)) => {
                return Task::done(Message::Missing);
            }
            Err(e) => {
                error!("{e}");
                return Task::none();
            }
        };
        let loaded = Message::Loaded(active.profile.name, active.connections.len());
        let tests = active.connections.into_iter().map(|connection| {
            Task::perform(
                test_connection(nirvana.clone(), connection.model.name),
                Message::ConnectionTested,
            )
        });
        tests.fold(Task::done(loaded), Task::chain)
    })
}

async fn test_connection(nirvana: Arc<Nirvana>, name: String) -> bool {
    match nirvana.test_connection(&name).await {
        Ok(()) => true,
        Err(e) => {
            let error = format!("{:#}", anyhow::Error::from(e));
            warn!(%name, %error, "connection test failed");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(connections: usize, tested: usize, failed: usize) -> Profile {
        Profile {
            name: "work".into(),
            connections,
            tested,
            failed,
        }
    }

    #[test]
    fn health_is_unknown_while_testing() {
        assert_eq!(profile(2, 1, 1).health(), None);
    }

    #[test]
    fn health_is_unknown_without_connections() {
        assert_eq!(profile(0, 0, 0).health(), None);
    }

    #[test]
    fn health_is_healthy_when_no_test_fails() {
        assert_eq!(profile(2, 2, 0).health(), Some(Health::Healthy));
    }

    #[test]
    fn health_is_degraded_when_some_tests_fail() {
        assert_eq!(profile(3, 3, 1).health(), Some(Health::Degraded));
    }

    #[test]
    fn health_is_down_when_all_tests_fail() {
        assert_eq!(profile(2, 2, 2).health(), Some(Health::Down));
    }
}
