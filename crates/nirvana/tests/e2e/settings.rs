use crate::helpers::{E2e, at, comment, day, line_with, row, timestamp};

#[tokio::test]
#[ignore]
async fn publish_settings() {
    let e2e = E2e::connected("publish_settings");
    let (jira, linear, nirvana) = (&e2e.jira, &e2e.linear, &e2e.nirvana);
    let d = day(1);

    let j1 = e2e.jira_issue("J1").await;
    let l1 = e2e.linear_issue("L1").await;

    assert_eq!(nirvana.get("jira", "publish"), "true");
    assert_eq!(nirvana.get("jira", "squash"), "false");
    assert_eq!(nirvana.get("linear", "publish"), "false");
    assert_eq!(nirvana.get("linear", "squash"), "false");
    let settings = nirvana.run(&["settings", "get", "--connection", "jira"]);
    assert!(
        line_with(&settings, "publish").ends_with("true"),
        "{settings}"
    );
    assert!(
        line_with(&settings, "squash").ends_with("false"),
        "{settings}"
    );

    nirvana.fail(
        &["settings", "set", "--connection", "jira", "unknown", "true"],
        "unknown setting 'unknown'",
    );
    nirvana.fail(
        &["settings", "set", "--connection", "jira", "publish", "yes"],
        "must be true or false",
    );
    assert_eq!(nirvana.get("jira", "publish"), "true");

    nirvana.run(&["add", &j1, "--start", &at(d, "09:00"), "--duration", "30m"]);
    nirvana.run(&["add", &l1, "--start", &at(d, "10:00"), "--duration", "30m"]);

    nirvana.fail(
        &["publish", "--all", "--target", "linear"],
        "publishing is disabled for connection 'linear'",
    );
    let published = nirvana.run(&["publish", "--all"]);
    assert!(published.contains("Published 1 slot (30m)"), "{published}");
    jira.assert_worklogs(&j1, &[(timestamp(d, "09:00"), 1800)])
        .await;
    linear.assert_comments(&l1, &[]).await;
    assert_eq!(
        nirvana.list_rows(d),
        [row(&j1, at(d, "09:00"), "✓"), row(&l1, at(d, "10:00"), "")]
    );

    nirvana.set("jira", "publish", "false");
    nirvana.set("linear", "publish", "true");
    assert_eq!(nirvana.get("jira", "publish"), "false");
    assert_eq!(nirvana.get("linear", "publish"), "true");
    nirvana.run(&["add", &j1, "--start", &at(d, "11:00"), "--duration", "15m"]);

    nirvana.fail(
        &["publish", "--all", "--target", "jira"],
        "publishing is disabled for connection 'jira'",
    );
    let published = nirvana.run(&["publish", "--all"]);
    assert!(published.contains("Published 1 slot (30m)"), "{published}");
    linear
        .assert_comments(&l1, &[comment(d, "10:00", "30m")])
        .await;
    jira.assert_worklogs(&j1, &[(timestamp(d, "09:00"), 1800)])
        .await;
    assert_eq!(
        nirvana.list_rows(d),
        [
            row(&j1, at(d, "09:00"), "✓"),
            row(&l1, at(d, "10:00"), "✓"),
            row(&j1, at(d, "11:00"), ""),
        ]
    );
}
