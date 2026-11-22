use std::sync::Arc;

use chrono::Utc;
use iced::{Subscription, Task, window};
use nirvana_core::{Nirvana, NirvanaError, Slot};
use tracing::{error, info};

use crate::gui::failure::{Failure, chain};
use crate::time::today_start;

const LOAD_FAILED: &str = "couldn't load tracking";
const STOP_FAILED: &str = "couldn't stop tracking";
const RESUME_FAILED: &str = "couldn't resume tracking";
const SWITCH_FAILED: &str = "couldn't switch back";
const PULSE_PERIOD_MS: i64 = 2000;
const PULSE_MIN_OPACITY: f32 = 0.35;

/// The running slot, the last stopped slot of today, the seconds of the
/// stopped slots of today and the ticket key that resume or switch starts.
#[derive(Default)]
pub(in crate::gui) struct State {
    running: Option<Slot>,
    last: Option<Slot>,
    stopped_today: i64,
    today: i64,
    previous: Option<String>,
    now_ms: i64,
}

/// The tracking state read from the database.
#[derive(Clone)]
pub(in crate::gui) struct Loaded {
    running: Option<Slot>,
    last: Option<Slot>,
    stopped_today: i64,
    today: i64,
    previous: Option<String>,
}

#[derive(Clone)]
pub(in crate::gui) enum Message {
    Load,
    Stop,
    Resume,
    Switch,
    Loaded(Box<Loaded>),
    Frame,
    /// A tracking change finished, with the text of its success toast.
    Changed(Result<Option<String>, Failure>),
    Succeeded(String),
    Failed(Failure),
}

impl State {
    pub(in crate::gui) fn is_tracking(&self) -> bool {
        self.running.is_some()
    }

    pub(in crate::gui) fn running(&self) -> Option<&Slot> {
        self.running.as_ref()
    }

    pub(in crate::gui) fn last(&self) -> Option<&Slot> {
        self.last.as_ref()
    }

    /// Returns the current timestamp in seconds, kept up to date while
    /// tracking.
    pub(in crate::gui) fn now(&self) -> i64 {
        self.now_ms / 1000
    }

    /// Returns the seconds the running slot has been running for.
    pub(in crate::gui) fn elapsed(&self) -> i64 {
        self.running
            .as_ref()
            .map_or(0, |running| (self.now() - running.model.started_at).max(0))
    }

    /// Returns the seconds tracked today, with the running slot when it
    /// started today.
    pub(in crate::gui) fn today_total(&self) -> i64 {
        let running = self
            .running
            .as_ref()
            .filter(|running| running.model.started_at >= self.today)
            .map_or(0, |_| self.elapsed());
        self.stopped_today + running
    }

    pub(in crate::gui) fn pulse_opacity(&self) -> f32 {
        pulse_opacity(self.now_ms)
    }

    /// Redraws on every frame while tracking, for the timer and the pulse.
    pub(in crate::gui) fn subscription(&self) -> Subscription<Message> {
        match self.running {
            Some(_) => window::frames().map(|_| Message::Frame),
            None => Subscription::none(),
        }
    }

    /// Returns the ticket key that resume starts while not tracking.
    pub(in crate::gui) fn resumable(&self) -> Option<&str> {
        self.previous.as_deref().filter(|_| !self.is_tracking())
    }

    /// Returns the ticket key that switch starts while tracking.
    pub(in crate::gui) fn switchable(&self) -> Option<&str> {
        self.previous.as_deref().filter(|_| self.is_tracking())
    }

