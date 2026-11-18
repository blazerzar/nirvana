use std::env;
use std::sync::LazyLock;

use chrono::Local;

use crate::helpers::{Config, Jira, Linear, Nirvana};

/// The config, clients and profile of the test `test`.
pub struct E2e {
    pub config: Config,
    pub jira: Jira,
    pub linear: Linear,
    pub nirvana: Nirvana,
    test: &'static str,
}

impl E2e {
    /// Returns the fixture of `test` with a profile without connections.
    pub fn new(test: &'static str) -> Self {
        let config = Config::from_env();
        let nirvana = Nirvana::new();
        nirvana.setup();
        Self {
            jira: Jira::new(&config),
            linear: Linear::new(&config),
            nirvana,
            config,
            test,
        }
    }

    /// Returns the fixture of `test` with a profile with the Jira connection
    /// `jira` and the Linear connection `linear`.
    pub fn connected(test: &'static str) -> Self {
        let e2e = Self::new(test);
        e2e.nirvana.add_jira(&e2e.config);
        e2e.nirvana.add_linear(&e2e.config);
        e2e
    }

    /// Returns a unique issue title for issue `name` of this test in this run.
    pub fn title(&self, name: &str) -> String {
        static RUN: LazyLock<String> = LazyLock::new(|| {
            env::var("GITHUB_RUN_ID")
                .unwrap_or_else(|_| Local::now().format("%Y%m%d%H%M%S").to_string())
        });
        let run = &*RUN;
        format!(
            "[e2e] {name} {}-{} {run} {}",
            env::consts::OS,
            env::consts::ARCH,
            self.test
        )
    }

    pub async fn jira_issue(&self, name: &str) -> String {
        self.jira.create_issue(&self.title(name)).await
    }

    pub async fn linear_issue(&self, name: &str) -> String {
        self.linear.create_issue(&self.title(name)).await
    }
}
