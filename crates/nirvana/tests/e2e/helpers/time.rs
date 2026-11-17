use chrono::{Days, Local, NaiveDate, NaiveDateTime};

/// Returns the local date `offset` days before today.
pub fn day(offset: u64) -> NaiveDate {
    Local::now().date_naive() - Days::new(offset)
}

/// Returns `time` on `day` as `YYYY-MM-DD HH:MM`.
pub fn at(day: NaiveDate, time: &str) -> String {
    format!("{day} {time}")
}

/// Returns the Unix timestamp of `time` on `day` in local time.
pub fn timestamp(day: NaiveDate, time: &str) -> i64 {
    NaiveDateTime::parse_from_str(&at(day, time), "%Y-%m-%d %H:%M")
        .unwrap()
        .and_local_timezone(Local)
        .single()
        .unwrap()
        .timestamp()
}
