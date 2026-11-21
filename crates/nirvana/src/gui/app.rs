use std::sync::Arc;

use iced::{Element, Subscription, Task, keyboard, window};
use nirvana_core::{Nirvana, NirvanaError};
use tracing::error;

use super::failure::{Failure, chain};
use super::profile::{self, ProfileState};
use super::shortcuts::shortcut;
use super::toast::{self, Kind};
use super::windows::{Window, Windows};
use super::{appearance, info, views};

pub(super) struct Gui {
    nirvana: Option<Result<Arc<Nirvana>, Failure>>,
    windows: Windows,
    appearance: appearance::State,
    info: info::State,
    profile: Option<ProfileState>,
    toast: toast::State,
}

#[derive(Clone)]
pub(super) enum Message {
    Loaded(Result<Arc<Nirvana>, Arc<NirvanaError>>),
    Closed(window::Id),
    Appearance(appearance::Message),
    Info(info::Message),
    Profile(profile::Message),
    Toast(toast::Message),
    Failed(window::Id, Failure),
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
            async { Nirvana::new().await.map(Arc::new).map_err(Arc::new) },
            Message::Loaded,
        );
        let boot = appearance::system_mode()
            .map(Message::Appearance)
            .chain(load);
        (gui, boot)
    }

    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Loaded(Ok(nirvana)) => {
                self.appearance
                    .set_theme(nirvana.theme().unwrap_or_default());
                self.nirvana = Some(Ok(nirvana.clone()));
                return profile::load(nirvana).map(Message::Profile);
            }
            Message::Loaded(Err(e)) => {
                error!("{}", chain(e.as_ref()));
                let failure = Failure::startup(&e);
                self.nirvana = Some(Err(failure.clone()));
                return self.show_error_in_main(failure);
            }
            Message::Closed(id) => {
                self.toast.close(id);
                if let Some(Window::Main) = self.windows.remove(id) {
                    return iced::exit();
                }
            }
            Message::Appearance(message) => self.appearance.update(message),
            Message::Info(info::Message::Failed(failure, window)) => {
                return self.show_error(window, failure);
            }
            Message::Info(message) => {
                return self
                    .info
                    .update(message, self.nirvana.as_ref(), &mut self.windows)
                    .map(Message::Info);
            }
            Message::Profile(profile::Message::Failed(e)) => {
                error!("{}", chain(e.as_ref()));
                return self.show_error_in_main(Failure::profile(&e));
            }
            Message::Profile(message) => {
                profile::update(&mut self.profile, message);
                return self.open_main().discard();
            }
            Message::Toast(message) => return self.toast.update(message).map(Message::Toast),
            Message::Failed(window, failure) => return self.show_error(window, failure),
        }
        Task::none()
    }

    fn show_error(&mut self, window: window::Id, failure: Failure) -> Task<Message> {
        self.toast
            .update(toast::Message::Show {
                window,
                kind: Kind::Error,
                text: failure.text.to_owned(),
                detail: failure.detail,
            })
            .map(Message::Toast)
    }

    fn show_error_in_main(&mut self, failure: Failure) -> Task<Message> {
        self.open_main()
            .map(move |window| Message::Failed(window, failure.clone()))
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
        let toast = self.toast.current(window);
        match self.windows.get(window) {
            Some(Window::Info(info)) => views::info(info, toast),
            _ => views::main(self.profile.as_ref(), toast),
        }
    }

    pub(super) fn theme(&self, _window: window::Id) -> Option<iced::Theme> {
        self.appearance.theme()
    }
}
