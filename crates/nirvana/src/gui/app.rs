use std::sync::Arc;

use iced::{Element, Subscription, Task, keyboard, window};
use nirvana_core::Nirvana;
use tracing::error;

use super::profile::{self, ProfileState};
use super::shortcuts::shortcut;
use super::toast::{self, Kind};
use super::windows::{Window, Windows};
use super::{appearance, info, views};

const PROFILE_FAILED: &str = "couldn't load the profile";

pub(super) struct Gui {
    nirvana: Option<Result<Arc<Nirvana>, String>>,
    windows: Windows,
    appearance: appearance::State,
    info: info::State,
    profile: Option<ProfileState>,
    toast: toast::State,
}

#[derive(Clone)]
pub(super) enum Message {
    Loaded(Result<Arc<Nirvana>, String>),
    Closed(window::Id),
    Appearance(appearance::Message),
    Info(info::Message),
    Profile(profile::Message),
    Toast(toast::Message),
    Failed(window::Id, &'static str, String),
}

impl Gui {
    pub(super) fn new() -> (Self, Task<Message>) {
        let gui = Self {
            nirvana: None,
            windows: Windows::default(),
            appearance: appearance::State::default(),
            info: info::State::default(),
            profile: None,
            toast: toast::State::default(),
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
        let boot = appearance::system_mode()
            .map(Message::Appearance)
            .chain(load);
        (gui, boot)
    }

    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Loaded(nirvana) => {
                let next = match &nirvana {
                    Ok(nirvana) => {
                        self.appearance
                            .set_theme(nirvana.theme().unwrap_or_default());
                        profile::load(nirvana.clone()).map(Message::Profile)
                    }
                    Err(e) => {
                        error!("{e}");
                        self.open_main().discard()
                    }
                };
                self.nirvana = Some(nirvana);
                return next;
            }
            Message::Closed(id) => {
                self.toast.close(id);
                if let Some(Window::Main) = self.windows.remove(id) {
                    return iced::exit();
                }
            }
            Message::Appearance(message) => self.appearance.update(message),
            Message::Info(info::Message::Failed(text, e, window)) => {
                return self.show_error(window, text, e);
            }
            Message::Info(message) => {
                return self
                    .info
                    .update(message, self.nirvana.as_ref(), &mut self.windows)
                    .map(Message::Info);
            }
            Message::Profile(profile::Message::Failed(e)) => {
                return self
                    .open_main()
                    .map(move |window| Message::Failed(window, PROFILE_FAILED, e.clone()));
            }
            Message::Profile(message) => {
                profile::update(&mut self.profile, message);
                return self.open_main().discard();
            }
            Message::Toast(message) => return self.toast.update(message).map(Message::Toast),
            Message::Failed(window, text, e) => return self.show_error(window, text, e),
        }
        Task::none()
    }

    fn show_error(&mut self, window: window::Id, text: &str, detail: String) -> Task<Message> {
        self.toast
            .update(toast::Message::Show {
                window,
                kind: Kind::Error,
                text: text.to_owned(),
                detail: Some(detail),
            })
            .map(Message::Toast)
    }

    fn open_main(&mut self) -> Task<window::Id> {
        match self.windows.main() {
            Some(id) => Task::done(id),
            None => self.windows.open(Window::Main, views::MAIN_WINDOW_SIZE),
        }
    }

    pub(super) fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            appearance::subscription().map(Message::Appearance),
            keyboard::listen().filter_map(shortcut),
            window::close_events().map(Message::Closed),
        ])
    }

    pub(super) fn view(&self, window: window::Id) -> Element<'_, Message> {
        let content = match self.windows.get(window) {
            Some(Window::Info(info)) => views::info(info),
            _ => {
                let error = self.nirvana.as_ref().and_then(|n| n.as_ref().err());
                views::main(error.map(String::as_str), self.profile.as_ref())
            }
        };
        views::with_toast(content, self.toast.current(window))
    }

    pub(super) fn theme(&self, _window: window::Id) -> Option<iced::Theme> {
        self.appearance.theme()
    }
}
