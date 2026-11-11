use anyhow::Context;
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

/// Parses `HH:MM` (on `today`) or `YYYY-MM-DD HH:MM` in local time into a
/// Unix timestamp.
#[cfg_attr(not(test), expect(dead_code))]
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
