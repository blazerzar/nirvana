use std::sync::Arc;
use std::time::Duration;
use std::{fmt, io};

use iced::Task;
use nirvana_core::{IdleConfig, IdleMethod, Nirvana, NirvanaError};
use tokio::sync::oneshot;
use tracing::error;

use crate::gui::failure::{Failure, chain};
use crate::web;

const SAVE_FAILED: &str = "couldn't save the settings";
const PORT_KEY: &str = "core.port";
/// How long the starting status shows at least, so a change is visible.
const MIN_STARTING: Duration = Duration::from_millis(500);
const OPEN_WEB_FAILED: &str = "couldn't open nirvana web";
const IDLE_AFTER_MINUTES: [u64; 6] = [1, 2, 5, 10, 15, 30];

/// Desktop settings. Available once the config is loaded.
#[derive(Default)]
pub(in crate::gui) struct State {
    idle: Option<IdleConfig>,
    idle_after_open: bool,
    port: Option<u16>,
    port_input: String,
    /// Stops the local web server when dropped.
    server: Option<oneshot::Sender<()>>,
    server_status: ServerStatus,
}

#[derive(Default)]
pub(in crate::gui) enum ServerStatus {
    Starting,
    /// Running on the port.
    Running(u16),
    #[default]
    Off,
    Failed(String),
}

#[derive(Clone)]
pub(in crate::gui) enum Message {
    IdleEnabled(bool),
    ToggleIdleAfter,
    CloseIdleAfter,
    IdleAfter(IdleAfter),
    IdleMethod(IdleMethod),
    ServerEnabled(bool),
    /// The port the server listens on, or why it couldn't start.
    ServerStarted(Result<u16, Arc<io::Error>>),
    PortInput(String),
    PortSaved(u16),
    OpenWeb,
    /// The error and the idle settings that are still saved.
    SaveFailed(Arc<NirvanaError>, Option<IdleConfig>),
    Failed(Failure),
}

/// The idle threshold in seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::gui) struct IdleAfter(u64);

impl fmt::Display for IdleAfter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_multiple_of(60) {
            write!(f, "{} min", self.0 / 60)
        } else {
            write!(f, "{} s", self.0)
        }
    }
}

impl IdleAfter {
    pub(in crate::gui) fn of(idle: &IdleConfig) -> Self {
        Self(idle.threshold_secs)
    }

    /// Returns the offered thresholds, with `current` added if it is not one
    /// of them.
    pub(in crate::gui) fn options(current: Self) -> Vec<Self> {
        let mut options: Vec<_> = IDLE_AFTER_MINUTES
            .iter()
            .map(|minutes| Self(minutes * 60))
            .collect();
        if !options.contains(&current) {
            options.push(current);
            options.sort_unstable_by_key(|option| option.0);
        }
        options
    }
}

impl State {
    /// Loads the settings and starts the local web server if `server` is on.
    pub(in crate::gui) fn load(
        &mut self,
        nirvana: &Arc<Nirvana>,
        idle: Option<IdleConfig>,
        server: bool,
        port: Option<u16>,
    ) -> Task<Message> {
        self.idle = idle;
        self.port = port;
        self.reset_port_input();
        if server {
            self.start_server(nirvana.clone())
        } else {
            Task::none()
        }
    }

    pub(in crate::gui) fn is_available(&self) -> bool {
        self.idle.is_some()
    }

    pub(in crate::gui) fn idle(&self) -> Option<&IdleConfig> {
        self.idle.as_ref()
    }

    pub(in crate::gui) fn is_idle_after_open(&self) -> bool {
        self.idle_after_open
    }

    pub(in crate::gui) fn close_idle_after(&mut self) {
        self.idle_after_open = false;
    }

    pub(in crate::gui) fn port_input(&self) -> &str {
        &self.port_input
    }

    /// Shows the saved port, replacing an empty field or a port that failed
    /// to save.
    pub(in crate::gui) fn reset_port_input(&mut self) {
        self.port_input = self.port.map(|port| port.to_string()).unwrap_or_default();
    }

    pub(in crate::gui) fn is_server_enabled(&self) -> bool {
        self.server.is_some()
    }

    pub(in crate::gui) fn server_status(&self) -> &ServerStatus {
        &self.server_status
    }

    pub(in crate::gui) fn is_web_available(&self) -> bool {
        matches!(self.server_status, ServerStatus::Running(_))
    }

    /// Opens nirvana web in the browser while the server is running.
    fn open_web(&self) -> Task<Message> {
        let ServerStatus::Running(port) = self.server_status else {
            return Task::none();
        };
        let url = web::url(port);
        Task::future(async move {
            let failure = match tokio::task::spawn_blocking(move || opener::open(&url)).await {
                Ok(Ok(())) => return None,
                Ok(Err(e)) => {
                    error!("cannot open nirvana web: {}", chain(&e));
                    Failure::from_error(OPEN_WEB_FAILED, &e)
                }
                Err(e) => {
                    error!("cannot open nirvana web: {e}");
                    Failure::new(OPEN_WEB_FAILED, None)
                }
            };
            Some(Message::Failed(failure))
        })
        .and_then(Task::done)
    }

