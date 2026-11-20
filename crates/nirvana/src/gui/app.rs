use std::sync::Arc;

use iced::theme::Mode;
use iced::{Element, Size, Subscription, Task, keyboard, system, window};
use nirvana_core::{Nirvana, Theme};
use tracing::error;

use super::theme::{DARK, LIGHT, Tokens};
use super::views;

const WINDOW_SIZE: Size = Size::new(960.0, 540.0);

pub(super) struct Gui {
    nirvana: Option<Result<Arc<Nirvana>, String>>,
    theme: Theme,
    system_mode: Mode,
}

#[derive(Clone)]
pub(super) enum Message {
    Loaded(Result<Arc<Nirvana>, String>),
    WindowClosed,
    SystemThemeChanged(Mode),
    ToggleTheme,
}

impl Gui {
    pub(super) fn new() -> (Self, Task<Message>) {
        let gui = Self {
            nirvana: None,
            theme: Theme::default(),
            system_mode: Mode::None,
        };
        let load = Task::perform(
            async {
                Nirvana::new()
                    .await
                    .map(Arc::new)
                    .map_err(|e| e.to_string())
            },
            Message::Loaded,
        );
        let boot = system::theme().map(Message::SystemThemeChanged).chain(load);
        (gui, boot)
    }

    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Loaded(nirvana) => {
                match &nirvana {
                    Ok(nirvana) => self.theme = nirvana.theme().unwrap_or_default(),
                    Err(e) => error!("{e}"),
                }
                self.nirvana = Some(nirvana);
                let (_, open) = window::open(window::Settings {
                    size: WINDOW_SIZE,
                    ..window::Settings::default()
                });
                return open.discard();
            }
            Message::WindowClosed => return iced::exit(),
            Message::SystemThemeChanged(mode) => self.system_mode = mode,
            Message::ToggleTheme => {
                self.theme = if self.is_dark() {
                    Theme::Light
                } else {
                    Theme::Dark
                };
            }
        }
        Task::none()
    }

    pub(super) fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            system::theme_changes().map(Message::SystemThemeChanged),
            keyboard::listen().filter_map(toggle_theme_shortcut),
            window::close_events().map(|_| Message::WindowClosed),
        ])
    }

    pub(super) fn view(&self, _window: window::Id) -> Element<'_, Message> {
        let error = self.nirvana.as_ref().and_then(|n| n.as_ref().err());
        views::main(self.tokens(), error.map(String::as_str))
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
