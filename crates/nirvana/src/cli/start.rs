use chrono::{Local, TimeZone};
use console::style;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{FuzzySelect, Input};
use nirvana_core::service::Nirvana;

use crate::cli::StartArgs;

pub(crate) async fn run(args: StartArgs) -> anyhow::Result<()> {
    let api = Nirvana::new().await?;

    let (ticket, at, note) = match args.ticket {
        Some(ticket) => (ticket, args.at, args.note),
        None => interactive_prompt(&api).await?,
    };

    let at_ts = at.as_deref().map(parse_time).transpose()?;

    let (slot, ticket) = api.start_slot(&ticket, at_ts, note.as_deref()).await?;
    let time = format_timestamp(slot.started_at);

    println!(
        "{} {} {} {} {}",
        style("Started").green().bold(),
        style(&ticket.ticket_key).bold(),
        style(ticket.summary.as_deref().unwrap_or("")).dim(),
        style("at").white(),
        time,
    );

    Ok(())
}

async fn interactive_prompt(
    api: &Nirvana,
) -> anyhow::Result<(String, Option<String>, Option<String>)> {
    let theme = ColorfulTheme {
        active_item_prefix: console::Style::new().green().apply_to(String::from("❯")),
        ..ColorfulTheme::default()
    };

    let tickets = api.list_recent_tickets().await?;

    let ticket = if tickets.is_empty() {
        Input::with_theme(&theme)
            .with_prompt("Ticket key")
            .interact_text()?
    } else {
        let items: Vec<String> = tickets
            .iter()
            .map(|t| match &t.summary {
                Some(s) => format!("{}  {}", t.ticket_key, s),
                None => t.ticket_key.clone(),
            })
            .collect();

        let idx = FuzzySelect::with_theme(&theme)
            .with_prompt("Ticket")
            .items(&items)
            .default(0)
            .max_length(5)
            .interact_opt()?;

        match idx {
            None => std::process::exit(0),
            Some(i) => tickets[i].ticket_key.clone(),
        }
    };

    let now_str = Local::now().format("%H:%M").to_string();
    let at: String = Input::with_theme(&theme)
        .with_prompt("Start time")
        .default(now_str)
        .allow_empty(true)
        .interact_text()?;
    let at = if at.is_empty() { None } else { Some(at) };

    let note: String = Input::with_theme(&theme)
        .with_prompt("Note")
        .default(String::new())
        .allow_empty(true)
        .interact_text()?;
    let note = if note.is_empty() { None } else { Some(note) };

    Ok((ticket, at, note))
}

fn format_timestamp(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_else(|| ts.to_string())
}

fn parse_time(input: &str) -> anyhow::Result<i64> {
    let now = chrono::Local::now();
    let today = now.date_naive();

    let parsed = if input.contains('-') {
        chrono::NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M").ok()
    } else {
        chrono::NaiveTime::parse_from_str(input, "%H:%M")
            .ok()
            .map(|t| today.and_time(t))
    };

    let dt = parsed.ok_or_else(|| {
        anyhow::anyhow!(
            "invalid time format: '{}'. Use HH:MM or YYYY-MM-DD HH:MM",
            input
        )
    })?;
    let local = chrono::Local
        .from_local_datetime(&dt)
        .single()
        .ok_or_else(|| anyhow::anyhow!("ambiguous or invalid local time"))?;
    let timestamp = local.timestamp();

    if timestamp > now.timestamp() {
        anyhow::bail!("cannot start in the future");
    }

    Ok(timestamp)
}
