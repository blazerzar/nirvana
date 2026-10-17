//! Log file setup.
//!
//! `nirvana-core` emits events with `tracing` and exposes the log file as a
//! [`Layer`]. The binary installs it.
//!
//! # Levels
//!
//! - `error`, `warn`
//! - `info`: user issued commands and state changes
//! - `debug`: everything else, enabled in release
//! - `trace`: dev only, compiled out in release builds
//!
//! # Filter
//!
//! Defaults to `warn,nirvana=debug,nirvana_core=debug` (`trace` in dev mode).
//! `NIRVANA_LOG` overrides it with a full filter directive.

use std::env;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::Arc;

use tracing::Subscriber;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::time::ChronoLocal;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, Layer};

use crate::paths::AppPaths;

const LOG_ENV_VAR: &str = "NIRVANA_LOG";

/// Returns a layer that appends events to `nirvana.log`, filtered by the
/// default filter or `NIRVANA_LOG`.
///
/// Creates the log directory if it does not exist. The binary installs the
/// layer once at startup, before any other `nirvana-core` call, so DB and
/// config migrations are logged:
///
/// ```no_run
/// use tracing_subscriber::layer::SubscriberExt;
/// use tracing_subscriber::util::SubscriberInitExt;
///
/// match nirvana_core::logs::file_layer() {
///     Ok(layer) => tracing_subscriber::registry().with(layer).init(),
///     Err(e) => eprintln!("logging disabled: {e}"),
/// }
/// ```
///
/// # Errors
///
/// Returns an error if the log directory cannot be created or the log file
/// cannot be opened.
pub fn file_layer<S>() -> io::Result<impl Layer<S>>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let paths = AppPaths::resolve();
    let file = open_log_file(&paths.log_file)?;
    let filter = filter(|key| env::var(key).ok(), paths.is_dev);
    Ok(fmt_layer(Arc::new(file)).with_filter(filter))
}

fn fmt_layer<S, W>(writer: W) -> impl Layer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: for<'w> MakeWriter<'w> + Send + Sync + 'static,
{
    tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_ansi(false)
        .with_timer(ChronoLocal::new("%Y-%m-%d %H:%M:%S".to_string()))
}

fn open_log_file(path: &Path) -> io::Result<File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    OpenOptions::new().create(true).append(true).open(path)
}

fn filter(var: impl Fn(&str) -> Option<String>, is_dev: bool) -> EnvFilter {
    let level = if is_dev { "trace" } else { "debug" };
    let directives = var(LOG_ENV_VAR)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("warn,nirvana={level},nirvana_core={level}"));
    EnvFilter::new(directives)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::sync::Mutex;

    use tracing::info;
    use tracing_subscriber::layer::SubscriberExt;

    use super::*;

    #[derive(Clone, Default)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);

    impl Buffer {
        fn contents(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    impl Write for Buffer {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let vars: Vec<(String, String)> = vars
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        move |key| {
            vars.iter()
                .find(|(k, _)| k == key)
                .map(|(_, value)| value.clone())
        }
    }

    #[test]
    fn writes_one_plain_line_per_event() {
        let buffer = Buffer::default();
        let writer = buffer.clone();
        let subscriber = tracing_subscriber::registry().with(fmt_layer(move || writer.clone()));

        tracing::subscriber::with_default(subscriber, || {
            info!(slot_id = 1, "slot started");
        });

        let output = buffer.contents();
        assert_eq!(output.lines().count(), 1);
        assert!(output.contains(" INFO "));
        assert!(output.contains("slot started slot_id=1"));
        assert!(!output.contains('\x1b'));
    }

    #[test]
    fn filter_defaults_follow_dev_mode() {
        assert!(
            filter(env(&[]), true)
                .to_string()
                .contains("nirvana_core=trace")
        );
        assert!(
            filter(env(&[]), false)
                .to_string()
                .contains("nirvana_core=debug")
        );
    }

    #[test]
    fn filter_uses_env_override() {
        assert_eq!(
            filter(env(&[(LOG_ENV_VAR, "error")]), true).to_string(),
            "error"
        );
    }

    #[test]
    fn filter_ignores_empty_env_override() {
        assert_eq!(
            filter(env(&[(LOG_ENV_VAR, "")]), false).to_string(),
            filter(env(&[]), false).to_string()
        );
    }

    #[test]
    fn open_log_file_creates_dir_and_appends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing").join("nirvana.log");

        writeln!(open_log_file(&path).unwrap(), "first").unwrap();
        writeln!(open_log_file(&path).unwrap(), "second").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "first\nsecond\n");
    }
}
