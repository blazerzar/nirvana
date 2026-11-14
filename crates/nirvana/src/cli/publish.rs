use anyhow::bail;
use chrono::Local;
use console::{Term, style};
use nirvana_core::Nirvana;
use tracing::{info, warn};

use crate::cli::PublishArgs;
use crate::cli::time::{format_duration, format_timestamp, parse_time, today_start};

pub(crate) async fn run(args: PublishArgs) -> anyhow::Result<()> {
    let today = Local::now().date_naive();
    let (from, to) = match args.all {
        true => (None, None),
        false => (
            Some(match args.start {
                Some(start) => parse_time(&start, today)?,
                None => today_start()?,
            }),
            args.stop.map(|stop| parse_time(&stop, today)).transpose()?,
        ),
    };

    let service = Nirvana::new().await?;
    let result = service
        .publish_slots(from, to, args.target.as_deref(), args.retry)
        .await?;
    info!(
        published = result.published.len(),
        failed = result.failed.len(),
        target = ?args.target,
        retry = args.retry,
        "slots published"
    );

    let term = Term::stdout();
    if result.published.is_empty() && result.failed.is_empty() {
        term.write_line(&format!("{}", style("Nothing to publish").yellow()))?;
        return Ok(());
    }
    if !result.published.is_empty() {
        let seconds: i64 = result.published.iter().map(|slot| slot.elapsed()).sum();
        term.write_line(&format!(
            "{} {} ({})",
            style("Published").green(),
            slots(result.published.len()),
            format_duration(seconds),
        ))?;
    }
    for failure in &result.failed {
        let key = &failure.slot.ticket.ticket_key;
        warn!(%key, slot_id = failure.slot.model.id, error = %failure.error, "slot publish failed");
        term.write_line(&format!(
            "{} {} {} {}",
            style("✗").red(),
            style(key).bold(),
            format_timestamp(failure.slot.model.started_at),
            failure.error,
        ))?;
    }
    if !result.failed.is_empty() {
        let total = result.published.len() + result.failed.len();
        bail!(
            "{} of {} failed to publish",
            result.failed.len(),
            slots(total)
        );
    }
    Ok(())
}

fn slots(count: usize) -> String {
    match count {
        1 => "1 slot".to_string(),
        count => format!("{count} slots"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_uses_singular_for_one() {
        assert_eq!(slots(0), "0 slots");
        assert_eq!(slots(1), "1 slot");
        assert_eq!(slots(2), "2 slots");
    }
}
