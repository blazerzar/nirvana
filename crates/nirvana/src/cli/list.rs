use anyhow::Context;
use chrono::{Local, NaiveTime, TimeZone};
use console::{Alignment, Term};
use nirvana_core::service::{Nirvana, SlotSort};

use crate::cli::table::{self, Column};
use crate::cli::{ListArgs, parse_time};

const COLUMNS: [Column; 6] = [
    Column::new("TICKET").align(Alignment::Right),
    Column::new("SUMMARY").max_width(30),
    Column::new("STARTED"),
    Column::new("STOPPED"),
    Column::new("DURATION"),
    Column::new("URL").max_width(50),
];

pub(crate) async fn run(args: ListArgs) -> anyhow::Result<()> {
    let api = Nirvana::new().await?;

    let today = Local::now().date_naive();
    let from = args
        .start
        .as_deref()
        .map(parse_time)
        .transpose()?
        .unwrap_or_else(|| {
            let dt = today.and_time(NaiveTime::MIN);
            Local.from_local_datetime(&dt).single().unwrap().timestamp()
        });

    let to = args.stop.as_deref().map(parse_time).transpose()?;

    let sort = if args.by_ticket {
        SlotSort::TicketId
    } else {
        SlotSort::StartedAt
    };

    let slots = api.get_slots(from, to, sort).await?;

    let term = Term::stdout();
    if slots.is_empty() {
        term.write_line("No slots found.")?;
        return Ok(());
    }

    let connection = api
        .get_active_connection()
        .await?
        .context("No active connection")?;
    let urls = api.issue_urls(&connection, slots.iter().map(|(_, ticket)| ticket))?;
    let now = chrono::Local::now().timestamp();
    let rows = slots
        .iter()
        .zip(urls)
        .map(|((s, ticket), url)| {
            let started = format_time(s.started_at);
            let stopped = match s.stopped_at {
                Some(t) => format_time(t),
                None => "".to_string(),
            };
            let ended = s.stopped_at.unwrap_or(now);
            let duration = format_duration(ended - s.started_at);
            [
                ticket.ticket_key.clone(),
                ticket.title.clone().unwrap_or_default(),
                started,
                stopped,
                duration,
                url,
            ]
        })
        .collect::<Vec<_>>();

    table::print(&term, &COLUMNS, &rows)?;

    Ok(())
}

fn format_time(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.with_timezone(&Local).format("%H:%M").to_string())
        .unwrap_or_else(|| ts.to_string())
}

fn format_duration(secs: i64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}
