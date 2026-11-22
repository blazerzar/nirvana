use std::fmt;
use std::sync::Arc;

use iced::Task;
use nirvana_core::{IdleConfig, IdleMethod, Nirvana, NirvanaError};
use tracing::error;

use crate::gui::failure::{Failure, chain};

const SAVE_FAILED: &str = "couldn't save the settings";
const IDLE_AFTER_MINUTES: [u64; 6] = [1, 2, 5, 10, 15, 30];

/// Desktop settings. Available once the idle settings are loaded.
#[derive(Default)]
pub(in crate::gui) struct State {
    open: bool,
    idle: Option<IdleConfig>,
    idle_after_open: bool,
}

#[derive(Clone)]
pub(in crate::gui) enum Message {
    Toggle,
    Close,
    IdleEnabled(bool),
    ToggleIdleAfter,
    CloseIdleAfter,
    IdleAfter(IdleAfter),
    IdleMethod(IdleMethod),
    /// The error and the idle settings that are still saved.
    SaveFailed(Arc<NirvanaError>, Option<IdleConfig>),
    Failed(Failure),
}

/// The idle threshold in seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::gui) struct IdleAfter(u64);

impl fmt::Display for IdleAfter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_multiple_of(60) {
            write!(f, "{} min", self.0 / 60)
        } else {
            write!(f, "{} s", self.0)
        }
    }
}

impl IdleAfter {
    pub(in crate::gui) fn of(idle: &IdleConfig) -> Self {
        Self(idle.threshold_secs)
    }

    /// Returns the offered thresholds, with `current` added if it is not one
    /// of them.
    pub(in crate::gui) fn options(current: Self) -> Vec<Self> {
        let mut options: Vec<_> = IDLE_AFTER_MINUTES
            .iter()
            .map(|minutes| Self(minutes * 60))
            .collect();
        if !options.contains(&current) {
            options.push(current);
            options.sort_unstable_by_key(|option| option.0);
        }
        options
    }
}

impl State {
    pub(in crate::gui) fn load(&mut self, idle: Option<IdleConfig>) {
        self.idle = idle;
    }

    pub(in crate::gui) fn is_available(&self) -> bool {
        self.idle.is_some()
    }

    /// Returns the idle settings while the settings are open.
    pub(in crate::gui) fn shown(&self) -> Option<&IdleConfig> {
        self.idle.as_ref().filter(|_| self.open)
    }

    pub(in crate::gui) fn is_idle_after_open(&self) -> bool {
        self.idle_after_open
    }

    pub(in crate::gui) fn update(
        &mut self,
        message: Message,
        nirvana: Option<&Result<Arc<Nirvana>, Failure>>,
    ) -> Task<Message> {
        let Some(Ok(nirvana)) = nirvana else {
            return Task::none();
        };
        let Some(idle) = &mut self.idle else {
            return Task::none();
        };
        match message {
            Message::Toggle => {
                self.open = !self.open;
                self.idle_after_open = false;
            }
            Message::Close => {
                self.open = false;
                self.idle_after_open = false;
            }
            Message::ToggleIdleAfter => self.idle_after_open = !self.idle_after_open,
            Message::CloseIdleAfter => self.idle_after_open = false,
            Message::IdleEnabled(enabled) => {
                idle.enabled = enabled;
                self.idle_after_open &= enabled;
                return save(nirvana.clone(), idle.clone());
            }
            Message::IdleAfter(IdleAfter(secs)) => {
                idle.threshold_secs = secs;
                self.idle_after_open = false;
                return save(nirvana.clone(), idle.clone());
            }
            Message::IdleMethod(method) => {
                idle.methods = toggled(&idle.methods, method);
                return save(nirvana.clone(), idle.clone());
            }
            Message::Failed(_) => {}
            Message::SaveFailed(e, saved) => {
                error!("{}", chain(e.as_ref()));
                if let Some(saved) = saved {
                    *idle = saved;
                }
                return Task::done(Message::Failed(Failure::from_error(
                    SAVE_FAILED,
                    e.as_ref(),
                )));
            }
        }
        Task::none()
    }
}

/// Returns `methods` with `method` added or removed, in the order of
/// [`IdleMethod::ALL`].
fn toggled(methods: &[IdleMethod], method: IdleMethod) -> Vec<IdleMethod> {
    IdleMethod::ALL
        .into_iter()
        .filter(|candidate| {
            let is_selected = methods.contains(candidate);
            if *candidate == method {
                !is_selected
            } else {
                is_selected
            }
        })
        .collect()
}

fn save(nirvana: Arc<Nirvana>, idle: IdleConfig) -> Task<Message> {
    Task::future(async move {
        match nirvana.set_idle(idle).await {
            Ok(()) => None,
            Err(e) => Some(Message::SaveFailed(Arc::new(e), nirvana.idle().await.ok())),
        }
    })
    .and_then(Task::done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_after_shows_minutes_or_seconds() {
        assert_eq!(IdleAfter(300).to_string(), "5 min");
        assert_eq!(IdleAfter(90).to_string(), "90 s");
    }

    #[test]
    fn idle_after_options_keep_an_offered_threshold() {
        let options = IdleAfter::options(IdleAfter(300));
        assert_eq!(options.len(), IDLE_AFTER_MINUTES.len());
    }

    #[test]
    fn idle_after_options_add_a_custom_threshold_in_order() {
        let options = IdleAfter::options(IdleAfter(90));
        assert_eq!(
            &options[..3],
            [IdleAfter(60), IdleAfter(90), IdleAfter(120)]
        );
    }

    #[test]
    fn toggled_removes_a_selected_method() {
        assert_eq!(
            toggled(&IdleMethod::ALL, IdleMethod::Sleep),
            [IdleMethod::Lock, IdleMethod::Input]
        );
    }

    #[test]
    fn toggled_adds_a_method_in_order() {
        assert_eq!(
            toggled(&[IdleMethod::Input], IdleMethod::Lock),
            [IdleMethod::Lock, IdleMethod::Input]
        );
    }
}
