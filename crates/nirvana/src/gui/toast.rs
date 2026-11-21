use std::time::Duration;

use iced::Task;

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
    current: Option<Toast>,
}

#[derive(Clone)]
pub(super) enum Message {
    Show {
        kind: Kind,
        text: String,
        detail: Option<String>,
    },
    Hide(u64),
}

impl State {
    pub(super) fn current(&self) -> Option<&Toast> {
        self.current.as_ref()
    }

    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Show { kind, text, detail } => {
                self.next_id += 1;
                let id = self.next_id;
                self.current = Some(Toast {
                    id,
                    kind,
                    text,
                    detail,
                });
                if let Some(lifetime) = kind.lifetime() {
                    return Task::future(async move { tokio::time::sleep(lifetime).await })
                        .map(move |()| Message::Hide(id));
                }
            }
            Message::Hide(id) => {
                if self.current.as_ref().is_some_and(|toast| toast.id == id) {
                    self.current = None;
                }
            }
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show(state: &mut State, text: &str) -> u64 {
        let _ = state.update(Message::Show {
            kind: Kind::Error,
            text: text.into(),
            detail: None,
        });
        state.current().map_or(0, |toast| toast.id)
    }

    #[test]
    fn hide_clears_the_shown_toast() {
        let mut state = State::default();
        let id = show(&mut state, "first");
        let _ = state.update(Message::Hide(id));
        assert!(state.current().is_none());
    }

    #[test]
    fn stale_hide_keeps_the_newer_toast() {
        let mut state = State::default();
        let first = show(&mut state, "first");
        show(&mut state, "second");
        let _ = state.update(Message::Hide(first));
        assert_eq!(state.current().map(|t| t.text.as_str()), Some("second"));
    }

    #[test]
    fn errors_stay_until_dismissed() {
        assert_eq!(Kind::Error.lifetime(), None);
    }
}
