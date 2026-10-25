use std::env;

use console::{Style, Term};
use nirvana_core::service::Nirvana;

const SHORT_HASH: Option<&str> = option_env!("SHORT_HASH");
const COMMIT_HASH: Option<&str> = option_env!("COMMIT_HASH");
const COMMIT_DATE: Option<&str> = option_env!("COMMIT_DATE");

pub(crate) async fn run() -> anyhow::Result<()> {
    let api = Nirvana::new().await?;
    let info = api.info().await?;
    let term = Term::stdout();
    let bold = Style::new().bold();

    let version_str = format_version(info.is_dev, info.version.clone());
    let binary_str = env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "<unknown>".into());
    let os_str = format!("{} ({})", env::consts::OS, env::consts::ARCH);

    let print_row = |label: &str, value: &str| -> std::io::Result<()> {
        term.write_line(&format!("{:16}{}", bold.apply_to(label), value))
    };

    term.write_line(&format!("Nirvana {}", bold.apply_to(&version_str)))?;
    term.write_line("")?;
    print_row("Version", &version_str)?;
    print_row("OS", &os_str)?;
    print_row("Binary", &binary_str)?;
    if let (Some(hash), Some(date)) = (COMMIT_HASH, COMMIT_DATE) {
        print_row("Commit hash", hash)?;
        print_row("Commit date", date)?;
    }
    print_row("SQLite", &info.sqlite_version)?;
    match &info.keyring {
        Ok(vendor) => print_row("Keyring", vendor)?,
        Err(reason) => print_row("Keyring", &format!("unavailable ({reason})"))?,
    }
    print_row("Config", &info.config_file.display().to_string())?;
    print_row("Database", &info.db_file.display().to_string())?;
    print_row("Log file", &info.log_file.display().to_string())?;

    Ok(())
}

fn format_version(is_dev: bool, base: String) -> String {
    match (is_dev, SHORT_HASH, COMMIT_DATE) {
        (true, Some(hash), Some(date)) => format!("{base}-dev ({hash} {date})"),
        (false, Some(hash), Some(date)) => format!("{base} ({hash} {date})"),
        (true, _, _) => format!("{base}-dev"),
        (false, _, _) => base,
    }
}