    /// Saves the typed port unless it is empty or already saved. A running
    /// server keeps its port until it starts again.
    fn save_port_input(&self, nirvana: &Arc<Nirvana>) -> Task<Message> {
        match self.port_input.parse().ok() {
            Some(port) if Some(port) != self.port => save_port(nirvana.clone(), port),
            _ => Task::none(),
        }
    }

    fn start_server(&mut self, nirvana: Arc<Nirvana>) -> Task<Message> {
        let Some(port) = self.port else {
            return Task::none();
        };
        let (stop, stopped) = oneshot::channel();
        self.server = Some(stop);
        self.server_status = ServerStatus::Starting;
        Task::perform(
            async move {
                let (listener, ()) =
                    tokio::join!(web::bind(port), tokio::time::sleep(MIN_STARTING));
                let listener = listener?;
                let port = listener.local_addr()?.port();
                tokio::spawn(async move {
                    let shutdown = async {
                        _ = stopped.await;
                    };
                    if let Err(e) = web::run(listener, nirvana, shutdown).await {
                        error!("web server failed: {e}");
                    }
                });
                Ok(port)
            },
            |started| Message::ServerStarted(started.map_err(Arc::new)),
        )
    }

    pub(in crate::gui) fn update(
        &mut self,
        message: Message,
        nirvana: Option<&Result<Arc<Nirvana>, Failure>>,
    ) -> Task<Message> {
        let Some(Ok(nirvana)) = nirvana else {
            return Task::none();
        };
        match message {
            Message::ServerEnabled(enabled) => {
                let save = save_server(nirvana.clone(), enabled);
                if !enabled {
                    self.server = None;
                    self.server_status = ServerStatus::Off;
                    return save;
                }
                return Task::batch([save, self.start_server(nirvana.clone())]);
            }
            Message::ServerStarted(started) => {
                if self.server.is_some() {
                    self.server_status = match started {
                        Ok(port) => ServerStatus::Running(port),
                        Err(e) => {
                            error!("couldn't start the web server: {e}");
                            ServerStatus::Failed(e.to_string())
                        }
                    };
                }
                return Task::none();
            }
            Message::PortInput(input) => {
                if !is_port_input(&input) {
                    return Task::none();
                }
                self.port_input = input;
                return self.save_port_input(nirvana);
            }
            Message::OpenWeb => return self.open_web(),
            Message::PortSaved(port) => {
                self.port = Some(port);
                return self.save_port_input(nirvana);
            }
            _ => {}
        }
        let Some(idle) = &mut self.idle else {
            return Task::none();
        };
        match message {
            Message::ToggleIdleAfter => self.idle_after_open = !self.idle_after_open,
            Message::CloseIdleAfter => self.idle_after_open = false,
            Message::IdleEnabled(enabled) => {
                idle.enabled = enabled;
                self.idle_after_open &= enabled;
                return save(nirvana.clone(), idle.clone());
            }
            Message::IdleAfter(IdleAfter(secs)) => {
                idle.threshold_secs = secs;
                self.idle_after_open = false;
                return save(nirvana.clone(), idle.clone());
            }
            Message::IdleMethod(method) => {
                idle.methods = toggled(&idle.methods, method);
                return save(nirvana.clone(), idle.clone());
            }
            Message::ServerEnabled(_)
            | Message::ServerStarted(_)
            | Message::PortInput(_)
            | Message::PortSaved(_)
            | Message::OpenWeb
            | Message::Failed(_) => {}
            Message::SaveFailed(e, saved) => {
                error!("{}", chain(e.as_ref()));
                self.port_input = self.port.map(|port| port.to_string()).unwrap_or_default();
                if let Some(saved) = saved {
                    *idle = saved;
                }
                return Task::done(Message::Failed(Failure::from_error(
                    SAVE_FAILED,
                    e.as_ref(),
                )));
            }
        }
        Task::none()
    }
}

/// Returns `methods` with `method` added or removed, in the order of
/// [`IdleMethod::ALL`].
fn toggled(methods: &[IdleMethod], method: IdleMethod) -> Vec<IdleMethod> {
    IdleMethod::ALL
        .into_iter()
        .filter(|candidate| {
            let is_selected = methods.contains(candidate);
            if *candidate == method {
                !is_selected
            } else {
                is_selected
            }
        })
        .collect()
}

fn save(nirvana: Arc<Nirvana>, idle: IdleConfig) -> Task<Message> {
    Task::future(async move {
        match nirvana.set_idle(idle).await {
            Ok(()) => None,
            Err(e) => Some(Message::SaveFailed(Arc::new(e), nirvana.idle().await.ok())),
        }
    })
    .and_then(Task::done)
}

