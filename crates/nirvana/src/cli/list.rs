use anyhow::Context;
use chrono::{Local, NaiveTime, TimeZone};
use console::{Style, Term};
use nirvana_core::service::{Nirvana, SlotSort};
use std::cmp::max;

use crate::cli::{ListArgs, parse_time};

enum Column {
    Unlimited(&'static str),
    Limited(&'static str, usize),
    Static(&'static str, usize),
}

impl Column {
    fn header(&self) -> &'static str {
        match self {
            Column::Unlimited(h) | Column::Limited(h, _) | Column::Static(h, _) => h,
        }
    }

    fn initial_width(&self) -> usize {
        match self {
            Column::Unlimited(h) | Column::Limited(h, _) => h.len(),
            Column::Static(h, n) => max(h.len(), *n),
        }
    }

    fn fit(&self, current: usize, cell: &str) -> usize {
        match self {
            Column::Unlimited(_) => max(current, cell.len()),
            Column::Limited(_, cap) => max(current, cell.len().min(*cap)),
            Column::Static(_, _) => current,
        }
    }

    fn render(&self, cell: &str) -> String {
        match self {
            Column::Limited(_, cap) if cell.len() > *cap => {
                let mut s = cell[..cap.saturating_sub(1)].to_string();
                s.push('…');
                s
            }
            Column::Static(_, n) if cell.len() > *n => cell[..*n].to_string(),
            _ => cell.to_string(),
        }
    }
}

const COLUMNS: &[Column] = &[
    Column::Unlimited("TICKET"),
    Column::Limited("SUMMARY", 30),
    Column::Limited("NOTE", 20),
    Column::Static("STARTED", "HH:MM".len()),
    Column::Static("STOPPED", "running…".len()),
    Column::Static("DURATION", "1h 23m".len()),
    Column::Limited("URL", 50),
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
                ticket.summary.clone().unwrap_or_default(),
                s.note.clone().unwrap_or_default(),
                started,
                stopped,
                duration,
                url,
            ]
        })
        .collect::<Vec<_>>();

    let mut widths: Vec<usize> = COLUMNS.iter().map(|c| c.initial_width()).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = COLUMNS[i].fit(widths[i], cell);
        }
    }

    let bold = Style::new().bold();
    let header: Vec<&str> = COLUMNS.iter().map(|c| c.header()).collect();
    print_row(&term, &header, &widths, Some(&bold))?;

    for row in &rows {
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| COLUMNS[i].render(cell))
            .collect();
        let cell_refs: Vec<&str> = cells.iter().map(|s| s.as_str()).collect();
        print_row(&term, &cell_refs, &widths, None)?;
    }

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

fn print_row(
    term: &Term,
    cells: &[&str],
    widths: &[usize],
    style: Option<&Style>,
) -> anyhow::Result<()> {
    debug_assert_eq!(cells.len(), widths.len());
    debug_assert_eq!(cells.len(), COLUMNS.len());

    let parts: Vec<String> = cells
        .iter()
        .zip(widths)
        .enumerate()
        .map(|(i, (cell, w))| {
            let padded = match i {
                0 => format!("{:>w$}", cell, w = *w),
                _ => format!("{:<w$}", cell, w = *w),
            };
            match style {
                Some(s) => s.apply_to(padded).to_string(),
                None => padded,
            }
        })
        .collect();

    term.write_line(&parts.join("  "))?;
    Ok(())
}
