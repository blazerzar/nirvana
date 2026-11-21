use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use iced::theme::Mode;
use iced::{Element, Size, Subscription, Task, keyboard, system, window};
use nirvana_core::{Nirvana, Theme};
use tracing::error;

use super::views;
use crate::about;

const MAIN_WINDOW_SIZE: Size = Size::new(960.0, 540.0);
const INFO_CLICKS: u32 = 5;

pub(super) struct Gui {
    nirvana: Option<Result<Arc<Nirvana>, String>>,
    theme: Theme,
    system_mode: Mode,
    windows: BTreeMap<window::Id, Window>,
    logo_clicks: u32,
    loading_info: bool,
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

#[derive(Clone)]
pub(super) enum Message {
    Loaded(Result<Arc<Nirvana>, String>),
    Closed(window::Id),
    LogoClicked,
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
                return self.open_window(Window::Main, MAIN_WINDOW_SIZE);
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
                views::main(error.map(String::as_str))
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
