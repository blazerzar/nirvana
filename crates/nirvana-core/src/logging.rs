//! Log file setup.
//!
//! `nirvana-core` emits events with `tracing` and exposes the log file as a
//! [`Layer`]. The binary installs it.
//!
//! # Levels
//!
//! - `error`, `warn`
//! - `info`: user issued commands and state changes
//! - `debug`: everything else
//! - `trace`: dev only, compiled out in release builds
//!
//! # Filter
//!
//! Only our crates are logged, at `[log] level` from `config.toml`, or
//! `info` (`trace` in dev mode) when it is not set. `NIRVANA_LOG` overrides it
//! with a full filter directive, which can also enable dependencies.

use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::Arc;
use std::{env, io};

use tracing::Subscriber;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::time::ChronoLocal;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, Layer};

use crate::config::{self, LogLevel};
use crate::paths::AppPaths;

const LOG_ENV_VAR: &str = "NIRVANA_LOG";
const CRATES: [&str; 4] = [
    "nirvana",
    "nirvana_core",
    "nirvana_migration",
    "nirvana_entity",
];

/// Returns a layer that appends events to `nirvana.log`, filtered by
/// `NIRVANA_LOG`, `[log] level` from `config.toml` or the default filter, in
/// that order.
///
/// Creates the log directory if it does not exist. The binary installs the
/// layer once at startup, before any other `nirvana-core` call, so DB and
/// config migrations are logged:
///
/// ```no_run
/// use tracing_subscriber::layer::SubscriberExt;
/// use tracing_subscriber::util::SubscriberInitExt;
///
/// match nirvana_core::logging::file_layer() {
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
    let paths = AppPaths::resolve()?;
    let file = open_log_file(&paths.log_file)?;
    let filter = filter(
        |key| env::var(key).ok(),
        paths.is_dev,
        config::log_level(&paths),
    );
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

fn filter(
    var: impl Fn(&str) -> Option<String>,
    is_dev: bool,
    level: Option<LogLevel>,
) -> EnvFilter {
    let level = match level {
        Some(level) => level.as_str(),
        None if is_dev => "trace",
        None => "info",
    };
    let directives = var(LOG_ENV_VAR)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| crate_directives(level));
    EnvFilter::new(directives)
}

/// Returns a filter directive that enables `level` for our crates only.
fn crate_directives(level: &str) -> String {
    CRATES.map(|name| format!("{name}={level}")).join(",")
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
        assert_eq!(
            filter(env(&[]), true, None).to_string(),
            EnvFilter::new(crate_directives("trace")).to_string()
        );
        assert_eq!(
            filter(env(&[]), false, None).to_string(),
            EnvFilter::new(crate_directives("info")).to_string()
        );
    }

    #[test]
    fn filter_uses_config_level_over_dev_mode() {
        assert_eq!(
            filter(env(&[]), true, Some(LogLevel::Warn)).to_string(),
            EnvFilter::new(crate_directives("warn")).to_string()
        );
    }

    #[test]
    fn crate_directives_cover_only_our_crates() {
        assert_eq!(
            crate_directives("info"),
            "nirvana=info,nirvana_core=info,nirvana_migration=info,nirvana_entity=info"
        );
    }

    #[test]
    fn filter_uses_env_override() {
        assert_eq!(
            filter(env(&[(LOG_ENV_VAR, "error")]), true, Some(LogLevel::Debug)).to_string(),
            "error"
        );
    }

    #[test]
    fn filter_ignores_empty_env_override() {
        assert_eq!(
            filter(env(&[(LOG_ENV_VAR, "")]), false, None).to_string(),
            filter(env(&[]), false, None).to_string()
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
