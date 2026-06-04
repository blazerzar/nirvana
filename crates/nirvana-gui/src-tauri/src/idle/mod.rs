#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::Emitter;
use tauri::Manager;

const POLL_INTERVAL: Duration = Duration::from_secs(1);

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct IdleFlags: u8 {
        const LOCK  = 0b0001;
        const SLEEP = 0b0010;
        const INPUT = 0b0100;
    }
}

impl Default for IdleFlags {
    fn default() -> Self {
        Self::all()
    }
}

fn fmt_time(secs: i64) -> String {
    let time_of_day = secs % 86400;
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    let second = time_of_day % 60;
    format!("{:02}:{:02}:{:02}", hour, minute, second)
}

pub struct IdlePeriod {
    pub from: i64,
    pub to: i64,
}

pub struct IdleTracker {
    periods: Arc<Mutex<Vec<IdlePeriod>>>,
    stop_flag: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl IdleTracker {
    pub fn new() -> Self {
        Self {
            periods: Arc::new(Mutex::new(Vec::new())),
            stop_flag: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }

    pub fn start(&mut self, app: tauri::AppHandle, flags: IdleFlags, threshold_secs: u64) {
        if self.handle.is_some() {
            return;
        }

        self.stop_flag.store(false, Ordering::Relaxed);
        let periods = self.periods.clone();
        let stop_flag = self.stop_flag.clone();

        self.handle = Some(thread::spawn(move || {
            let mut idle_started_at: Option<i64> = None;
            let mut last_poll = SystemTime::now();

            loop {
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }

                thread::sleep(POLL_INTERVAL);

                let now = SystemTime::now();
                let poll_gap_secs = now.duration_since(last_poll).unwrap_or_default().as_secs();

                let locked = is_locked();
                let was_suspended = poll_gap_secs >= threshold_secs;
                let idle_secs = get_idle_seconds();
                let no_input = idle_secs >= threshold_secs;

                let now_secs = now
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                // Start idle using any of the methods
                if idle_started_at.is_none() {
                    let triggered = match () {
                        _ if flags.contains(IdleFlags::LOCK) && locked => {
                            Some((IdleFlags::LOCK, now_secs))
                        }
                        _ if flags.contains(IdleFlags::SLEEP) && was_suspended => Some((
                            IdleFlags::INPUT,
                            now_secs.saturating_sub(poll_gap_secs as i64),
                        )),
                        _ if flags.contains(IdleFlags::INPUT) && no_input => {
                            Some((IdleFlags::INPUT, now_secs.saturating_sub(idle_secs as i64)))
                        }
                        _ => None,
                    };

                    if let Some((trigger, start)) = triggered {
                        idle_started_at = Some(start);
                        eprintln!("[idle] went idle ({:?}) at {}", trigger, fmt_time(start));
                    }
                }

                // End idle when user is active and computer is unlocked
                if let Some(start) = idle_started_at {
                    let end = now_secs;

                    if !locked && !no_input {
                        eprintln!(
                            "[idle] returned, idle period: {}..{} ({}s)",
                            fmt_time(start),
                            fmt_time(end),
                            end - start
                        );
                        if end - start >= threshold_secs as i64 {
                            periods.lock().unwrap().push(IdlePeriod {
                                from: start,
                                to: end,
                            });
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                            let _ = app.emit("idle-period-ended", ());
                        }
                        idle_started_at = None;
                    }
                }

                last_poll = now;
            }
        }));
    }

    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    pub fn drain_periods(&self) -> Vec<IdlePeriod> {
        self.periods.lock().unwrap().drain(..).collect()
    }
}

#[cfg(target_os = "macos")]
fn get_idle_seconds() -> u64 {
    macos::get_idle_seconds()
}
#[cfg(target_os = "windows")]
fn get_idle_seconds() -> u64 {
    windows::get_idle_seconds()
}

#[cfg(target_os = "macos")]
fn is_locked() -> bool {
    macos::is_locked()
}
#[cfg(target_os = "windows")]
fn is_locked() -> bool {
    windows::is_locked()
}
