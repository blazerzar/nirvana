//! Idle detection is not supported on this platform yet, so the subscription
//! never polls these.

pub(super) fn idle_seconds() -> u64 {
    0
}

pub(super) fn is_locked() -> bool {
    false
}
