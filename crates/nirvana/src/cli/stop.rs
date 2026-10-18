use chrono::TimeZone;
use console::style;
use nirvana_core::service::Nirvana;

use crate::cli::StopArgs;

pub(crate) fn run(args: StopArgs) -> anyhow::Result<()> {
    let api = Nirvana::new()?;
    let at_ts = args.at.as_deref().map(parse_time).transpose()?;

    match api.stop_slot(at_ts)? {
        Some(slot) => {
            let duration = slot.stopped_at.unwrap() - slot.started_at;

            println!(
                "{} {} {} {}",
                style("Stopped").green().bold(),
                style(&slot.ticket_key).bold(),
                style(slot.summary.as_deref().unwrap_or("")).dim(),
                style(format!("({})", style(format_duration(duration)))).white(),
            );
        }
        None => println!("{}", style("No active slot").yellow()),
    }

    Ok(())
}

#[allow(unused)]
fn format_timestamp(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_else(|| ts.to_string())
}

fn format_duration(seconds: i64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
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
        anyhow::bail!("cannot stop in the future");
    }

    Ok(timestamp)
}
