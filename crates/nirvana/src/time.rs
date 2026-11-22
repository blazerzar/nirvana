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
