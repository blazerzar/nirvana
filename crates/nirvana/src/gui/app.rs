use std::sync::Arc;

use iced::{Element, Event, Subscription, Task, event, window};
use nirvana_core::{DesktopSettings, Nirvana, NirvanaError};
use tracing::error;

use super::components::LogicalScroll;
use super::failure::{Failure, chain};
use super::shortcuts::Shortcut;
use super::state::profile::{self, ProfileState};
use super::state::toast::{self, Kind};
use super::state::{appearance, info, listing, overview, settings, start, tracking};
use super::windows::{Window, Windows};
use super::{shortcuts, views};

pub(super) struct Gui {
    nirvana: Option<Result<Arc<Nirvana>, Failure>>,
    windows: Windows,
    appearance: appearance::State,
    info: info::State,
    listing: listing::State,
    overview: overview::State,
    profile: Option<ProfileState>,
    settings: settings::State,
    start: start::State,
    toast: toast::State,
    tracking: tracking::State,
    screen: Screen,
    command_held: bool,
    scale_factor: f32,
}

/// The screen shown between the header and the bottom bar of the main
/// window.
#[derive(Clone, Copy, Default, PartialEq)]
pub(super) enum Screen {
    #[default]
    Main,
    Settings,
    Overview,
}

/// `Nirvana` with the config values the GUI needs before its first frame.
#[derive(Clone)]
pub(super) struct Loaded {
    nirvana: Arc<Nirvana>,
    settings: Option<DesktopSettings>,
}

