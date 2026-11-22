use console::{Term, style};
use nirvana_core::{Nirvana, PublishTarget, TargetState};

use crate::cli::table::{self, Column};
use crate::cli::time::format_timestamp;
use crate::cli::{ShowArgs, slot_target};
use crate::time::format_duration;

const COLUMNS: [Column; 4] = [
    Column::new("CONNECTION"),
    Column::new("STATE"),
    Column::new("PUBLISHED"),
    Column::new("ERROR").max_width(50),
];

pub(crate) async fn run(args: ShowArgs) -> anyhow::Result<()> {
    let target = slot_target(args.id, args.at.as_deref())?;

    let service = Nirvana::new().await?;
    let term = Term::stdout();
    let (slot, targets) = service.get_slot_targets(target).await?;

    term.write_line(&format!(
        "{} {}",
        style(&slot.ticket.ticket_key).bold(),
        style(&slot.ticket.title).dim(),
    ))?;
    term.write_line(&format!(
        "{} - {} ({})",
        format_timestamp(slot.model.started_at),
        slot.model
            .stopped_at
            .map_or_else(|| "now".to_string(), format_timestamp),
        format_duration(slot.elapsed()),
    ))?;
    term.write_line("")?;

    if targets.is_empty() {
        term.write_line(&format!("{}", style("Not published").yellow()))?;
        return Ok(());
    }
    let rows: Vec<[String; 4]> = targets.iter().map(row).collect();
    table::print(&term, &COLUMNS, &rows)?;

    if args.show_errors {
        term.write_line("")?;

        for target in &targets {
            if let Some(error) = &target.entry.error {
                term.write_line(&format!(
                    "{}: {}",
                    style(&target.connection.name),
                    style(error).dim()
                ))?;
            }
        }
    }

    Ok(())
}

fn row(target: &PublishTarget) -> [String; 4] {
    let state = match target.state {
        TargetState::Pending => style("pending").yellow(),
        TargetState::Stuck => style("stuck").red(),
        TargetState::Failed => style("failed").red(),
        TargetState::Published => style("published").green(),
    };
    [
        target.connection.name.clone(),
        state.to_string(),
        target
            .entry
            .published_at
            .map(format_timestamp)
            .unwrap_or_default(),
        target
            .entry
            .error
            .as_deref()
            .unwrap_or_default()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    ]
}