    pub(in crate::gui) fn update(
        &mut self,
        message: Message,
        nirvana: Option<&Result<Arc<Nirvana>, Failure>>,
    ) -> Task<Message> {
        let Some(Ok(nirvana)) = nirvana else {
            return Task::none();
        };
        let nirvana = nirvana.clone();
        match message {
            Message::Load => return load(nirvana),
            Message::Stop if self.is_tracking() => {
                return change(STOP_FAILED, async move {
                    let stopped = nirvana.stop_slot(None).await?;
                    Ok(stopped.map(|slot| {
                        info!(
                            key = %slot.ticket.ticket_key,
                            slot_id = slot.model.id,
                            stopped_at = slot.model.stopped_at,
                            "slot stopped");
                        format!(
                            "stopped {} · {}",
                            slot.ticket.ticket_key,
                            clock(slot.elapsed())
                        )
                    }))
                });
            }
            Message::Resume if self.resumable().is_some() => {
                return change(RESUME_FAILED, async move {
                    let slot = nirvana.resume_slot(None).await?;
                    info!(
                        key = %slot.ticket.ticket_key,
                        slot_id = slot.model.id,
                        started_at = slot.model.started_at,
                        "slot resumed");
                    Ok(None)
                });
            }
            Message::Switch if self.switchable().is_some() => {
                return change(SWITCH_FAILED, async move {
                    let switched = nirvana.switch_slot(None).await?;
                    let slot = &switched.started;
                    info!(
                        key = %slot.ticket.ticket_key,
                        slot_id = slot.model.id,
                        started_at = slot.model.started_at,
                        "slot switched");
                    Ok(Some(format!("switched back to {}", slot.ticket.ticket_key)))
                });
            }
            Message::Stop | Message::Resume | Message::Switch => {}
            Message::Loaded(loaded) => {
                let Loaded {
                    running,
                    last,
                    stopped_today,
                    today,
                    previous,
                } = *loaded;
                self.running = running;
                self.last = last;
                self.stopped_today = stopped_today;
                self.today = today;
                self.previous = previous;
                self.now_ms = Utc::now().timestamp_millis();
            }
            Message::Frame => self.now_ms = Utc::now().timestamp_millis(),
            Message::Changed(changed) => {
                let reload = load(nirvana);
                return match changed {
                    Ok(Some(text)) => reload.chain(Task::done(Message::Succeeded(text))),
                    Ok(None) => reload,
                    Err(failure) => reload.chain(Task::done(Message::Failed(failure))),
                };
            }
            Message::Succeeded(_) | Message::Failed(_) => {}
        }
        Task::none()
    }
}

fn load(nirvana: Arc<Nirvana>) -> Task<Message> {
    let today = match today_start() {
        Ok(today) => today,
        Err(e) => {
            error!("{e:#}");
            return Task::done(Message::Failed(Failure::new(LOAD_FAILED, None)));
        }
    };
    Task::future(async move {
        let running = nirvana.running_slot().await?;
        let stopped: Vec<Slot> = nirvana
            .list_slots(today, None, false)
            .await?
            .into_iter()
            .map(|(slot, _)| slot)
            .filter(|slot| slot.model.stopped_at.is_some())
            .collect();
        let previous = nirvana.previous_ticket().await?;
        Ok(Loaded {
            running,
            stopped_today: stopped.iter().map(Slot::elapsed).sum(),
            last: stopped.into_iter().max_by_key(|slot| slot.model.stopped_at),
            today,
            previous: previous.map(|ticket| ticket.ticket_key),
        })
    })
    .map(|loaded| match loaded {
        Ok(loaded) => Message::Loaded(Box::new(loaded)),
        Err(e) => Message::Failed(failure(LOAD_FAILED, &e)),
    })
}

/// Fades from full opacity to the minimum and back once per period, easing
/// in and out.
fn pulse_opacity(now_ms: i64) -> f32 {
    let phase = now_ms.rem_euclid(PULSE_PERIOD_MS) as f32 / PULSE_PERIOD_MS as f32;
    let fade = 1.0 - (2.0 * phase - 1.0).abs();
    let eased = fade * fade * (3.0 - 2.0 * fade);
    1.0 - (1.0 - PULSE_MIN_OPACITY) * eased
}

/// Runs a tracking change that returns the text of its success toast.
fn change(
    text: &'static str,
    run: impl Future<Output = Result<Option<String>, NirvanaError>> + Send + 'static,
) -> Task<Message> {
    Task::future(run).map(move |changed| Message::Changed(changed.map_err(|e| failure(text, &e))))
}

fn failure(text: &'static str, error: &NirvanaError) -> Failure {
    error!("{}", chain(error));
    Failure::from_error(text, error)
}

/// Formats `seconds` as `h:mm:ss`.
pub(in crate::gui) fn clock(seconds: i64) -> String {
    format!(
        "{}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_pads_minutes_and_seconds() {
        assert_eq!(clock(23 * 60 + 41), "0:23:41");
        assert_eq!(clock(12 * 3600 + 5), "12:00:05");
    }

    #[test]
    fn pulse_is_opaque_at_the_start_of_a_period() {
        assert_eq!(pulse_opacity(0), 1.0);
        assert_eq!(pulse_opacity(PULSE_PERIOD_MS), 1.0);
    }

    #[test]
    fn pulse_is_faintest_in_the_middle_of_a_period() {
        assert!((pulse_opacity(PULSE_PERIOD_MS / 2) - PULSE_MIN_OPACITY).abs() < f32::EPSILON);
    }
}
