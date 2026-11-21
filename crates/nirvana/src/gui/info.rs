use std::path::PathBuf;
use std::sync::Arc;

use iced::{Task, window};
use nirvana_core::Nirvana;
use tracing::error;

use super::views;
use super::windows::{Window, Windows};
use crate::about;

const INFO_CLICKS: u32 = 5;
const LOG_FILE_FAILED: &str = "couldn't find the log file";
const OPEN_LOGS_FAILED: &str = "couldn't open the log file";

#[derive(Clone)]
pub(super) struct Info {
    pub(super) rows: Result<Vec<(&'static str, String)>, String>,
    pub(super) log_file: Result<PathBuf, String>,
}

#[derive(Default)]
pub(super) struct State {
    logo_clicks: u32,
    loading: bool,
}

#[derive(Clone)]
pub(super) enum Message {
    LogoClicked,
    Loaded(Info),
    ShowLogs,
    Failed(&'static str, String, window::Id),
}

impl State {
    pub(super) fn update(
        &mut self,
        message: Message,
        nirvana: Option<&Result<Arc<Nirvana>, String>>,
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
                let failed = info.log_file.as_ref().err().cloned();
                return windows.open(Window::Info(info), size).then(move |window| {
                    failed.clone().map_or_else(Task::none, |e| {
                        Task::done(Message::Failed(LOG_FILE_FAILED, e, window))
                    })
                });
            }
            Message::ShowLogs => {
                if let Some((window, Ok(log_file))) = windows
                    .info()
                    .map(|(window, info)| (window, info.log_file.clone()))
                {
                    return Task::future(open_file(log_file)).then(move |opened| {
                        opened.map_or_else(
                            |e| Task::done(Message::Failed(OPEN_LOGS_FAILED, e, window)),
                            |()| Task::none(),
                        )
                    });
                }
            }
            Message::Failed(..) => {}
        }
        Task::none()
    }

    fn show(
        &mut self,
        nirvana: Option<&Result<Arc<Nirvana>, String>>,
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
    let log_file = Nirvana::log_file()
        .map_err(|e| e.to_string())
        .inspect_err(|e| error!("{e}"));
    Info { rows, log_file }
}

async fn open_file(path: PathBuf) -> Result<(), String> {
    let target = path.clone();
    let error = match tokio::task::spawn_blocking(move || opener::open(&target)).await {
        Ok(Ok(())) => return Ok(()),
        Ok(Err(e)) => e.to_string(),
        Err(e) => e.to_string(),
    };
    let error = format!("cannot open {}: {error}", path.display());
    error!("{error}");
    Err(error)
}
