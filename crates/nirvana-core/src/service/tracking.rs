//! Slots.
//!
//! A slot is a span of time tracked on a ticket.

use chrono::Utc;

use crate::errors::DomainError;

#[cfg_attr(not(test), expect(dead_code))]
fn reject_future_time(timestamp: i64) -> Result<(), DomainError> {
    if timestamp > Utc::now().timestamp() {
        return Err(DomainError::FutureTime);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_future_time_accepts_past_and_present() {
        let now = Utc::now().timestamp();
        assert!(reject_future_time(now - 60).is_ok());
        assert!(reject_future_time(now).is_ok());
    }

    #[test]
    fn reject_future_time_rejects_future() {
        let now = Utc::now().timestamp();
        assert!(matches!(
            reject_future_time(now + 60),
            Err(DomainError::FutureTime)
        ));
    }
}
