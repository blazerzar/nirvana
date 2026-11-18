use crate::helpers::{E2e, at, day, line_with, row};

#[tokio::test]
#[ignore]
async fn jira_connects() {
    let e2e = E2e::new("jira_connects");
    let (config, nirvana) = (&e2e.config, &e2e.nirvana);

    nirvana.add_jira(config);
    nirvana.run(&["connection", "test", "jira"]);
}

#[tokio::test]
#[ignore]
async fn linear_connects() {
    let e2e = E2e::new("linear_connects");
    let (config, nirvana) = (&e2e.config, &e2e.nirvana);

    nirvana.add_linear(config);
    nirvana.run(&["connection", "test", "linear"]);
}

#[tokio::test]
#[ignore]
async fn lists_connections() {
    let e2e = E2e::connected("lists_connections");
    let (config, nirvana) = (&e2e.config, &e2e.nirvana);

    let connections = nirvana.run(&["connection", "list"]);

    let jira_connection = line_with(&connections, "jira-cloud");
    assert!(jira_connection.starts_with("jira"), "{connections}");
    assert!(jira_connection.contains("env"), "{connections}");

    let linear_host = format!("linear.app/{}", config.linear_workspace);
    let linear_connection = line_with(&connections, &linear_host);
    assert!(linear_connection.starts_with("linear"), "{connections}");
    assert!(linear_connection.contains("env"), "{connections}");
}

#[tokio::test]
#[ignore]
async fn tests_all_connections() {
    let e2e = E2e::connected("tests_all_connections");
    let nirvana = &e2e.nirvana;

    let tested = nirvana.run(&["connection", "test", "--all"]);
    assert_eq!(tested.lines().count(), 2, "{tested}");
    for connection in ["jira", "linear"] {
        assert!(line_with(&tested, connection).starts_with('✓'), "{tested}");
    }
}

#[tokio::test]
#[ignore]
async fn edits_connections() {
    let e2e = E2e::connected("edits_connections");
    let (config, nirvana) = (&e2e.config, &e2e.nirvana);

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

    nirvana.set("jira", "squash", "true");
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
    assert_eq!(nirvana.get("work", "squash"), "true");

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
    let e2e = E2e::connected("deletes_connections");
    let nirvana = &e2e.nirvana;
    let d = day(1);

    let j1 = e2e.jira_issue("J1").await;
    let l1 = e2e.linear_issue("L1").await;

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
