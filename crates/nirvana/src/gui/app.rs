use iced::theme::Mode;
use iced::{Element, Subscription, Task, keyboard, system};
use nirvana_core::Theme;

use super::theme::{DARK, LIGHT, Tokens};
use super::views;

pub(super) struct Gui {
    theme: Theme,
    system_mode: Mode,
}

#[derive(Debug, Clone)]
pub(super) enum Message {
    SystemThemeChanged(Mode),
    ToggleTheme,
}

impl Gui {
    pub(super) fn new(theme: Theme) -> (Self, Task<Message>) {
        let gui = Self {
            theme,
            system_mode: Mode::None,
        };
        (gui, system::theme().map(Message::SystemThemeChanged))
    }

    pub(super) fn update(&mut self, message: Message) {
        match message {
            Message::SystemThemeChanged(mode) => self.system_mode = mode,
            Message::ToggleTheme => {
                self.theme = if self.is_dark() {
                    Theme::Light
                } else {
                    Theme::Dark
                };
            }
        }
    }

    pub(super) fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            system::theme_changes().map(Message::SystemThemeChanged),
            keyboard::listen().filter_map(toggle_theme_shortcut),
        ])
    }

    pub(super) fn view(&self) -> Element<'_, Message> {
        views::main(self.tokens())
    }

    fn is_dark(&self) -> bool {
        match self.theme {
            Theme::System => self.system_mode == Mode::Dark,
            Theme::Light => false,
            Theme::Dark => true,
        }
    }

    fn tokens(&self) -> &'static Tokens {
        if self.is_dark() { &DARK } else { &LIGHT }
    }
}

fn toggle_theme_shortcut(event: keyboard::Event) -> Option<Message> {
    let keyboard::Event::KeyPressed {
        key,
        physical_key,
        modifiers,
        repeat: false,
        ..
    } = event
    else {
        return None;
    };
    let is_t = key
        .to_latin(physical_key)
        .is_some_and(|c| c.eq_ignore_ascii_case(&'t'));
    (modifiers.command() && modifiers.shift() && is_t).then_some(Message::ToggleTheme)
}
