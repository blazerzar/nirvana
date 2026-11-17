mod config;
mod jira;
mod linear;
mod nirvana;
mod time;

use std::env;
use std::sync::LazyLock;

use chrono::Local;
pub use config::Config;
pub use jira::Jira;
pub use linear::{Linear, comment};
pub use nirvana::{Nirvana, Row};
pub use time::{at, day, timestamp};

/// Returns a unique issue title for issue `name` of `test` in this run.
pub fn title(test: &str, name: &str) -> String {
    static RUN: LazyLock<String> = LazyLock::new(|| {
        env::var("GITHUB_RUN_ID")
            .unwrap_or_else(|_| Local::now().format("%Y%m%d%H%M%S").to_string())
    });
    let run = &*RUN;
    format!(
        "[e2e] {name} {}-{} {run} {test}",
        env::consts::OS,
        env::consts::ARCH
    )
}

/// Returns the part of `title` that tables show before truncating it.
pub fn shown(title: &str) -> String {
    title.chars().take(49).collect()
}

pub fn row(key: &str, start: String, marker: &str) -> Row {
    (key.to_string(), start, marker.to_string())
}

pub fn all_published(rows: Vec<Row>) -> bool {
    !rows.is_empty() && rows.iter().all(|(_, _, marker)| marker == "✓")
}

pub fn sorted(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut items: Vec<String> = items.into_iter().collect();
    items.sort();
    items
}

/// Returns the first column of each row of the table in `output`.
pub fn first_column(output: &str) -> Vec<String> {
    output
        .lines()
        .skip(1)
        .map(|line| line.split_whitespace().next().unwrap().to_string())
        .collect()
}

/// Returns the first line of `output` that has `text` as a whole word.
pub fn line_with<'a>(output: &'a str, text: &str) -> &'a str {
    output
        .lines()
        .find(|line| line.split_whitespace().any(|word| word == text))
        .unwrap_or_else(|| panic!("no line with {text} in:\n{output}"))
}
