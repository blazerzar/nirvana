use chrono::Utc;
use nirvana_entity::{publish_entry, slot, ticket};

#[derive(Debug)]
pub struct Slot {
    pub model: slot::Model,
    pub ticket: ticket::Model,
}

impl Slot {
    /// Returns the seconds from the start to the stop, or to now while the
    /// slot is running. A start after the stop or now gives zero.
    pub fn elapsed(&self) -> i64 {
        elapsed(
            self.model.started_at,
            self.model.stopped_at,
            Utc::now().timestamp(),
        )
    }
}

fn elapsed(started_at: i64, stopped_at: Option<i64>, now: i64) -> i64 {
    (stopped_at.unwrap_or(now) - started_at).max(0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishState {
    Unpublished,
    Pending,
    Failed,
    Published,
}

impl PublishState {
    /// Derives the state of a slot from its publish entries. A pending entry
    /// claimed before `stuck_before` counts as failed.
    pub fn from_entries(entries: &[publish_entry::Model], stuck_before: i64) -> Self {
        if entries.is_empty() {
            Self::Unpublished
        } else if entries
            .iter()
            .any(|entry| entry.is_pending() && !entry.is_stuck(stuck_before))
        {
            Self::Pending
        } else if entries.iter().all(|entry| entry.published_at.is_some()) {
            Self::Published
        } else {
            Self::Failed
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SlotTarget {
    Id(i64),
    At(i64),
}

#[derive(Debug)]
pub struct StartResult {
    pub started: Slot,
    pub stopped: Option<Slot>,
}

#[derive(Debug, Default)]
pub struct PublishResult {
    pub published: Vec<Slot>,
    pub failed: Vec<PublishFailure>,
}

#[derive(Debug)]
pub struct PublishFailure {
    pub slot: Slot,
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_of_running_slot_counts_to_now() {
        assert_eq!(elapsed(1000, None, 1600), 600);
    }

    #[test]
    fn elapsed_of_stopped_slot_counts_to_stop() {
        assert_eq!(elapsed(1000, Some(2800), 5000), 1800);
    }

    #[test]
    fn elapsed_of_slot_started_in_future_is_zero() {
        assert_eq!(elapsed(1060, None, 1000), 0);
    }

    fn entry(
        published_at: Option<i64>,
        error: Option<&str>,
        claimed_at: i64,
    ) -> publish_entry::Model {
        publish_entry::Model {
            slot_id: 1,
            connection_id: 1,
            published_at,
            error: error.map(str::to_string),
            claimed_at,
        }
    }

    #[test]
    fn slot_without_entries_is_unpublished() {
        assert_eq!(
            PublishState::from_entries(&[], 1000),
            PublishState::Unpublished
        );
    }

    #[test]
    fn slot_with_recently_claimed_entry_is_pending() {
        let entries = [entry(None, None, 1000)];
        assert_eq!(
            PublishState::from_entries(&entries, 1000),
            PublishState::Pending
        );
    }

    #[test]
    fn slot_with_stuck_entry_is_failed() {
        let entries = [entry(None, None, 999)];
        assert_eq!(
            PublishState::from_entries(&entries, 1000),
            PublishState::Failed
        );
    }

    #[test]
    fn slot_with_failed_entry_is_failed() {
        let entries = [entry(None, Some("error"), 1000)];
        assert_eq!(
            PublishState::from_entries(&entries, 1000),
            PublishState::Failed
        );
    }

    #[test]
    fn slot_with_ok_entries_is_published() {
        let entries = [entry(Some(1100), None, 1000), entry(Some(1200), None, 1000)];
        assert_eq!(
            PublishState::from_entries(&entries, 1000),
            PublishState::Published
        );
    }
}
