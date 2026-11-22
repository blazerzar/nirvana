use iced::theme::Mode;
use iced::{Subscription, Task, system};
use nirvana_core::Theme;

use super::design_system::{Tokens, tokens};

#[derive(Default)]
pub(super) struct State {
    theme: Theme,
    system_mode: Mode,
}

#[derive(Clone, Copy)]
pub(super) enum Message {
    SystemThemeChanged(Mode),
    Toggle,
}

impl State {
    pub(super) fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub(super) fn update(&mut self, message: Message) {
        match message {
            Message::SystemThemeChanged(mode) => self.system_mode = mode,
            Message::Toggle => {
                self.theme = if self.is_dark() {
                    Theme::Light
                } else {
                    Theme::Dark
                };
            }
        }
    }

    pub(super) fn theme(&self) -> Option<iced::Theme> {
        match self.theme {
            Theme::System => None,
            Theme::Light => Some(iced::Theme::Light),
            Theme::Dark => Some(iced::Theme::Dark),
        }
    }

    pub(super) fn tokens(&self) -> &'static Tokens {
        tokens(&if self.is_dark() {
            iced::Theme::Dark
        } else {
            iced::Theme::Light
        })
    }

    fn is_dark(&self) -> bool {
        match self.theme {
            Theme::System => self.system_mode == Mode::Dark,
            Theme::Light => false,
            Theme::Dark => true,
        }
    }
}

pub(super) fn system_mode() -> Task<Message> {
    system::theme().map(Message::SystemThemeChanged)
}

pub(super) fn subscription() -> Subscription<Message> {
    system::theme_changes().map(Message::SystemThemeChanged)
}
