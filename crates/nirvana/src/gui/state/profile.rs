use std::sync::Arc;

use iced::Task;
use nirvana_core::{DomainError, Nirvana, NirvanaError};
use tracing::warn;

pub(in crate::gui) enum ProfileState {
    Missing,
    Active(Profile),
}

pub(in crate::gui) struct Profile {
    pub(in crate::gui) name: String,
    connections: usize,
    has_slots: bool,
    tested: usize,
    failed: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::gui) enum Health {
    Healthy,
    Degraded,
    Down,
}

impl ProfileState {
    pub(in crate::gui) fn setup_step(&self) -> Option<u8> {
        match self {
            Self::Missing => Some(1),
            Self::Active(profile) if profile.connections == 0 && !profile.has_slots => Some(2),
            Self::Active(_) => None,
        }
    }
}

impl Profile {
    pub(in crate::gui) fn has_connections(&self) -> bool {
        self.connections > 0
    }

    /// Returns `None` until every connection is tested, and for a profile
    /// without connections.
    pub(in crate::gui) fn health(&self) -> Option<Health> {
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
pub(in crate::gui) enum Message {
    Missing,
    Failed(Arc<NirvanaError>),
    Loaded(String, usize, bool),
    ConnectionTested(bool),
}

pub(in crate::gui) fn update(profile: &mut Option<ProfileState>, message: Message) {
    match message {
        Message::Missing => *profile = Some(ProfileState::Missing),
        Message::Failed(_) => {}
        Message::Loaded(name, connections, has_slots) => {
            *profile = Some(ProfileState::Active(Profile {
                name,
                connections,
                has_slots,
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

pub(in crate::gui) fn load(nirvana: Arc<Nirvana>) -> Task<Message> {
    let service = nirvana.clone();
    Task::future(async move {
        let active = service.active_profile().await?;
        let has_slots = service.has_slots().await?;
        Ok::<_, NirvanaError>((active, has_slots))
    })
    .then(move |loaded| {
        let (active, has_slots) = match loaded {
            Ok(loaded) => loaded,
            Err(NirvanaError::Domain(DomainError::NoActiveProfile)) => {
                return Task::done(Message::Missing);
            }
            Err(e) => return Task::done(Message::Failed(Arc::new(e))),
        };
        let loaded = Message::Loaded(active.profile.name, active.connections.len(), has_slots);
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
            has_slots: false,
            tested,
            failed,
        }
    }

    #[test]
    fn setup_starts_at_profile_step_without_profile() {
        assert_eq!(ProfileState::Missing.setup_step(), Some(1));
    }

    #[test]
    fn setup_continues_at_connection_step_for_empty_profile() {
        let empty = ProfileState::Active(profile(0, 0, 0));
        assert_eq!(empty.setup_step(), Some(2));
    }

    #[test]
    fn setup_is_done_with_connections() {
        let connected = ProfileState::Active(profile(1, 0, 0));
        assert_eq!(connected.setup_step(), None);
    }

    #[test]
    fn setup_is_done_with_slots() {
        let tracked = ProfileState::Active(Profile {
            has_slots: true,
            ..profile(0, 0, 0)
        });
        assert_eq!(tracked.setup_step(), None);
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