fn save_server(nirvana: Arc<Nirvana>, server: bool) -> Task<Message> {
    Task::future(async move {
        nirvana
            .set_server(server)
            .await
            .err()
            .map(|e| Message::SaveFailed(Arc::new(e), None))
    })
    .and_then(Task::done)
}

fn save_port(nirvana: Arc<Nirvana>, port: u16) -> Task<Message> {
    Task::future(async move {
        match nirvana.set_config(PORT_KEY, &port.to_string()).await {
            Ok(()) => Message::PortSaved(port),
            Err(e) => Message::SaveFailed(Arc::new(e), None),
        }
    })
}

/// Returns whether `input` is empty or a port from 1 to 65535.
fn is_port_input(input: &str) -> bool {
    input.is_empty()
        || (input.bytes().all(|byte| byte.is_ascii_digit())
            && input.parse::<u16>().is_ok_and(|port| port > 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_input_allows_only_valid_ports() {
        for input in ["", "1", "5167", "65535"] {
            assert!(is_port_input(input), "{input}");
        }
        for input in ["0", "65536", "+80", "-1", "80a", " 80"] {
            assert!(!is_port_input(input), "{input}");
        }
    }

    #[tokio::test]
    async fn saved_port_keeps_a_running_server() {
        let dir = tempfile::tempdir().unwrap();
        let nirvana = Some(Ok(Arc::new(Nirvana::open_in(dir.path()).await.unwrap())));
        let mut state = State {
            port: Some(0),
            ..State::default()
        };
        _ = state.update(Message::ServerEnabled(true), nirvana.as_ref());

        _ = state.update(Message::ServerStarted(Ok(5167)), nirvana.as_ref());

        _ = state.update(Message::PortSaved(8080), nirvana.as_ref());

        assert!(matches!(state.server_status(), ServerStatus::Running(5167)));
        assert_eq!(state.port, Some(8080));
    }

    #[tokio::test]
    async fn server_status_follows_the_toggle() {
        let dir = tempfile::tempdir().unwrap();
        let nirvana = Some(Ok(Arc::new(Nirvana::open_in(dir.path()).await.unwrap())));
        let mut state = State {
            port: Some(0),
            ..State::default()
        };

        _ = state.update(Message::ServerEnabled(true), nirvana.as_ref());
        assert!(matches!(state.server_status(), ServerStatus::Starting));
        _ = state.update(Message::ServerStarted(Ok(5167)), nirvana.as_ref());
        assert!(state.is_server_enabled());
        assert!(matches!(state.server_status(), ServerStatus::Running(_)));

        _ = state.update(Message::ServerEnabled(false), nirvana.as_ref());
        let refused = Arc::new(io::Error::from(io::ErrorKind::AddrInUse));
        _ = state.update(Message::ServerStarted(Err(refused)), nirvana.as_ref());
        assert!(!state.is_server_enabled());
        assert!(matches!(state.server_status(), ServerStatus::Off));
    }

    #[tokio::test]
    async fn server_status_shows_the_bind_error() {
        let dir = tempfile::tempdir().unwrap();
        let nirvana = Some(Ok(Arc::new(Nirvana::open_in(dir.path()).await.unwrap())));
        let mut state = State {
            port: Some(0),
            ..State::default()
        };

        _ = state.update(Message::ServerEnabled(true), nirvana.as_ref());
        let refused = Arc::new(io::Error::from(io::ErrorKind::AddrInUse));
        _ = state.update(Message::ServerStarted(Err(refused)), nirvana.as_ref());

        assert!(state.is_server_enabled());
        assert!(matches!(state.server_status(), ServerStatus::Failed(_)));
    }

    #[test]
    fn idle_after_shows_minutes_or_seconds() {
        assert_eq!(IdleAfter(300).to_string(), "5 min");
        assert_eq!(IdleAfter(90).to_string(), "90 s");
    }

    #[test]
    fn idle_after_options_keep_an_offered_threshold() {
        let options = IdleAfter::options(IdleAfter(300));
        assert_eq!(options.len(), IDLE_AFTER_MINUTES.len());
    }

    #[test]
    fn idle_after_options_add_a_custom_threshold_in_order() {
        let options = IdleAfter::options(IdleAfter(90));
        assert_eq!(
            &options[..3],
            [IdleAfter(60), IdleAfter(90), IdleAfter(120)]
        );
    }

    #[test]
    fn toggled_removes_a_selected_method() {
        assert_eq!(
            toggled(&IdleMethod::ALL, IdleMethod::Sleep),
            [IdleMethod::Lock, IdleMethod::Input]
        );
    }

    #[test]
    fn toggled_adds_a_method_in_order() {
        assert_eq!(
            toggled(&[IdleMethod::Input], IdleMethod::Lock),
            [IdleMethod::Lock, IdleMethod::Input]
        );
    }
}
