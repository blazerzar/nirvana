use console::{Style, Term, style};
use dialoguer::Confirm;
use nirvana_core::Nirvana;
use tracing::info;

use crate::cli::theme;

pub(crate) async fn view(lines: Option<usize>, follow: bool) -> anyhow::Result<()> {
    let mut reader = Nirvana::open_logs().await?;
    let term = Term::stdout();

    for line in reader.tail(lines).await? {
        term.write_line(&colorize(&line))?;
    }
    if !follow {
        return Ok(());
    }
    loop {
        for line in reader.follow().await? {
            term.write_line(&colorize(&line))?;
        }
    }
}

struct LogLine<'a> {
    timestamp: &'a str,
    level: &'a str,
    target: &'a str,
    message: &'a str,
}

/// Splits a line written by `nirvana_core::file_layer`, e.g.
/// `2026-05-19 14:30:00  INFO nirvana::cli: slot started slot_id=1`.
fn parse(line: &str) -> Option<LogLine<'_>> {
    let (date, rest) = line.split_once(' ')?;
    let (time, rest) = rest.split_once(' ')?;
    let (level, rest) = rest.trim_start().split_once(' ')?;
    let (target, message) = rest.split_once(": ")?;
    Some(LogLine {
        timestamp: &line[..date.len() + 1 + time.len()],
        level,
        target,
        message,
    })
}

fn level_style(level: &str) -> Option<Style> {
    let style = Style::new();
    match level {
        "ERROR" => Some(style.red()),
        "WARN" => Some(style.yellow()),
        "INFO" => Some(style.green()),
        "DEBUG" => Some(style.blue()),
        "TRACE" => Some(style.magenta()),
        _ => None,
    }
}

/// Colors the level and dims the timestamp and target. Lines in another
/// format are returned unchanged.
fn colorize(line: &str) -> String {
    let Some(parsed) = parse(line) else {
        return line.to_string();
    };
    let Some(level_style) = level_style(parsed.level) else {
        return line.to_string();
    };
    format!(
        "{} {} {}: {}",
        style(parsed.timestamp).dim(),
        level_style.apply_to(format!("{:>5}", parsed.level)),
        style(parsed.target).dim(),
        parsed.message,
    )
}

pub(crate) async fn clear(yes: bool) -> anyhow::Result<()> {
    let term = Term::stdout();
    if yes
        || Confirm::with_theme(&theme())
            .with_prompt("Are you sure you want to clear the log file?")
            .default(false)
            .interact()?
    {
        Nirvana::clear_logs().await?;
        info!("logs cleared");
        term.write_line(&format!("{} the log file", style("Cleared").green()))?;
    } else {
        term.write_line(&format!(
            "{}",
            style("The log file was not cleared").yellow()
        ))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_splits_log_line() {
        let line = parse("2026-05-19 14:30:00  INFO nirvana::cli: slot started slot_id=1").unwrap();

        assert_eq!(line.timestamp, "2026-05-19 14:30:00");
        assert_eq!(line.level, "INFO");
        assert_eq!(line.target, "nirvana::cli");
        assert_eq!(line.message, "slot started slot_id=1");
    }

    #[test]
    fn colorize_keeps_line_without_colors() {
        console::set_colors_enabled(false);
        let line = "2026-05-19 14:30:00  WARN nirvana_core::secrets: keyring unavailable";

        assert_eq!(colorize(line), line);
    }

    #[test]
    fn colorize_keeps_other_lines() {
        let line = "   0: std::backtrace::Backtrace::create";

        assert_eq!(colorize(line), line);
    }
}
