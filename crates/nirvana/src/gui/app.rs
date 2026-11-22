use std::sync::Arc;

use iced::{Element, Subscription, Task, window};
use nirvana_core::{IdleConfig, Nirvana, NirvanaError, Theme};
use tracing::error;

use super::failure::{Failure, chain};
use super::shortcuts::Shortcut;
use super::state::profile::{self, ProfileState};
use super::state::toast::{self, Kind};
use super::state::{appearance, info, settings, tracking};
use super::windows::{Window, Windows};
use super::{shortcuts, views};

pub(super) struct Gui {
    nirvana: Option<Result<Arc<Nirvana>, Failure>>,
    windows: Windows,
    appearance: appearance::State,
    info: info::State,
    profile: Option<ProfileState>,
    settings: settings::State,
    toast: toast::State,
    tracking: tracking::State,
}

/// `Nirvana` with the config values the GUI needs before its first frame.
#[derive(Clone)]
pub(super) struct Loaded {
    nirvana: Arc<Nirvana>,
    theme: Theme,
    idle: Option<IdleConfig>,
}

#[derive(Clone)]
pub(super) enum Message {
    Loaded(Result<Loaded, Arc<NirvanaError>>),
    Closed(window::Id),
    Appearance(appearance::Message),
    Info(info::Message),
    Profile(profile::Message),
    Settings(settings::Message),
    Shortcut(window::Id, Shortcut),
    Toast(toast::Message),
    Tracking(tracking::Message),
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
            settings: settings::State::default(),
            toast: toast::State::default(),
            tracking: tracking::State::default(),
        };
        let load = Task::perform(async { load().await.map_err(Arc::new) }, Message::Loaded);
        let boot = appearance::system_mode()
            .map(Message::Appearance)
            .chain(load);
        (gui, boot)
    }

    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Loaded(Ok(Loaded {
                nirvana,
                theme,
                idle,
            })) => {
                self.appearance.set_theme(theme);
                self.settings.load(idle);
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
                let loaded = matches!(message, profile::Message::Loaded(..));
                profile::update(&mut self.profile, message);
                let open = self.open_main().discard();
                let tracking = if loaded {
                    self.update(Message::Tracking(tracking::Message::Load))
                } else {
                    Task::none()
                };
                return Task::batch([open, tracking]);
            }
            Message::Settings(settings::Message::Failed(failure)) => {
                return self.show_error_in_main(failure);
            }
            Message::Settings(message) => {
                return self
                    .settings
                    .update(message, self.nirvana.as_ref())
                    .map(Message::Settings);
            }
            Message::Shortcut(window, shortcut) => {
                if self.windows.main() != Some(window) {
                    return Task::none();
                }
                match shortcut {
                    Shortcut::Settings(message) => return self.update(Message::Settings(message)),
                    Shortcut::Tracking(message) if self.settings.shown().is_none() => {
                        return self.update(Message::Tracking(message));
                    }
                    Shortcut::Tracking(_) => {}
                }
            }
            Message::Toast(message) => return self.toast.update(message).map(Message::Toast),
            Message::Tracking(tracking::Message::Succeeded(text)) => {
                if let Some(window) = self.windows.main() {
                    return self
                        .toast
                        .update(toast::Message::Show {
                            window,
                            kind: Kind::Success,
                            text,
                            detail: None,
                        })
                        .map(Message::Toast);
                }
            }
            Message::Tracking(tracking::Message::Failed(failure)) => {
                return self.show_error_in_main(failure);
            }
            Message::Tracking(message) => {
                return self
                    .tracking
                    .update(message, self.nirvana.as_ref())
                    .map(Message::Tracking);
            }
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
            shortcuts::listen(),
            window::close_events().map(Message::Closed),
            self.tracking.subscription().map(Message::Tracking),
        ])
    }

    pub(super) fn view(&self, window: window::Id) -> Element<'_, Message> {
        let toast = self.toast.current(window);
        match self.windows.get(window) {
            Some(Window::Info(info)) => views::info(info, toast),
            _ => views::main(
                self.profile.as_ref(),
                &self.settings,
                &self.tracking,
                self.appearance.tokens(),
                toast,
            ),
        }
    }

    pub(super) fn theme(&self, _window: window::Id) -> Option<iced::Theme> {
        self.appearance.theme()
    }
}

async fn load() -> Result<Loaded, NirvanaError> {
    let nirvana = Nirvana::new().await?;
    Ok(Loaded {
        theme: nirvana.theme().await.unwrap_or_default(),
        idle: nirvana.idle().await.ok(),
        nirvana: Arc::new(nirvana),
    })
}
