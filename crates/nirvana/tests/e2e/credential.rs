use crate::helpers::{E2e, line_with};

#[tokio::test]
#[ignore]
async fn lists_credentials() {
    let e2e = E2e::new("lists_credentials");
    let (config, nirvana) = (&e2e.config, &e2e.nirvana);

    let credentials = nirvana.run(&["credential", "list"]);
    assert!(
        credentials.contains("No credentials found"),
        "{credentials}"
    );

    nirvana.add_jira(config);
    nirvana.add_linear(config);
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
    let e2e = E2e::connected("env_credentials_cannot_be_updated_or_moved");
    let nirvana = &e2e.nirvana;

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
    for command in ["update", "move"] {
        let output = nirvana.run(&["credential", command]);
        assert!(output.contains("No credentials found"), "{output}");
    }

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

#[tokio::test]
#[ignore]
async fn updates_plaintext_credentials() {
    let e2e = E2e::new("updates_plaintext_credentials");
    let (config, nirvana) = (&e2e.config, &e2e.nirvana);

    let host = format!("linear.app/{}", config.linear_workspace);
    nirvana.run(&[
        "connection",
        "add",
        "linear",
        "--kind",
        "linear",
        "--host",
        &host,
        "--credential",
        "lin_api_e2e_first",
        "--storage",
        "plaintext",
    ]);
    let credentials = nirvana.run(&["credential", "list"]);
    let line: Vec<&str> = line_with(&credentials, "linear")
        .split_whitespace()
        .collect();
    assert_eq!(line, ["linear", "plaintext"], "{credentials}");
    nirvana.fail(&["connection", "test", "linear"], "authentication failed");

    let updated = nirvana.run(&[
        "credential",
        "update",
        "linear",
        "--token",
        "lin_api_e2e_second",
    ]);
    assert!(
        updated.contains("Updated credential of linear"),
        "{updated}"
    );
    nirvana.fail(&["connection", "test", "linear"], "authentication failed");
}
