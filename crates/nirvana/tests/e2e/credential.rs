use crate::helpers::{Config, Nirvana, line_with};

#[tokio::test]
#[ignore]
async fn lists_credentials() {
    let config = Config::from_env();

    let nirvana = Nirvana::new();
    nirvana.setup();
    let credentials = nirvana.run(&["credential", "list"]);
    assert!(
        credentials.contains("No credentials found"),
        "{credentials}"
    );

    nirvana.add_jira(&config);
    nirvana.add_linear(&config);
    let credentials = nirvana.run(&["credential", "list"]);
    let lines: Vec<Vec<&str>> = credentials
        .lines()
        .map(|line| line.split_whitespace().collect())
        .collect();
    assert_eq!(
        lines,
        [
            vec!["NAME", "STORAGE", "ACCOUNT"],
            vec!["jira", "env", "NIRVANA_E2E_JIRA_TOKEN"],
            vec!["linear", "env", "NIRVANA_E2E_LINEAR_TOKEN"],
        ]
    );
}

#[tokio::test]
#[ignore]
async fn env_credentials_cannot_be_updated_or_moved() {
    let config = Config::from_env();

    let nirvana = Nirvana::connected(&config);

    nirvana.fail(
        &[
            "credential",
            "update",
            "linear",
            "--token",
            "NIRVANA_E2E_UNSET",
        ],
        "unsupported secret store: env",
    );
    nirvana.fail(
        &["credential", "move", "jira", "--to", "plaintext"],
        "unsupported secret store: env",
    );

    let credentials = nirvana.run(&["credential", "list"]);
    for (connection, variable) in [
        ("jira", "NIRVANA_E2E_JIRA_TOKEN"),
        ("linear", "NIRVANA_E2E_LINEAR_TOKEN"),
    ] {
        let line = line_with(&credentials, connection);
        assert!(
            line.contains("env") && line.contains(variable),
            "{credentials}"
        );
    }
}
