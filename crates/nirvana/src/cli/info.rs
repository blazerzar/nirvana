use console::{Style, Term};
use std::env;
use std::path::PathBuf;

use nirvana_core::AppPaths;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const SHORT_HASH: Option<&str> = option_env!("SHORT_HASH");
const COMMIT_HASH: Option<&str> = option_env!("COMMIT_HASH");
const COMMIT_DATE: Option<&str> = option_env!("COMMIT_DATE");

pub fn run() -> anyhow::Result<()> {
    let paths = AppPaths::resolve();
    let term = Term::stdout();

    let bold = Style::new().bold();

    let version_str = match (paths.is_dev, SHORT_HASH, COMMIT_DATE) {
        (true, Some(hash), Some(date)) => format!("{VERSION}-dev ({hash} {date})"),
        (false, Some(hash), Some(date)) => format!("{VERSION} ({hash} {date})"),
        (true, _, _) => format!("{VERSION}-dev"),
        (false, _, _) => VERSION.to_string(),
    };

    let os_str = format!("{} ({})", env::consts::OS, env::consts::ARCH);

    term.write_line(&format!("Nirvana {}", bold.apply_to(&version_str)))?;
    term.write_line("")?;
    print_row(&term, "Version", &version_str)?;
    print_row(&term, "OS", &os_str)?;
    print_row(&term, "Binary", &exe_display())?;
    if let (Some(hash), Some(date)) = (COMMIT_HASH, COMMIT_DATE) {
        print_row(&term, "Commit hash", hash)?;
        print_row(&term, "Commit date", date)?;
    }
    print_row(&term, "Config", &paths.config_file.display().to_string())?;
    print_row(&term, "Database", &paths.db_file.display().to_string())?;
    print_row(&term, "Log file", &paths.log_file.display().to_string())?;

    Ok(())
}

fn print_row(term: &Term, label: &str, value: &str) -> std::io::Result<()> {
    let bold = Style::new().bold();
    term.write_line(&format!("{:16}{}", bold.apply_to(label), value))
}

fn exe_display() -> String {
    env::current_exe()
        .map(|p: PathBuf| p.display().to_string())
        .unwrap_or_else(|_| "<unknown>".into())
}
