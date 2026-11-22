use std::path::PathBuf;
use std::sync::Arc;

use iced::{Task, window};
use nirvana_core::Nirvana;
use tracing::error;

use crate::about;
use crate::gui::failure::{Failure, chain};
use crate::gui::views;
use crate::gui::windows::{Window, Windows};

const INFO_CLICKS: u32 = 5;
const INFO_FAILED: &str = "couldn't load the app info";
const LOG_FILE_FAILED: &str = "couldn't find the log file";
const OPEN_LOGS_FAILED: &str = "couldn't open the log file";
const MISSING_FILE: &str = "the file doesn't exist";

#[derive(Clone)]
pub(in crate::gui) struct Info {
    pub(in crate::gui) rows: Result<Vec<(&'static str, String)>, Failure>,
    pub(in crate::gui) log_file: Result<PathBuf, String>,
}

#[derive(Default)]
pub(in crate::gui) struct State {
    logo_clicks: u32,
    loading: bool,
}

#[derive(Clone)]
pub(in crate::gui) enum Message {
    LogoClicked,
    Loaded(Info),
    ShowLogs,
    Failed(Failure, window::Id),
}

impl State {
    pub(in crate::gui) fn update(
        &mut self,
        message: Message,
        nirvana: Option<&Result<Arc<Nirvana>, Failure>>,
        windows: &mut Windows,
    ) -> Task<Message> {
        match message {
            Message::LogoClicked => {
                self.logo_clicks += 1;
                if self.logo_clicks == INFO_CLICKS {
                    self.logo_clicks = 0;
                    return self.show(nirvana, windows);
                }
            }
            Message::Loaded(info) => {
                self.loading = false;
                let size = views::info_size(&info);
                let failure = info.rows.as_ref().err().cloned().or_else(|| {
                    info.log_file
                        .as_ref()
                        .err()
                        .map(|_| Failure::new(LOG_FILE_FAILED, None))
                });
                return windows.open(Window::Info(info), size).then(move |window| {
                    failure.clone().map_or_else(Task::none, |failure| {
                        Task::done(Message::Failed(failure, window))
                    })
                });
            }
            Message::ShowLogs => {
                if let Some((window, Ok(log_file))) = windows
                    .info()
                    .map(|(window, info)| (window, info.log_file.clone()))
                {
                    return Task::future(open_file(log_file)).then(move |opened| match opened {
                        Ok(()) => Task::none(),
                        Err(failure) => Task::done(Message::Failed(failure, window)),
                    });
                }
            }
            Message::Failed(..) => {}
        }
        Task::none()
    }

    fn show(
        &mut self,
        nirvana: Option<&Result<Arc<Nirvana>, Failure>>,
        windows: &Windows,
    ) -> Task<Message> {
        if let Some((id, _)) = windows.info() {
            return window::gain_focus(id);
        }
        let Some(nirvana) = nirvana.cloned() else {
            return Task::none();
        };
        if self.loading {
            return Task::none();
        }
        self.loading = true;
        Task::perform(load_info(nirvana), Message::Loaded)
    }
}

async fn load_info(nirvana: Result<Arc<Nirvana>, Failure>) -> Info {
    let rows = match nirvana {
        Ok(nirvana) => nirvana
            .info()
            .await
            .map(|info| about::rows(&info))
            .map_err(|e| {
                error!("{}", chain(&e));
                Failure::from_error(INFO_FAILED, &e)
            }),
        Err(failure) => Err(failure),
    };
    let log_file = Nirvana::log_file()
        .map_err(|e| e.to_string())
        .inspect_err(|e| error!("{e}"));
    Info { rows, log_file }
}

async fn open_file(path: PathBuf) -> Result<(), Failure> {
    if !path.exists() {
        error!("cannot open {}: {MISSING_FILE}", path.display());
        return Err(Failure::new(OPEN_LOGS_FAILED, Some(MISSING_FILE)));
    }
    let target = path.clone();
    let failure = match tokio::task::spawn_blocking(move || opener::open(&target)).await {
        Ok(Ok(())) => return Ok(()),
        Ok(Err(e)) => {
            error!("cannot open {}: {}", path.display(), chain(&e));
            Failure::from_error(OPEN_LOGS_FAILED, &e)
        }
        Err(e) => {
            error!("cannot open {}: {e}", path.display());
            Failure::new(OPEN_LOGS_FAILED, None)
        }
    };
    Err(failure)
}
