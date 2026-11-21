use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use iced::theme::Mode;
use iced::{Element, Size, Subscription, Task, keyboard, system, window};
use nirvana_core::{DomainError, Nirvana, NirvanaError, Theme};
use tracing::{error, warn};

use super::views;
use crate::about;

const INFO_CLICKS: u32 = 5;

pub(super) struct Gui {
    nirvana: Option<Result<Arc<Nirvana>, String>>,
    theme: Theme,
    system_mode: Mode,
    windows: BTreeMap<window::Id, Window>,
    logo_clicks: u32,
    loading_info: bool,
    profile: Option<ProfileState>,
}

pub(super) enum Window {
    Main,
    Info(Info),
}

#[derive(Clone)]
pub(super) struct Info {
    pub(super) rows: Result<Vec<(&'static str, String)>, String>,
    pub(super) log_file: Option<PathBuf>,
}

pub(super) enum ProfileState {
    Missing,
    Active(Profile),
}

pub(super) struct Profile {
    pub(super) name: String,
    connections: usize,
    tested: usize,
    failed: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Health {
    Healthy,
    Degraded,
    Down,
}

impl Profile {
    /// Returns `None` until every connection is tested, and for a profile
    /// without connections.
    pub(super) fn health(&self) -> Option<Health> {
        (self.connections > 0 && self.tested == self.connections).then_some(
            if self.failed == self.connections {
                Health::Down
            } else if self.failed == 0 {
                Health::Healthy
            } else {
                Health::Degraded
            },
        )
    }
}

#[derive(Clone)]
pub(super) enum Message {
    Loaded(Result<Arc<Nirvana>, String>),
    Closed(window::Id),
    LogoClicked,
    NoProfile,
    ProfileLoaded(String, usize),
    ConnectionTested(bool),
    InfoLoaded(Info),
    ShowLogs,
    SystemThemeChanged(Mode),
    ToggleTheme,
}

impl Gui {
    pub(super) fn new() -> (Self, Task<Message>) {
        let gui = Self {
            nirvana: None,
            theme: Theme::default(),
            system_mode: Mode::None,
            windows: BTreeMap::new(),
            logo_clicks: 0,
            loading_info: false,
            profile: None,
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
                let check = match &nirvana {
                    Ok(nirvana) => {
                        self.theme = nirvana.theme().unwrap_or_default();
                        check_profile(nirvana.clone())
                    }
                    Err(e) => {
                        error!("{e}");
                        Task::none()
                    }
                };
                self.nirvana = Some(nirvana);
                return Task::batch([
                    self.open_window(Window::Main, views::MAIN_WINDOW_SIZE),
                    check,
                ]);
            }
            Message::Closed(id) => {
                if let Some(Window::Main) = self.windows.remove(&id) {
                    return iced::exit();
                }
            }
            Message::LogoClicked => {
                self.logo_clicks += 1;
                if self.logo_clicks == INFO_CLICKS {
                    self.logo_clicks = 0;
                    return self.show_info();
                }
            }
            Message::NoProfile => self.profile = Some(ProfileState::Missing),
            Message::ProfileLoaded(name, connections) => {
                self.profile = Some(ProfileState::Active(Profile {
                    name,
                    connections,
                    tested: 0,
                    failed: 0,
                }));
            }
            Message::ConnectionTested(passed) => {
                if let Some(ProfileState::Active(profile)) = &mut self.profile {
                    profile.tested += 1;
                    profile.failed += usize::from(!passed);
                }
            }
            Message::InfoLoaded(info) => {
                self.loading_info = false;
                let size = views::info_size(&info);
                return self.open_window(Window::Info(info), size);
            }
            Message::ShowLogs => {
                if let Some(log_file) = self
                    .info_window()
                    .and_then(|(_, info)| info.log_file.clone())
                {
                    return Task::future(open_file(log_file)).discard();
                }
            }
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
            window::close_events().map(Message::Closed),
        ])
    }

