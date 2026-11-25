use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use chrono::NaiveDate;
use iced::Task;
use nirvana_core::{Nirvana, PublishResult, Slot};
use tracing::{error, info};

use super::listing::group_by_ticket;
use super::toast::Kind;
use crate::gui::failure::{Failure, chain};
use crate::time::{format_duration, local_date};

const LOAD_FAILED: &str = "couldn't load the slots to publish";
const PUBLISH_FAILED: &str = "couldn't publish";

/// The slots a publish would send, shown in the publish modal, and what the
/// publishes made while it is open sent.
#[derive(Default)]
pub(in crate::gui) struct State {
    slots: Option<Vec<Slot>>,
    sending: bool,
    outcome: Option<Outcome>,
}

/// The slots that were sent, and the errors of the slots that failed in the
/// last publish.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::gui) struct Outcome {
    sent: HashSet<i64>,
    failed: HashMap<i64, String>,
}

impl From<PublishResult> for Outcome {
    fn from(result: PublishResult) -> Self {
        Self {
            sent: result.published.iter().map(|slot| slot.model.id).collect(),
            failed: result
                .failed
                .into_iter()
                .map(|failure| (failure.slot.model.id, failure.error))
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::gui) enum Phase {
    Review,
    Sending,
    /// The results of the publishes made while the modal is open.
    Done,
}

/// The slots of one ticket.
pub(in crate::gui) struct Row<'a> {
    pub(in crate::gui) key: &'a str,
    pub(in crate::gui) title: &'a str,
    pub(in crate::gui) slots: usize,
    pub(in crate::gui) seconds: i64,
    pub(in crate::gui) status: Option<Status<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::gui) enum Status<'a> {
    Sending,
    /// Every slot was sent.
    Sent,
    /// Some slots were sent, with the error of a slot that failed.
    Partial(Option<&'a str>),
    /// No slot was sent and a slot failed, with its error.
    Failed(&'a str),
}

/// What the publishes made while the modal was open sent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::gui) struct Summary {
    pub(in crate::gui) sent: usize,
    pub(in crate::gui) failed: usize,
    pub(in crate::gui) total: usize,
    pub(in crate::gui) seconds: i64,
}

#[derive(Clone)]
pub(in crate::gui) enum Message {
    Load,
    Loaded(Vec<Slot>),
    /// Publishes the slots, or retries the failed ones.
    Publish,
    Published(Result<Outcome, Failure>),
    Close,
    /// The modal closes, with what was published while it was open.
    Closed(Option<Summary>),
    Failed(Failure),
}

impl State {
    pub(in crate::gui) fn update(
        &mut self,
        message: Message,
        nirvana: Option<&Result<Arc<Nirvana>, Failure>>,
    ) -> Task<Message> {
        let nirvana = match nirvana {
            Some(Ok(nirvana)) => Some(nirvana.clone()),
            _ => None,
        };
        match message {
            Message::Load => {
                *self = Self::default();
                if let Some(nirvana) = nirvana {
                    return load(nirvana);
                }
            }
            Message::Loaded(slots) => self.slots = Some(slots),
            Message::Publish => {
                if self.can_publish()
                    && let Some(nirvana) = nirvana
                {
                    self.sending = true;
                    return publish(nirvana);
                }
            }
            Message::Published(Ok(outcome)) => {
                self.sending = false;
                let mut sent = self.outcome.take().unwrap_or_default().sent;
                sent.extend(outcome.sent);
                self.outcome = Some(Outcome {
                    sent,
                    failed: outcome.failed,
                });
            }
            Message::Published(Err(failure)) => {
                self.sending = false;
                return Task::done(Message::Failed(failure));
            }
            Message::Close if !self.sending => return Task::done(Message::Closed(self.summary())),
            Message::Close | Message::Closed(_) | Message::Failed(_) => {}
        }
        Task::none()
    }

    pub(in crate::gui) fn phase(&self) -> Phase {
        match &self.outcome {
            _ if self.sending => Phase::Sending,
            Some(_) => Phase::Done,
            None => Phase::Review,
        }
    }

    /// Returns whether there are slots to publish in review, or failed slots
    /// to retry.
    pub(in crate::gui) fn can_publish(&self) -> bool {
        match self.phase() {
            Phase::Review => self.slots.as_ref().is_some_and(|slots| !slots.is_empty()),
            Phase::Sending => false,
            Phase::Done => self
                .outcome
                .as_ref()
                .is_some_and(|outcome| !outcome.failed.is_empty()),
        }
    }

