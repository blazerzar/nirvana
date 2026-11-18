mod config;
mod e2e;
mod jira;
mod linear;
mod nirvana;
mod time;

use std::fmt::Debug;
use std::time::Duration;
use std::{panic, thread};

pub use config::Config;
pub use e2e::E2e;
pub use jira::Jira;
pub use linear::{Linear, comment};
pub use nirvana::{Nirvana, Row};
pub use time::{at, day, timestamp};
use tokio::runtime;
use tokio::time::sleep;

/// Runs `cleanup` on a new thread and runtime, so `Drop` can wait for async
/// work inside a test runtime. A failed cleanup panics, unless the test
/// already panicked.
fn run_cleanup(cleanup: impl Future<Output = ()> + Send + 'static) {
    let result = thread::spawn(|| {
        runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(cleanup)
    })
    .join();
    if let Err(error) = result
        && !thread::panicking()
    {
        panic::resume_unwind(error);
    }
}

/// Asserts that `fetch` returns `expected` for `key`, fetching up to 3 times
/// 1 second apart.
async fn assert_fetched<T: PartialEq + Debug>(
    key: &str,
    expected: &[T],
    fetch: impl AsyncFn() -> Vec<T>,
) {
    let mut actual = fetch().await;
    for _ in 1..3 {
        if actual == expected {
            return;
        }
        sleep(Duration::from_secs(1)).await;
        actual = fetch().await;
    }
    assert_eq!(actual, expected, "{key}");
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
