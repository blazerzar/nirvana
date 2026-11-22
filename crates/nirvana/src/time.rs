use anyhow::Context;
use chrono::{Local, NaiveTime};

/// Returns the Unix timestamp of today 00:00 in local time.
pub(crate) fn today_start() -> anyhow::Result<i64> {
    Ok(Local::now()
        .with_time(NaiveTime::MIN)
        .earliest()
        .context("today 00:00 does not exist in the local time zone")?
        .timestamp())
}

/// Formats `seconds` as hours and minutes, e.g. `1h 5m` or `45m`.
pub(crate) fn format_duration(seconds: i64) -> String {
    match seconds / 60 {
        0 => format!("{seconds}s"),
        minutes => match minutes / 60 {
            0 => format!("{minutes}m"),
            hours => format!("{hours}h {}m", minutes % 60),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_duration_shows_hours_and_minutes() {
        assert_eq!(format_duration(32), "32s");
        assert_eq!(format_duration(45 * 60), "45m");
        assert_eq!(format_duration(3600), "1h 0m");
        assert_eq!(format_duration(3600 + 5 * 60 + 30), "1h 5m");
    }
}
