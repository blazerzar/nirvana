use crate::helpers::{Config, Jira, Linear, Nirvana, at, day, line_with, row, title};

#[tokio::test]
#[ignore]
async fn jira_connects() {
    let config = Config::from_env();

    let nirvana = Nirvana::new();
    nirvana.setup();
    nirvana.add_jira(&config);
    let credentials = nirvana.run(&["credential", "list"]);
    let jira = line_with(&credentials, "jira");
    assert!(jira.contains("env"), "{credentials}");
    assert!(jira.contains("NIRVANA_E2E_JIRA_TOKEN"), "{credentials}");
    nirvana.run(&["connection", "test", "jira"]);
}

#[tokio::test]
#[ignore]
async fn linear_connects() {
    let config = Config::from_env();
    let linear = Linear::new(&config);

    let nirvana = Nirvana::new();
    nirvana.setup();
    nirvana.add_linear(linear.workspace().await);
    let credentials = nirvana.run(&["credential", "list"]);
    let linear = line_with(&credentials, "linear");
    assert!(linear.contains("env"), "{credentials}");
    assert!(linear.contains("NIRVANA_E2E_LINEAR_TOKEN"), "{credentials}");
    nirvana.run(&["connection", "test", "linear"]);
}

#[tokio::test]
#[ignore]
async fn lists_connections() {
    let config = Config::from_env();
    let linear = Linear::new(&config);

    let workspace = linear.workspace().await;
    let nirvana = Nirvana::connected(&config, workspace);

    let connections = nirvana.run(&["connection", "list"]);

    let jira_connection = line_with(&connections, "jira-cloud");
    assert!(jira_connection.starts_with("jira"), "{connections}");
    assert!(jira_connection.contains("env"), "{connections}");

    let linear_host = format!("linear.app/{workspace}");
    let linear_connection = line_with(&connections, &linear_host);
    assert!(linear_connection.starts_with("linear"), "{connections}");
    assert!(linear_connection.contains("env"), "{connections}");
}

#[tokio::test]
#[ignore]
async fn tests_all_connections() {
    let config = Config::from_env();
    let linear = Linear::new(&config);

    let nirvana = Nirvana::connected(&config, linear.workspace().await);

    let tested = nirvana.run(&["connection", "test", "--all"]);
    assert_eq!(tested.lines().count(), 2, "{tested}");
    for connection in ["jira", "linear"] {
        assert!(line_with(&tested, connection).starts_with('✓'), "{tested}");
    }
}

#[tokio::test]
#[ignore]
async fn edits_connections() {
    let config = Config::from_env();
    let linear = Linear::new(&config);

    let nirvana = Nirvana::connected(&config, linear.workspace().await);

    nirvana.run(&[
        "connection",
        "edit",
        "jira",
        "--identity",
        "nobody@example.com",
    ]);
    nirvana.fail(
        &["connection", "test", "jira"],
        "authentication failed: 401 Unauthorized",
    );
    nirvana.run(&[
        "connection",
        "edit",
        "jira",
        "--identity",
        &config.jira_email,
    ]);
    nirvana.run(&["connection", "test", "jira"]);

    nirvana.run(&["connection", "edit", "jira", "--host", "http://127.0.0.1:9"]);
    nirvana.fail(&["connection", "test", "jira"], "network error");
    nirvana.run(&["connection", "edit", "jira", "--host", &config.jira_host]);
    nirvana.run(&["connection", "test", "jira"]);

    nirvana.run(&["connection", "edit", "jira", "--name", "work"]);
    let connections = nirvana.run(&["connection", "list"]);
    assert!(
        line_with(&connections, "jira-cloud").starts_with("work"),
        "{connections}"
    );
    nirvana.fail(
        &["connection", "test", "jira"],
        "connection 'jira' not found",
    );
    nirvana.run(&["connection", "test", "work"]);
    assert_eq!(nirvana.get("work", "publish"), "true");

    nirvana.fail(
        &[
            "connection",
            "edit",
            "linear",
            "--identity",
            "nobody@example.com",
        ],
        "does not use an identity",
    );
}

#[tokio::test]
#[ignore]
async fn deletes_connections() {
    const TEST: &str = "deletes_connections";
    let config = Config::from_env();
    let (jira, linear) = (Jira::new(&config), Linear::new(&config));
    let d = day(1);

    let j1 = jira.create_issue(&title(TEST, "J1")).await;
    let l1 = linear.create_issue(&title(TEST, "L1")).await;

    let nirvana = Nirvana::connected(&config, linear.workspace().await);
    nirvana.run(&["add", &j1, "--start", &at(d, "09:00"), "--duration", "30m"]);
    nirvana.run(&["add", &l1, "--start", &at(d, "10:00"), "--duration", "30m"]);

    nirvana.run(&["connection", "delete", "linear", "-y"]);
    let tickets = nirvana.run(&["ticket", "list", "--no-connection"]);
    assert!(tickets.contains(&l1) && !tickets.contains(&j1), "{tickets}");
    assert_eq!(
        nirvana.list_rows(d),
        [row(&j1, at(d, "09:00"), ""), row(&l1, at(d, "10:00"), "")]
    );

    nirvana.run(&["connection", "delete", "jira", "--with-tickets", "-y"]);
    let tickets = nirvana.run(&["ticket", "list"]);
    assert!(tickets.contains(&l1) && !tickets.contains(&j1), "{tickets}");
    assert_eq!(nirvana.list_rows(d), [row(&l1, at(d, "10:00"), "")]);
    let connections = nirvana.run(&["connection", "list"]);
    assert!(
        connections.contains("No connections found"),
        "{connections}"
    );
}