    /// Returns the slots grouped by ticket, the ticket with the most time
    /// first, once they are loaded.
    pub(in crate::gui) fn rows(&self) -> Vec<Row<'_>> {
        let slots: Vec<_> = self.slots.iter().flatten().collect();
        let phase = self.phase();
        group_by_ticket(slots, None, |slot| slot.ticket.id, |slot| slot.elapsed())
            .into_iter()
            .filter_map(|ticket| {
                let &first = ticket.items.first()?;
                let status = match (phase, &self.outcome) {
                    (Phase::Sending, _) => Some(Status::Sending),
                    (_, Some(outcome)) => {
                        let ids: Vec<i64> = ticket.items.iter().map(|slot| slot.model.id).collect();
                        outcome.status(&ids)
                    }
                    _ => None,
                };
                Some(Row {
                    key: &first.ticket.ticket_key,
                    title: &first.ticket.title,
                    slots: ticket.items.len(),
                    seconds: ticket.items.iter().map(|slot| slot.elapsed()).sum(),
                    status,
                })
            })
            .collect()
    }

    /// Returns the number of slots.
    pub(in crate::gui) fn count(&self) -> usize {
        self.slots.as_ref().map_or(0, Vec::len)
    }

    /// Returns the summed seconds of the slots.
    pub(in crate::gui) fn seconds(&self) -> i64 {
        self.slots.iter().flatten().map(Slot::elapsed).sum()
    }

    /// Returns the local days the slots started on, oldest first.
    pub(in crate::gui) fn days(&self) -> Vec<NaiveDate> {
        self.slots
            .iter()
            .flatten()
            .filter_map(|slot| local_date(slot.model.started_at).ok())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Returns what the publishes made while the modal was open sent, or
    /// `None` before the first publish.
    pub(in crate::gui) fn summary(&self) -> Option<Summary> {
        let outcome = self.outcome.as_ref()?;
        let slots = self.slots.as_deref().unwrap_or_default();
        let sent: Vec<_> = slots
            .iter()
            .filter(|slot| outcome.sent.contains(&slot.model.id))
            .collect();
        Some(Summary {
            sent: sent.len(),
            failed: outcome.failed.len(),
            total: slots.len(),
            seconds: sent.iter().map(|slot| slot.elapsed()).sum(),
        })
    }
}

impl Outcome {
    /// Returns the status of a ticket's slots: sent when all were sent,
    /// partial when only some were, and failed when none was and one failed.
    fn status(&self, slot_ids: &[i64]) -> Option<Status<'_>> {
        let error = slot_ids
            .iter()
            .find_map(|id| self.failed.get(id))
            .map(String::as_str);
        let sent = slot_ids.iter().filter(|id| self.sent.contains(id)).count();
        match (sent, error) {
            (0, None) => None,
            (0, Some(error)) => Some(Status::Failed(error)),
            (sent, _) if sent == slot_ids.len() => Some(Status::Sent),
            (_, error) => Some(Status::Partial(error)),
        }
    }
}

impl Summary {
    /// Returns the kind, text and detail of the toast shown when the modal
    /// closes.
    pub(in crate::gui) fn toast(&self) -> (Kind, String, Option<String>) {
        match (self.sent, self.failed) {
            (0, 0) => (Kind::Warning, "nothing to publish".to_owned(), None),
            (sent, 0) => (
                Kind::Success,
                format!(
                    "published {} · {}",
                    slots(sent),
                    format_duration(self.seconds)
                ),
                None,
            ),
            (sent, failed) => (
                Kind::Warning,
                format!(
                    "published {sent} of {} · {failed} failed",
                    slots(self.total)
                ),
                Some("failed slots stay unpublished and are marked × in the list.".to_owned()),
            ),
        }
    }
}

/// Formats `count` as e.g. `3 slots`.
pub(in crate::gui) fn slots(count: usize) -> String {
    match count {
        1 => "1 slot".to_owned(),
        count => format!("{count} slots"),
    }
}

