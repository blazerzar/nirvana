use anyhow::Context;
use chrono::{Local, NaiveTime};
use console::{Alignment, Term, style};
use nirvana_core::Nirvana;

use crate::cli::ListArgs;
use crate::cli::table::{self, Column};
use crate::cli::time::{format_duration, format_timestamp, parse_time};

const COLUMNS: [Column; 5] = [
    Column::new("KEY"),
    Column::new("TITLE").max_width(50),
    Column::new("START"),
    Column::new("STOP"),
    Column::new("DURATION").align(Alignment::Right),
];

const COLUMNS_WITH_ID: [Column; 6] = [
    Column::new("ID").align(Alignment::Right),
    Column::new("KEY"),
    Column::new("TITLE").max_width(50),
    Column::new("START"),
    Column::new("STOP"),
    Column::new("DURATION").align(Alignment::Right),
];

pub(crate) async fn run(args: ListArgs) -> anyhow::Result<()> {
    let now = Local::now();
    let today = now.date_naive();
    let from = match args.from {
        Some(from) => parse_time(&from, today)?,
        None => now
            .with_time(NaiveTime::MIN)
            .earliest()
            .context("today 00:00 does not exist in the local time zone")?
            .timestamp(),
    };
    let to = args.to.map(|to| parse_time(&to, today)).transpose()?;

    let service = Nirvana::new().await?;
    let slots = service.list_slots(from, to, args.by_ticket).await?;
    let term = Term::stdout();
    if slots.is_empty() {
        term.write_line(&format!("{}", style("No slots in this range").yellow()))?;
        return Ok(());
    }

    let mut previous_ticket_id = None;
    let rows: Vec<(String, [String; 5])> = slots
        .into_iter()
        .map(|slot| {
            let same_ticket = args.by_ticket
                && previous_ticket_id.replace(slot.ticket.id) == Some(slot.ticket.id);
            let (key, title) = match same_ticket {
                true => Default::default(),
                false => (
                    slot.ticket.ticket_key,
                    slot.ticket.title.unwrap_or_default(),
                ),
            };
            let row = [
                key,
                title,
                format_timestamp(slot.model.started_at),
                slot.model
                    .stopped_at
                    .map(format_timestamp)
                    .unwrap_or_default(),
                format_duration(
                    slot.model.stopped_at.unwrap_or(now.timestamp()) - slot.model.started_at,
                ),
            ];
            (slot.model.id.to_string(), row)
        })
        .collect();

    if args.show_ids {
        let rows: Vec<[String; 6]> = rows
            .into_iter()
            .map(|(id, [key, title, start, stop, duration])| {
                [id, key, title, start, stop, duration]
            })
            .collect();
        table::print(&term, &COLUMNS_WITH_ID, &rows)
    } else {
        let rows: Vec<[String; 5]> = rows.into_iter().map(|(_, row)| row).collect();
        table::print(&term, &COLUMNS, &rows)
    }
}
