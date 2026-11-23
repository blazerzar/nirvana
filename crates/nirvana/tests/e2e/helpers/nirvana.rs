use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::process::{self, Child, Stdio};
use std::time::Duration;
use std::{env, fs, thread};

use assert_cmd::Command;
use assert_cmd::assert::Assert;
use chrono::{Days, NaiveDate};
use tempfile::TempDir;

use crate::helpers::{Config, at, first_column};

/// The key, start and publish marker of a slot in `list`.
pub type Row = (String, String, String);

/// Runs the nirvana binary at `NIRVANA_E2E_BIN` with its config, data and
/// log in a temp dir.
pub struct Nirvana {
    bin: String,
    dir: TempDir,
}

impl Nirvana {
    pub fn new() -> Self {
        Self {
            bin: env::var("NIRVANA_E2E_BIN").expect("NIRVANA_E2E_BIN is not set"),
            dir: tempfile::tempdir().unwrap(),
        }
    }

    /// Runs `args`, asserts success and returns stdout.
    pub fn run(&self, args: &[&str]) -> String {
        let assert = self.assert(args).success();
        String::from_utf8(assert.get_output().stdout.clone()).unwrap()
    }

    /// Runs `args`, asserts exit code 1 with `error` in stderr and returns
    /// stdout.
    pub fn fail(&self, args: &[&str], error: &str) -> String {
        let assert = self.assert(args).code(1);
        let output = assert.get_output();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(error), "{stderr}");
        String::from_utf8(output.stdout.clone()).unwrap()
    }

    fn assert(&self, args: &[&str]) -> Assert {
        let assert = Command::from_std(self.command(args)).assert();
        let log = fs::read_to_string(self.dir.path().join("nirvana.log")).unwrap_or_default();
        assert.append_context("nirvana.log", log)
    }

    fn command(&self, args: &[&str]) -> process::Command {
        let mut command = process::Command::new(&self.bin);
        command
            .args(args)
            .env_remove("NIRVANA_DEV")
            .env("NIRVANA_CONFIG_DIR", self.dir.path())
            .env("NIRVANA_DATA_DIR", self.dir.path())
            .env("NIRVANA_LOG_DIR", self.dir.path());
        command
    }

    /// Runs `serve` on a free port and waits until it accepts connections.
    pub fn serve(&self) -> Server {
        let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .and_then(|listener| listener.local_addr())
            .unwrap()
            .port();
        self.run(&["config", "set", "core.port", &port.to_string()]);
        let child = self
            .command(&["serve"])
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let server = Server { child, port };
        for _ in 0..50 {
            if TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_ok() {
                return server;
            }
            thread::sleep(Duration::from_millis(100));
        }
        panic!("nirvana serve is not listening on port {port}");
    }

    pub fn setup(&self) {
        self.run(&["profile", "create", "e2e", "--switch"]);
    }

    pub fn add_jira(&self, config: &Config) {
        let credential = format!("{}:NIRVANA_E2E_JIRA_TOKEN", config.jira_email);
        self.run(&[
            "connection",
            "add",
            "jira",
            "--kind",
            "jira-cloud",
            "--host",
            &config.jira_host,
            "--credential",
            &credential,
            "--storage",
            "env",
        ]);
    }

    pub fn add_linear(&self, config: &Config) {
        let host = format!("linear.app/{}", config.linear_workspace);
        self.run(&[
            "connection",
            "add",
            "linear",
            "--kind",
            "linear",
            "--host",
            &host,
            "--credential",
            "NIRVANA_E2E_LINEAR_TOKEN",
            "--storage",
            "env",
        ]);
    }

    pub fn get(&self, connection: &str, key: &str) -> String {
        let stdout = self.run(&["settings", "get", "--connection", connection, key]);
        stdout.trim().to_string()
    }

    pub fn set(&self, connection: &str, key: &str, value: &str) {
        self.run(&["settings", "set", "--connection", connection, key, value]);
    }

    /// Returns the key, start and publish marker of each slot listed on `day`.
    pub fn list_rows(&self, day: NaiveDate) -> Vec<Row> {
        self.rows(day, &[])
    }

    /// Returns the rows of [`Self::list_rows`] ordered by ticket, with the key
    /// shown only on the first slot of each ticket.
    pub fn list_rows_by_ticket(&self, day: NaiveDate) -> Vec<Row> {
        self.rows(day, &["--by-ticket"])
    }

    /// Returns the ID of each slot listed on `day`.
    pub fn slot_ids(&self, day: NaiveDate) -> Vec<String> {
        self.list(day, &["--show-ids"])
            .map_or_else(Vec::new, |stdout| first_column(&stdout))
    }

    /// Returns the `list` output of `day`, or `None` if it has no slots.
    fn list(&self, day: NaiveDate, extra: &[&str]) -> Option<String> {
        let (from, to) = (at(day, "00:00"), at(day + Days::new(1), "00:00"));
        let mut args = vec!["list", "--from", &from, "--to", &to];
        args.extend(extra);
        let stdout = self.run(&args);
        (stdout.trim() != "No slots in this range").then_some(stdout)
    }

    fn rows(&self, day: NaiveDate, extra: &[&str]) -> Vec<Row> {
        let Some(stdout) = self.list(day, extra) else {
            return Vec::new();
        };
        let mut lines = stdout.lines();
        let header = lines.next().unwrap_or_default();
        let column = |name: &str| {
            header
                .find(name)
                .unwrap_or_else(|| panic!("no {name} column in:\n{stdout}"))
        };
        let (title, start, published) = (column("TITLE"), column("START"), column("PUBLISHED"));
        lines
            .map(|line| {
                let chars: Vec<char> = line.chars().collect();
                let cell = |from: usize, to: usize| {
                    let to = to.min(chars.len());
                    let from = from.min(to);
                    chars[from..to]
                        .iter()
                        .collect::<String>()
                        .trim()
                        .to_string()
                };
                (
                    cell(0, title),
                    cell(start, start + "YYYY-MM-DD HH:MM".len()),
                    cell(published, chars.len()),
                )
            })
            .collect()
    }
}

/// A running `nirvana serve`, killed on drop.
pub struct Server {
    child: Child,
    pub port: u16,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
