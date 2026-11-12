use chrono::Utc;
use nirvana_entity::{slot, ticket};

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

#[derive(Debug)]
pub struct StartResult {
    pub started: Slot,
    pub stopped: Option<Slot>,
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
}