fn load(nirvana: Arc<Nirvana>) -> Task<Message> {
    let listed = async move { nirvana.list_publishable_slots(None, None, None, true).await };
    Task::future(listed).map(|listed| match listed {
        Ok(slots) => Message::Loaded(slots),
        Err(e) => {
            error!("{}", chain(&e));
            Message::Failed(Failure::from_error(LOAD_FAILED, &e))
        }
    })
}

fn publish(nirvana: Arc<Nirvana>) -> Task<Message> {
    Task::future(async move { nirvana.publish_slots(None, None, None, true).await }).map(
        |published| match published {
            Ok(result) => {
                info!(
                    published = result.published.len(),
                    failed = result.failed.len(),
                    "slots published"
                );
                Message::Published(Ok(result.into()))
            }
            Err(e) => {
                error!("{}", chain(&e));
                Message::Published(Err(Failure::from_error(PUBLISH_FAILED, &e)))
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(sent: usize, failed: usize) -> Summary {
        Summary {
            sent,
            failed,
            total: sent + failed,
            seconds: 5400,
        }
    }

    fn outcome(sent: &[i64], failed: &[i64]) -> Outcome {
        Outcome {
            sent: sent.iter().copied().collect(),
            failed: failed
                .iter()
                .map(|id| (*id, "timed out".to_owned()))
                .collect(),
        }
    }

    #[test]
    fn slots_uses_singular_for_one() {
        assert_eq!(slots(1), "1 slot");
        assert_eq!(slots(3), "3 slots");
    }

    #[test]
    fn toast_warns_when_nothing_was_sent() {
        assert_eq!(
            summary(0, 0).toast(),
            (Kind::Warning, "nothing to publish".to_owned(), None)
        );
    }

    #[test]
    fn toast_shows_sent_logs_and_time() {
        assert_eq!(
            summary(3, 0).toast(),
            (Kind::Success, "published 3 slots · 1h 30m".to_owned(), None)
        );
    }

    #[test]
    fn toast_warns_about_failed_slots() {
        let (kind, text, detail) = summary(2, 1).toast();
        assert_eq!(kind, Kind::Warning);
        assert_eq!(text, "published 2 of 3 slots · 1 failed");
        assert!(detail.is_some());
    }

    #[test]
    fn nothing_can_be_published_before_the_slots_load() {
        let state = State::default();
        assert_eq!(state.phase(), Phase::Review);
        assert!(!state.can_publish());
    }

    #[test]
    fn nothing_can_be_published_while_sending() {
        let state = State {
            sending: true,
            ..State::default()
        };
        assert_eq!(state.phase(), Phase::Sending);
        assert!(!state.can_publish());
    }

    #[test]
    fn failed_slots_stay_on_the_results() {
        let mut state = State::default();
        let _ = state.update(Message::Published(Ok(outcome(&[1], &[2]))), None);
        assert_eq!(state.phase(), Phase::Done);
        assert!(state.can_publish());
    }

    #[test]
    fn retry_keeps_the_slots_sent_before() {
        let mut state = State {
            outcome: Some(outcome(&[1], &[2, 3])),
            ..State::default()
        };
        let _ = state.update(Message::Published(Ok(outcome(&[2], &[3]))), None);
        assert_eq!(state.outcome, Some(outcome(&[1, 2], &[3])));
    }

    #[test]
    fn status_is_failed_when_no_slot_was_sent() {
        let outcome = outcome(&[], &[2]);
        assert_eq!(outcome.status(&[2, 3]), Some(Status::Failed("timed out")));
    }

    #[test]
    fn status_is_partial_with_the_error_when_some_slots_failed() {
        let outcome = outcome(&[1], &[2]);
        assert_eq!(
            outcome.status(&[1, 2]),
            Some(Status::Partial(Some("timed out")))
        );
    }

    #[test]
    fn status_is_sent_only_when_every_slot_was_sent() {
        let outcome = outcome(&[1, 2], &[]);
        assert_eq!(outcome.status(&[1, 2]), Some(Status::Sent));
        assert_eq!(outcome.status(&[1, 3]), Some(Status::Partial(None)));
        assert_eq!(outcome.status(&[3]), None);
    }

    #[test]
    fn a_publish_without_failures_stays_on_the_results() {
        let mut state = State::default();
        let _ = state.update(Message::Published(Ok(outcome(&[1], &[]))), None);
        assert_eq!(state.phase(), Phase::Done);
        assert!(!state.can_publish());
    }
}
