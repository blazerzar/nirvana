use chrono::{Local, TimeZone};
use console::{Style, Term};
use nirvana_core::db::connection_repo;
use nirvana_core::domain::Connection;
use nirvana_core::{ActiveConnection, AppConfig, AppPaths, Database};
use std::cmp::max;

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
            Column::Unlimited(h) => h.len(),
            Column::Limited(h, _) => h.len(),
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

    /// Truncate cell contents with ellipsis if needed.
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
    Column::Unlimited("ID"),
    Column::Static("ACTIVE", "ACTIVE".len()),
    Column::Limited("NAME", 12),
    Column::Unlimited("KIND"),
    Column::Limited("HOST", 28),
    Column::Limited("IDENTITY", 28),
    Column::Static("UPDATED", "2026-05-17".len()),
];

pub fn list() -> anyhow::Result<()> {
    let paths = AppPaths::resolve();
    let db = Database::initialize(&paths.db_file)?;
    let config = AppConfig::load(&paths.config_file)?;
    let connections = connection_repo::list(&db)?;

    let term = Term::stdout();
    if connections.is_empty() {
        term.write_line("No connections found.")?;
        term.write_line("Add one with: nirvana connection add")?;
        return Ok(());
    }

    let rows: Vec<[String; COLUMNS.len()]> = connections
        .iter()
        .map(|c| {
            let updated = Local
                .timestamp_opt(c.updated_at, 0)
                .single()
                .unwrap()
                .date_naive()
                .to_string();
            let active = if is_active(c, &config) { "*" } else { "" };
            [
                c.id.to_string(),
                active.to_string(),
                c.name.clone(),
                c.kind.clone(),
                c.base_url.clone(),
                c.identity.clone(),
                updated,
            ]
        })
        .collect();

    let mut widths: Vec<usize> = COLUMNS.iter().map(|c| c.initial_width()).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = COLUMNS[i].fit(widths[i], cell);
        }
    }

    // Header
    let bold = Style::new().bold();
    let header: Vec<&str> = COLUMNS.iter().map(|c| c.header()).collect();
    print_row(&term, &header, &widths, Some(&bold))?;

    // Data rows
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

fn is_active(conn: &Connection, config: &AppConfig) -> bool {
    match &config.active_connection {
        Some(ActiveConnection::Id(id)) => conn.id == *id,
        Some(ActiveConnection::Name(name)) => conn.name == *name,
        None => false,
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
                1 => format!("{:^w$}", cell, w = *w),
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
