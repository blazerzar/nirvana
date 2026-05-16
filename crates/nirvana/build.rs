use std::process::Command;

fn main() {
    let git_log = Command::new("git")
        .args(["log", "-1", "--format=%H%n%h%n%cs"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok());

    if let Some(log) = &git_log {
        let lines: Vec<&str> = log.lines().collect();
        if let [full_hash, short_hash, commit_date] = lines.as_slice() {
            let dirty = Command::new("git")
                .args(["status", "--porcelain"])
                .output()
                .ok()
                .map(|o| o.status.success() && !o.stdout.is_empty())
                .unwrap_or(false);

            let suffix = if dirty { "-dirty" } else { "" };
            println!("cargo:rustc-env=COMMIT_HASH={full_hash}{suffix}");
            println!("cargo:rustc-env=SHORT_HASH={short_hash}{suffix}");
            println!("cargo:rustc-env=COMMIT_DATE={commit_date}");
        }
    }
}
