use std::collections::BTreeMap;
use std::time::Duration;

use iced::{Task, window};

#[derive(Clone, Copy)]
pub(super) enum Kind {
    #[allow(dead_code)]
    Success,
    #[allow(dead_code)]
    Warning,
    Error,
}

impl Kind {
    fn lifetime(self) -> Option<Duration> {
        match self {
            Self::Success => Some(Duration::from_secs(4)),
            Self::Warning => Some(Duration::from_secs(8)),
            Self::Error => None,
        }
    }
}

pub(super) struct Toast {
    pub(super) id: u64,
    pub(super) kind: Kind,
    pub(super) text: String,
    pub(super) detail: Option<String>,
}

#[derive(Default)]
pub(super) struct State {
    next_id: u64,
    current: BTreeMap<window::Id, Toast>,
}

#[derive(Clone)]
pub(super) enum Message {
    Show {
        window: window::Id,
        kind: Kind,
        text: String,
        detail: Option<String>,
    },
    Hide(u64),
}

impl State {
    pub(super) fn current(&self, window: window::Id) -> Option<&Toast> {
        self.current.get(&window)
    }

    pub(super) fn close(&mut self, window: window::Id) {
        self.current.remove(&window);
    }

    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Show {
                window,
                kind,
                text,
                detail,
            } => {
                self.next_id += 1;
                let id = self.next_id;
                self.current.insert(
                    window,
                    Toast {
                        id,
                        kind,
                        text,
                        detail,
                    },
                );
                if let Some(lifetime) = kind.lifetime() {
                    return Task::future(async move { tokio::time::sleep(lifetime).await })
                        .map(move |()| Message::Hide(id));
                }
            }
            Message::Hide(id) => self.current.retain(|_, toast| toast.id != id),
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show(state: &mut State, window: window::Id, text: &str) -> u64 {
        let _ = state.update(Message::Show {
            window,
            kind: Kind::Error,
            text: text.into(),
            detail: None,
        });
        state.current(window).map_or(0, |toast| toast.id)
    }

    fn text(state: &State, window: window::Id) -> Option<&str> {
        state.current(window).map(|toast| toast.text.as_str())
    }

    #[test]
    fn hide_clears_the_shown_toast() {
        let mut state = State::default();
        let window = window::Id::unique();
        let id = show(&mut state, window, "first");
        let _ = state.update(Message::Hide(id));
        assert!(state.current(window).is_none());
    }

    #[test]
    fn stale_hide_keeps_the_newer_toast() {
        let mut state = State::default();
        let window = window::Id::unique();
        let first = show(&mut state, window, "first");
        show(&mut state, window, "second");
        let _ = state.update(Message::Hide(first));
        assert_eq!(text(&state, window), Some("second"));
    }

    #[test]
    fn toasts_are_kept_per_window() {
        let mut state = State::default();
        let (main, info) = (window::Id::unique(), window::Id::unique());
        show(&mut state, main, "main");
        let id = show(&mut state, info, "info");
        assert_eq!(text(&state, main), Some("main"));
        let _ = state.update(Message::Hide(id));
        assert!(state.current(info).is_none());
        assert_eq!(text(&state, main), Some("main"));
    }

    #[test]
    fn closing_a_window_drops_its_toast() {
        let mut state = State::default();
        let window = window::Id::unique();
        show(&mut state, window, "first");
        state.close(window);
        assert!(state.current(window).is_none());
    }

    #[test]
    fn errors_stay_until_dismissed() {
        assert_eq!(Kind::Error.lifetime(), None);
    }
}
