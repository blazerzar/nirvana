#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use std::time::Duration;

use chrono::Utc;
use iced::Subscription;
use iced::futures::{Stream, stream};
#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
use nirvana_core::{IdleConfig, IdleMethod};
use tracing::{debug, trace};
#[cfg(target_os = "windows")]
use windows as platform;

const POLL_INTERVAL: Duration = Duration::from_secs(1);
const SUPPORTED: bool = cfg!(any(target_os = "macos", target_os = "windows"));

/// A period the user was idle for, in seconds since the epoch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::gui) struct Period {
    pub(in crate::gui) from: i64,
    pub(in crate::gui) to: i64,
}

/// Checks every second whether the user is present and reports each period
/// of at least the threshold they were idle for, detected by the methods of
/// `config`. Nothing is reported on Linux yet.
pub(in crate::gui) fn subscription(config: &IdleConfig) -> Subscription<Period> {
    if SUPPORTED {
        Subscription::run_with(config.clone(), poll)
    } else {
        Subscription::none()
    }
}

fn poll(config: &IdleConfig) -> impl Stream<Item = Period> + use<> {
    trace!(
        threshold_secs = config.threshold_secs,
        methods = ?config.methods,
        "idle detection started"
    );
    let detector = Detector::new(config.clone(), Utc::now().timestamp());
    stream::unfold(detector, |mut detector| async move {
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            let now = Utc::now().timestamp();
            if let Some(period) =
                detector.sample(now, platform::is_locked(), platform::idle_seconds())
            {
                return Some((period, detector));
            }
        }
    })
}

/// When the user went idle, and when they were last checked on. The wall
/// clock is used, because monotonic clocks stop while the computer sleeps.
struct Detector {
    config: IdleConfig,
    idle_since: Option<i64>,
    last_sample: i64,
}

impl Detector {
    fn new(config: IdleConfig, now: i64) -> Self {
        Self {
            config,
            idle_since: None,
            last_sample: now,
        }
    }

    /// Takes whether the screen is `locked` and the seconds since the last
    /// input at `now`. Returns the idle period once the user is back, which
    /// is when the screen is unlocked and there was input within the
    /// threshold, if it lasted at least the threshold.
    fn sample(&mut self, now: i64, locked: bool, idle_secs: u64) -> Option<Period> {
        let threshold = i64::try_from(self.config.threshold_secs).unwrap_or(i64::MAX);
        let idle_secs = i64::try_from(idle_secs).unwrap_or(i64::MAX);
        let gap = now.saturating_sub(self.last_sample);
        self.last_sample = now;
        let no_input = idle_secs >= threshold;

        if self.idle_since.is_none() {
            let went_idle = [
                (IdleMethod::Lock, locked, now),
                (IdleMethod::Sleep, gap >= threshold, now.saturating_sub(gap)),
                (IdleMethod::Input, no_input, now.saturating_sub(idle_secs)),
            ]
            .into_iter()
            .find(|(method, detected, _)| *detected && self.config.methods.contains(method));
            if let Some((method, _, since)) = went_idle {
                debug!(?method, since, "idle detected");
                self.idle_since = Some(since);
            }
        }

        let since = self.idle_since?;
        if locked || no_input {
            return None;
        }
        self.idle_since = None;
        if now - since < threshold {
            trace!(since, now, "idle shorter than the threshold, ignored");
            return None;
        }
        debug!(since, now, "back from idle");
        Some(Period {
            from: since,
            to: now,
        })
    }
}

impl Drop for Detector {
    fn drop(&mut self) {
        trace!(
            threshold_secs = self.config.threshold_secs,
            methods = ?self.config.methods,
            "idle detection stopped"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const THRESHOLD: u64 = 300;

    fn detector(methods: &[IdleMethod], now: i64) -> Detector {
        let config = IdleConfig {
            enabled: true,
            methods: methods.to_vec(),
            threshold_secs: THRESHOLD,
        };
        Detector::new(config, now)
    }

    #[test]
    fn reports_a_lock_once_unlocked() {
        let mut detector = detector(&IdleMethod::ALL, 0);

        assert_eq!(detector.sample(1, true, 0), None);
        assert_eq!(detector.sample(400, true, 0), None);
        assert_eq!(
            detector.sample(401, false, 0),
            Some(Period { from: 1, to: 401 })
        );
    }

    #[test]
    fn ignores_a_lock_shorter_than_the_threshold() {
        let mut detector = detector(&IdleMethod::ALL, 0);

        assert_eq!(detector.sample(1, true, 0), None);
        assert_eq!(detector.sample(2, false, 0), None);
        assert_eq!(detector.sample(3, false, 0), None);
    }

    #[test]
    fn reports_no_input_from_the_last_input() {
        let mut detector = detector(&IdleMethod::ALL, 599);

        assert_eq!(detector.sample(600, false, THRESHOLD), None);
        assert_eq!(
            detector.sample(601, false, 0),
            Some(Period { from: 300, to: 601 })
        );
    }

    #[test]
    fn reports_sleep_from_the_last_sample() {
        let mut detector = detector(&IdleMethod::ALL, 0);

        assert_eq!(detector.sample(1, false, 0), None);
        assert_eq!(
            detector.sample(500, false, 0),
            Some(Period { from: 1, to: 500 })
        );
    }

    #[test]
    fn ignores_methods_that_are_not_selected() {
        let mut detector = detector(&[IdleMethod::Input], 0);

        assert_eq!(detector.sample(1, true, 0), None);
        assert_eq!(detector.sample(500, false, 0), None);
    }
}
