use std::env;

use nirvana_core::AppInfo;

const SHORT_HASH: Option<&str> = option_env!("SHORT_HASH");
const COMMIT_HASH: Option<&str> = option_env!("COMMIT_HASH");
const COMMIT_DATE: Option<&str> = option_env!("COMMIT_DATE");

pub(crate) fn version(info: &AppInfo) -> String {
    format_version(info, SHORT_HASH.zip(COMMIT_DATE))
}

pub(crate) fn rows(info: &AppInfo) -> Vec<(&'static str, String)> {
    build_rows(info, COMMIT_HASH.zip(COMMIT_DATE))
}

fn format_version(info: &AppInfo, commit: Option<(&str, &str)>) -> String {
    let base = &info.version;
    let dev = if info.is_dev { "-dev" } else { "" };
    match commit {
        Some((hash, date)) => format!("{base}{dev} ({hash} {date})"),
        None => format!("{base}{dev}"),
    }
}

fn build_rows(info: &AppInfo, commit: Option<(&str, &str)>) -> Vec<(&'static str, String)> {
    let binary = env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "<unknown>".into());
    let mut rows = vec![
        ("Version", version(info)),
        ("OS", format!("{} ({})", env::consts::OS, env::consts::ARCH)),
        ("Binary", binary),
    ];
    if let Some((hash, date)) = commit {
        rows.push(("Commit hash", hash.to_string()));
        rows.push(("Commit date", date.to_string()));
    }
    let keyring = match &info.keyring {
        Ok(vendor) => vendor.clone(),
        Err(reason) => format!("unavailable ({reason})"),
    };
    rows.extend([
        ("SQLite", info.sqlite_version.clone()),
        ("Keyring", keyring),
        ("Config", info.config_file.display().to_string()),
        ("Database", info.db_file.display().to_string()),
        ("Log file", info.log_file.display().to_string()),
    ]);
    rows
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn app_info(is_dev: bool, keyring: Result<String, String>) -> AppInfo {
        AppInfo {
            version: "1.0.0".into(),
            config_file: PathBuf::from("/config/config.toml"),
            db_file: PathBuf::from("/data/nirvana.db"),
            log_file: PathBuf::from("/logs/nirvana.log"),
            sqlite_version: "3.50.0".into(),
            keyring,
            is_dev,
        }
    }

    fn labels(rows: &[(&'static str, String)]) -> Vec<&'static str> {
        rows.iter().map(|(label, _)| *label).collect()
    }

    fn value<'a>(rows: &'a [(&'static str, String)], label: &str) -> &'a str {
        rows.iter()
            .find(|(l, _)| *l == label)
            .map(|(_, v)| v.as_str())
            .unwrap()
    }

    #[test]
    fn format_version_includes_dev_suffix_and_commit() {
        let commit = Some(("abc1234", "2026-11-21"));

        assert_eq!(
            format_version(&app_info(false, Ok(String::new())), commit),
            "1.0.0 (abc1234 2026-11-21)"
        );
        assert_eq!(
            format_version(&app_info(true, Ok(String::new())), commit),
            "1.0.0-dev (abc1234 2026-11-21)"
        );
        assert_eq!(
            format_version(&app_info(false, Ok(String::new())), None),
            "1.0.0"
        );
        assert_eq!(
            format_version(&app_info(true, Ok(String::new())), None),
            "1.0.0-dev"
        );
    }

    #[test]
    fn build_rows_lists_commit_rows_only_with_commit() {
        let info = app_info(false, Ok("macOS Keychain".into()));

        assert_eq!(
            labels(&build_rows(&info, Some(("abc1234def", "2026-11-21")))),
            [
                "Version",
                "OS",
                "Binary",
                "Commit hash",
                "Commit date",
                "SQLite",
                "Keyring",
                "Config",
                "Database",
                "Log file",
            ]
        );
        assert_eq!(
            labels(&build_rows(&info, None)),
            [
                "Version", "OS", "Binary", "SQLite", "Keyring", "Config", "Database", "Log file",
            ]
        );
    }

    #[test]
    fn build_rows_formats_values() {
        let info = app_info(false, Ok("macOS Keychain".into()));
        let rows = build_rows(&info, Some(("abc1234def", "2026-11-21")));

        assert_eq!(value(&rows, "Version"), version(&info));
        assert_eq!(value(&rows, "Commit hash"), "abc1234def");
        assert_eq!(value(&rows, "Commit date"), "2026-11-21");
        assert_eq!(value(&rows, "SQLite"), "3.50.0");
        assert_eq!(value(&rows, "Keyring"), "macOS Keychain");
        assert_eq!(value(&rows, "Config"), "/config/config.toml");
        assert_eq!(value(&rows, "Database"), "/data/nirvana.db");
        assert_eq!(value(&rows, "Log file"), "/logs/nirvana.log");
    }

    #[test]
    fn build_rows_reports_unavailable_keyring() {
        let info = app_info(false, Err("no secret service".into()));

        assert_eq!(
            value(&build_rows(&info, None), "Keyring"),
            "unavailable (no secret service)"
        );
    }
}