#[derive(Clone)]
pub(super) enum Message {
    Loaded(Result<Loaded, Arc<NirvanaError>>),
    Closed(window::Id),
    Opened(window::Id),
    Rescaled(window::Id, f32),
    Appearance(appearance::Message),
    Info(info::Message),
    Listing(listing::Message),
    Overview(overview::Message),
    Profile(profile::Message),
    Settings(settings::Message),
    /// Shows the screen, or the main screen when it is already shown.
    ToggleScreen(Screen),
    Shortcut(window::Id, Shortcut),
    /// ⌘ or Ctrl was pressed or released.
    CommandHeld(bool),
    Start(start::Message),
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
            listing: listing::State::default(),
            overview: overview::State::default(),
            profile: None,
            settings: settings::State::default(),
            start: start::State::default(),
            toast: toast::State::default(),
            tracking: tracking::State::default(),
            screen: Screen::default(),
            command_held: false,
            scale_factor: 1.0,
        };
        let load = Task::perform(async { load().await.map_err(Arc::new) }, Message::Loaded);
        let boot = appearance::system_mode()
            .map(Message::Appearance)
            .chain(load);
        (gui, boot)
    }

    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Loaded(Ok(Loaded { nirvana, settings })) => {
                self.appearance.set_theme(
                    settings
                        .as_ref()
                        .map(|settings| settings.theme)
                        .unwrap_or_default(),
                );
                let server = self
                    .settings
                    .load(&nirvana, settings)
                    .map(Message::Settings);
                self.nirvana = Some(Ok(nirvana.clone()));
                return Task::batch([server, profile::load(nirvana).map(Message::Profile)]);
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
            Message::Opened(id) => {
                return window::scale_factor(id).map(move |factor| Message::Rescaled(id, factor));
            }
            Message::Rescaled(id, factor) => {
                if self.windows.main() == Some(id) {
                    self.scale_factor = factor;
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
            Message::ToggleScreen(screen) if self.can_show(screen) => {
                let shown = if self.screen == screen {
                    Screen::Main
                } else {
                    screen
                };
                self.show(shown);
                if shown == Screen::Overview {
                    return self.update(Message::Overview(overview::Message::Load));
                }
            }
            Message::ToggleScreen(_) => {}
            Message::Listing(listing::Message::Failed(failure)) => {
                return self.show_error_in_main(failure);
            }
            Message::Listing(message) => {
                return self
                    .listing
                    .update(message, self.nirvana.as_ref())
                    .map(Message::Listing);
            }
            Message::Overview(overview::Message::Failed(failure)) => {
                return self.show_error_in_main(failure);
            }
            Message::Overview(message) => {
                return self
                    .overview
                    .update(message, self.nirvana.as_ref())
                    .map(Message::Overview);
            }
            Message::Shortcut(window, shortcut) => {
                if self.windows.main() != Some(window) {
                    return Task::none();
                }
                let main_shown = self.screen == Screen::Main && !self.start.is_open();
                match shortcut {
                    Shortcut::Escape if self.screen != Screen::Main => self.show(Screen::Main),
                    Shortcut::Escape => return self.update(Message::Start(start::Message::Cancel)),
                    Shortcut::OpenWeb => {
                        return self.update(Message::Settings(settings::Message::OpenWeb));
                    }
                    Shortcut::Return if main_shown && self.is_in_setup() => {
                        return self.update(Message::Settings(settings::Message::OpenWeb));
                    }
                    Shortcut::ToggleScreen(screen) => {
                        return self.update(Message::ToggleScreen(screen));
                    }
                    Shortcut::Start(message) if main_shown && self.is_set_up() => {
                        return self.update(Message::Start(message));
                    }
                    Shortcut::Tracking(message) if main_shown => {
                        return self.update(Message::Tracking(message));
                    }
                    Shortcut::Recent(index) if main_shown && self.is_set_up() => {
                        if let Some(&recent) = self.tracking.recent().get(index) {
                            let start = tracking::Message::Start(recent.clone());
                            return self.update(Message::Tracking(start));
                        }
                    }
                    Shortcut::Return
                    | Shortcut::Start(_)
                    | Shortcut::Tracking(_)
                    | Shortcut::Recent(_) => {}
                }
            }
            Message::CommandHeld(held) => self.command_held = held,
            Message::Start(start::Message::Succeeded) => {
                return self.update(Message::Tracking(tracking::Message::Load));
            }
            Message::Start(start::Message::Failed(failure)) => {
                return self.show_error_in_main(failure);
            }
            Message::Start(message) => {
                return self
                    .start
                    .update(message, self.nirvana.as_ref())
                    .map(Message::Start);
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
                let loaded = matches!(message, tracking::Message::Loaded(_));
                let update = self
                    .tracking
                    .update(message, self.nirvana.as_ref())
                    .map(Message::Tracking);
                if !loaded {
                    return update;
                }
                let listing = self.update(Message::Listing(listing::Message::Load));
                let overview = if self.screen == Screen::Overview {
                    self.update(Message::Overview(overview::Message::Load))
                } else {
                    Task::none()
                };
                return Task::batch([update, listing, overview]);
            }
            Message::Failed(window, failure) => return self.show_error(window, failure),
        }
        Task::none()
    }

    fn show(&mut self, screen: Screen) {
        self.screen = screen;
        self.settings.close_idle_after();
        self.settings.reset_port_input();
    }

    fn can_show(&self, screen: Screen) -> bool {
        match screen {
            Screen::Main => true,
            Screen::Settings => self.settings.is_available(),
            Screen::Overview => self.is_set_up(),
        }
    }

    /// Returns whether a profile is set up, which the start form and the
    /// overview need.
    fn is_set_up(&self) -> bool {
        self.profile
            .as_ref()
            .is_some_and(|profile| profile.setup_step().is_none())
    }

    /// Returns whether the setup steps are shown.
    fn is_in_setup(&self) -> bool {
        self.profile
            .as_ref()
            .and_then(ProfileState::setup_step)
            .is_some()
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
            window::open_events().map(Message::Opened),
            event::listen_with(rescaled),
            self.tracking.subscription().map(Message::Tracking),
        ])
    }

    pub(super) fn view(&self, window: window::Id) -> Element<'_, Message> {
        let toast = self.toast.current(window);
        match self.windows.get(window) {
            Some(Window::Info(info)) => views::info(info, toast),
            _ => {
                let header = views::header(self.profile.as_ref(), self.screen, &self.settings);
                let screen = match (self.screen, self.settings.idle(), self.profile.as_ref()) {
                    (Screen::Settings, Some(idle), _) => (
                        views::settings_view(idle, &self.settings),
                        views::settings_bar(self.settings.is_web_available()),
                    ),
                    (Screen::Overview, _, Some(ProfileState::Active(profile))) => {
                        let elapsed = self.tracking.elapsed();
                        let stats = self.overview.stats(elapsed);
                        let week = self.overview.week(elapsed);
                        (
                            views::overview_view(&profile.name, stats, week),
                            views::overview_bar(self.settings.is_web_available()),
                        )
                    }
                    _ => views::main_screen(
                        self.profile.as_ref(),
                        &self.tracking,
                        &self.start,
                        &self.listing,
                        self.appearance.tokens(),
                        self.command_held,
                        self.settings.is_web_available(),
                    ),
                };
                LogicalScroll::new(views::main(header, screen, toast), self.scale_factor).into()
            }
        }
    }

    pub(super) fn theme(&self, _window: window::Id) -> Option<iced::Theme> {
        self.appearance.theme()
    }
}

fn rescaled(event: Event, _: event::Status, id: window::Id) -> Option<Message> {
    match event {
        Event::Window(window::Event::Rescaled(factor)) => Some(Message::Rescaled(id, factor)),
        _ => None,
    }
}

async fn load() -> Result<Loaded, NirvanaError> {
    let nirvana = Nirvana::new().await?;
    Ok(Loaded {
        settings: nirvana.desktop_settings().await.ok(),
        nirvana: Arc::new(nirvana),
    })
}
