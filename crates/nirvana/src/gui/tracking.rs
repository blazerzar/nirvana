use std::sync::Arc;

use iced::Task;
use nirvana_core::{Nirvana, NirvanaError, Slot};
use tracing::{error, info};

use super::failure::{Failure, chain};

const LOAD_FAILED: &str = "couldn't load tracking";
const STOP_FAILED: &str = "couldn't stop tracking";
const RESUME_FAILED: &str = "couldn't resume tracking";
const SWITCH_FAILED: &str = "couldn't switch back";

/// The running slot and the ticket key that resume or switch starts.
#[derive(Default)]
pub(super) struct State {
    running: Option<Slot>,
    previous: Option<String>,
}

#[derive(Clone)]
pub(super) enum Message {
    Load,
    Stop,
    Resume,
    Switch,
    Loaded(Option<Slot>, Option<String>),
    /// A tracking change finished, with the text of its success toast.
    Changed(Result<Option<String>, Failure>),
    Succeeded(String),
    Failed(Failure),
}

impl State {
    pub(super) fn is_tracking(&self) -> bool {
        self.running.is_some()
    }

    /// Returns the ticket key that resume starts while not tracking.
    pub(super) fn resumable(&self) -> Option<&str> {
        self.previous.as_deref().filter(|_| !self.is_tracking())
    }

    /// Returns the ticket key that switch starts while tracking.
    pub(super) fn switchable(&self) -> Option<&str> {
        self.previous.as_deref().filter(|_| self.is_tracking())
    }

    pub(super) fn update(
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
            Message::Loaded(running, previous) => {
                self.running = running;
                self.previous = previous;
            }
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
    Task::future(async move {
        let running = nirvana.running_slot().await?;
        let previous = nirvana.previous_ticket().await?;
        Ok((running, previous.map(|ticket| ticket.ticket_key)))
    })
    .map(|loaded| match loaded {
        Ok((running, previous)) => Message::Loaded(running, previous),
        Err(e) => Message::Failed(failure(LOAD_FAILED, &e)),
    })
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
fn clock(seconds: i64) -> String {
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
}
