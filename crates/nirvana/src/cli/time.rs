use anyhow::{Context, anyhow};
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime};

pub(crate) fn format_timestamp(timestamp: i64) -> String {
    DateTime::from_timestamp(timestamp, 0)
        .map(|utc| {
            utc.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
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

/// Parses hours and minutes, e.g. `1h 30m`, `1h30m`, `1h` or `45m`, into
/// seconds. The duration must be positive.
pub(crate) fn parse_duration(input: &str) -> anyhow::Result<i64> {
    let invalid = || anyhow!("invalid duration '{input}', expected e.g. 1h 30m, 1h or 45m");
    let compact: String = input.split_whitespace().collect();
    let (hours, rest) = compact.split_once('h').unwrap_or(("0", &compact));
    let minutes = match rest {
        "" => "0",
        rest => rest.strip_suffix('m').ok_or_else(invalid)?,
    };
    let hours: u32 = hours.parse().map_err(|_| invalid())?;
    let minutes: u32 = minutes.parse().map_err(|_| invalid())?;
    match (i64::from(hours) * 60 + i64::from(minutes)) * 60 {
        0 => Err(invalid()),
        seconds => Ok(seconds),
    }
}

/// Parses `HH:MM` (on `today`) or `YYYY-MM-DD HH:MM` in local time into a
/// Unix timestamp.
pub(crate) fn parse_time(input: &str, today: NaiveDate) -> anyhow::Result<i64> {
    let input = input.trim();
    let naive = if input.contains('-') {
        NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M")
    } else {
        NaiveTime::parse_from_str(input, "%H:%M").map(|time| today.and_time(time))
    }
    .with_context(|| format!("invalid time '{input}', expected HH:MM or YYYY-MM-DD HH:MM"))?;
    let local = naive.and_local_timezone(Local).single().with_context(|| {
        format!("time '{input}' is ambiguous or does not exist in the local time zone")
    })?;
    Ok(local.timestamp())
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn local_timestamp(date: NaiveDate, hour: u32, minute: u32) -> i64 {
        Local
            .from_local_datetime(&date.and_hms_opt(hour, minute, 0).unwrap())
            .single()
            .unwrap()
            .timestamp()
    }

    #[test]
    fn format_duration_shows_hours_and_minutes() {
        assert_eq!(format_duration(32), "32s");
        assert_eq!(format_duration(45 * 60), "45m");
        assert_eq!(format_duration(3600), "1h 0m");
        assert_eq!(format_duration(3600 + 5 * 60 + 30), "1h 5m");
    }

    #[test]
    fn parse_duration_accepts_hours_and_minutes() {
        assert_eq!(parse_duration("1h 30m").unwrap(), 5400);
        assert_eq!(parse_duration("1h30m").unwrap(), 5400);
        assert_eq!(parse_duration(" 2h ").unwrap(), 7200);
        assert_eq!(parse_duration("45m").unwrap(), 2700);
        assert_eq!(parse_duration("90m").unwrap(), 5400);
        assert_eq!(parse_duration("1h 0m").unwrap(), 3600);
        assert_eq!(parse_duration("0h 1m").unwrap(), 60);
        assert_eq!(parse_duration("150h 500m").unwrap(), 570000);
    }

    #[test]
    fn parse_duration_reads_formatted_duration() {
        assert_eq!(parse_duration(&format_duration(3900)).unwrap(), 3900);
    }

    #[test]
    fn parse_duration_rejects_invalid_input() {
        for input in [
            "", "0m", "0h 0m", "h", "m", "30", "1h30", "1.5h", "-1h", "1h -5m", "1d", "1m 1h",
        ] {
            assert!(parse_duration(input).is_err(), "accepted '{input}'");
        }
    }

    #[test]
    fn parse_time_accepts_time_on_today() {
        let today = date(2026, 5, 19);
        assert_eq!(
            parse_time("14:30", today).unwrap(),
            local_timestamp(today, 14, 30)
        );
    }

    #[test]
    fn parse_time_accepts_date_and_time() {
        assert_eq!(
            parse_time("2026-05-18 14:30", date(2026, 5, 19)).unwrap(),
            local_timestamp(date(2026, 5, 18), 14, 30)
        );
    }

    #[test]
    fn parse_time_trims_whitespace() {
        let today = date(2026, 5, 19);
        assert_eq!(
            parse_time(" 09:05 ", today).unwrap(),
            local_timestamp(today, 9, 5)
        );
    }

    #[test]
    fn parse_time_rejects_invalid_input() {
        for input in [
            "",
            "14",
            "25:00",
            "14:60",
            "2026-13-01 14:30",
            "2026-05-19",
            "now",
        ] {
            assert!(
                parse_time(input, date(2026, 5, 19)).is_err(),
                "accepted '{input}'"
            );
        }
    }
}
