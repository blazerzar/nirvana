use anyhow::Context;
use chrono::{DateTime, Local, NaiveDate, NaiveTime};

/// Returns the Unix timestamp of today 00:00 in local time.
pub(crate) fn today_start() -> anyhow::Result<i64> {
    day_start(Local::now().date_naive())
}

/// Returns the Unix timestamp of `date` 00:00 in local time.
pub(crate) fn day_start(date: NaiveDate) -> anyhow::Result<i64> {
    Ok(date
        .and_time(NaiveTime::MIN)
        .and_local_timezone(Local)
        .earliest()
        .with_context(|| format!("{date} 00:00 does not exist in the local time zone"))?
        .timestamp())
}

/// Returns the local date of `timestamp`.
pub(crate) fn local_date(timestamp: i64) -> anyhow::Result<NaiveDate> {
    Ok(DateTime::from_timestamp(timestamp, 0)
        .context("time is out of range")?
        .with_timezone(&Local)
        .date_naive())
}

/// Formats `timestamp` as local `HH:MM`.
pub(crate) fn time_of_day(timestamp: i64) -> String {
    DateTime::from_timestamp(timestamp, 0)
        .map(|utc| utc.with_timezone(&Local).format("%H:%M").to_string())
        .unwrap_or_default()
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

    fn local_timestamp(hour: u32, minute: u32, second: u32) -> i64 {
        NaiveDate::from_ymd_opt(2026, 5, 22)
            .and_then(|date| date.and_hms_opt(hour, minute, second))
            .and_then(|time| time.and_local_timezone(Local).earliest())
            .unwrap()
            .timestamp()
    }

    #[test]
    fn time_of_day_shows_local_hours_and_minutes() {
        assert_eq!(time_of_day(local_timestamp(14, 30, 0)), "14:30");
    }

    #[test]
    fn time_of_day_pads_hours_and_minutes() {
        assert_eq!(time_of_day(local_timestamp(9, 5, 0)), "09:05");
    }

    #[test]
    fn time_of_day_drops_seconds() {
        assert_eq!(time_of_day(local_timestamp(23, 59, 59)), "23:59");
    }

    #[test]
    fn time_of_day_is_empty_out_of_range() {
        assert_eq!(time_of_day(i64::MAX), "");
    }
}