    pub(super) fn view(&self, window: window::Id) -> Element<'_, Message> {
        match self.windows.get(&window) {
            Some(Window::Info(info)) => views::info(info),
            _ => {
                let error = self.nirvana.as_ref().and_then(|n| n.as_ref().err());
                views::main(error.map(String::as_str), self.profile.as_ref())
            }
        }
    }

    pub(super) fn theme(&self, _window: window::Id) -> Option<iced::Theme> {
        match self.theme {
            Theme::System => None,
            Theme::Light => Some(iced::Theme::Light),
            Theme::Dark => Some(iced::Theme::Dark),
        }
    }

    fn show_info(&mut self) -> Task<Message> {
        if let Some((id, _)) = self.info_window() {
            return window::gain_focus(id);
        }
        let Some(nirvana) = self.nirvana.clone() else {
            return Task::none();
        };
        if self.loading_info {
            return Task::none();
        }
        self.loading_info = true;
        Task::perform(load_info(nirvana), Message::InfoLoaded)
    }

    fn open_window(&mut self, kind: Window, size: Size) -> Task<Message> {
        let (id, open) = window::open(window::Settings {
            size,
            ..window::Settings::default()
        });
        self.windows.insert(id, kind);
        open.discard()
    }

    fn info_window(&self) -> Option<(window::Id, &Info)> {
        self.windows.iter().find_map(|(id, window)| match window {
            Window::Info(info) => Some((*id, info)),
            Window::Main => None,
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

fn check_profile(nirvana: Arc<Nirvana>) -> Task<Message> {
    let service = nirvana.clone();
    Task::future(async move { service.active_profile().await }).then(move |active| {
        let active = match active {
            Ok(active) => active,
            Err(NirvanaError::Domain(DomainError::NoActiveProfile)) => {
                return Task::done(Message::NoProfile);
            }
            Err(e) => {
                error!("{e}");
                return Task::none();
            }
        };
        let loaded = Message::ProfileLoaded(active.profile.name, active.connections.len());
        let tests = active.connections.into_iter().map(|connection| {
            Task::perform(
                test_connection(nirvana.clone(), connection.model.name),
                Message::ConnectionTested,
            )
        });
        tests.fold(Task::done(loaded), Task::chain)
    })
}

async fn test_connection(nirvana: Arc<Nirvana>, name: String) -> bool {
    match nirvana.test_connection(&name).await {
        Ok(()) => true,
        Err(e) => {
            let error = format!("{:#}", anyhow::Error::from(e));
            warn!(%name, %error, "connection test failed");
            false
        }
    }
}

async fn load_info(nirvana: Result<Arc<Nirvana>, String>) -> Info {
    let rows = match nirvana {
        Ok(nirvana) => nirvana
            .info()
            .await
            .map(|info| about::rows(&info))
            .map_err(|e| e.to_string())
            .inspect_err(|e| error!("{e}")),
        Err(e) => Err(e),
    };
    let log_file = Nirvana::log_file().inspect_err(|e| error!("{e}")).ok();
    Info { rows, log_file }
}

async fn open_file(path: PathBuf) {
    let target = path.clone();
    match tokio::task::spawn_blocking(move || opener::open(&target)).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => error!("cannot open {}: {e}", path.display()),
        Err(e) => error!("cannot open {}: {e}", path.display()),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(connections: usize, tested: usize, failed: usize) -> Profile {
        Profile {
            name: "work".into(),
            connections,
            tested,
            failed,
        }
    }

    #[test]
    fn health_is_unknown_while_testing() {
        assert_eq!(profile(2, 1, 1).health(), None);
    }

    #[test]
    fn health_is_unknown_without_connections() {
        assert_eq!(profile(0, 0, 0).health(), None);
    }

    #[test]
    fn health_is_healthy_when_no_test_fails() {
        assert_eq!(profile(2, 2, 0).health(), Some(Health::Healthy));
    }

    #[test]
    fn health_is_degraded_when_some_tests_fail() {
        assert_eq!(profile(3, 3, 1).health(), Some(Health::Degraded));
    }

    #[test]
    fn health_is_down_when_all_tests_fail() {
        assert_eq!(profile(2, 2, 2).health(), Some(Health::Down));
    }
}
