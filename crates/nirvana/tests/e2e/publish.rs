use chrono::Days;

use crate::helpers::{
    Config, Jira, Linear, Nirvana, all_published, at, comment, day, row, sorted, timestamp, title,
};

#[tokio::test]
#[ignore]
async fn publishes_a_range() {
    const TEST: &str = "publishes_a_range";
    let config = Config::from_env();
    let (jira, linear) = (Jira::new(&config), Linear::new(&config));
    let (d2, d) = (day(2), day(1));

    let j1 = jira.create_issue(&title(TEST, "J1")).await;

    let nirvana = Nirvana::connected(&config, linear.workspace().await);
    nirvana.run(&["add", &j1, "--start", &at(d2, "09:00"), "--duration", "30m"]);
    nirvana.run(&["add", &j1, "--start", &at(d, "09:00"), "--duration", "30m"]);
    nirvana.run(&["add", &j1, "--start", &at(d, "11:00"), "--duration", "15m"]);

    let published = nirvana.run(&[
        "publish",
        "--start",
        &at(d, "00:00"),
        "--stop",
        &at(d, "10:00"),
    ]);
    assert!(published.contains("Published 1 slot (30m)"), "{published}");
    assert_eq!(jira.worklogs(&j1).await, [(timestamp(d, "09:00"), 1800)]);
    assert_eq!(nirvana.list_rows(d2), [row(&j1, at(d2, "09:00"), "")]);

    let published = nirvana.run(&[
        "publish",
        "--start",
        &at(d, "00:00"),
        "--stop",
        &at(d + Days::new(1), "00:00"),
    ]);
    assert!(published.contains("Published 1 slot (15m)"), "{published}");
    let published = nirvana.run(&["publish"]);
    assert!(published.contains("Nothing to publish"), "{published}");

    let published = nirvana.run(&["publish", "--all"]);
    assert!(published.contains("Published 1 slot (30m)"), "{published}");
    assert_eq!(
        jira.worklogs(&j1).await,
        [
            (timestamp(d2, "09:00"), 1800),
            (timestamp(d, "09:00"), 1800),
            (timestamp(d, "11:00"), 900),
        ]
    );
    assert!(all_published(nirvana.list_rows(d2)));
    assert!(all_published(nirvana.list_rows(d)));
}

#[tokio::test]
#[ignore]
async fn jira_and_linear_publish_together() {
    const TEST: &str = "jira_and_linear_publish_together";
    let config = Config::from_env();
    let (jira, linear) = (Jira::new(&config), Linear::new(&config));
    let d = day(1);

    let j1 = jira.create_issue(&title(TEST, "J1")).await;
    let j2 = jira.create_issue(&title(TEST, "J2")).await;
    let l1 = linear.create_issue(&title(TEST, "L1")).await;
    let l2 = linear.create_issue(&title(TEST, "L2")).await;

    let nirvana = Nirvana::connected(&config, linear.workspace().await);
    nirvana.set("jira", "publish", "true");
    nirvana.set("jira", "squash", "true");
    nirvana.set("linear", "publish", "true");

    nirvana.run(&["start", &j1, "--at", &at(d, "09:00")]);
    nirvana.run(&["start", &l1, "--at", &at(d, "09:40")]);
    nirvana.run(&["switch", "--at", &at(d, "10:10")]);
    nirvana.run(&["start", &l2, "--at", &at(d, "10:30")]);
    nirvana.run(&["stop", "--at", &at(d, "11:00")]);
    nirvana.run(&["resume", "--at", &at(d, "11:25")]);
    nirvana.run(&["start", &j2, "--at", &at(d, "11:45")]);
    nirvana.run(&["stop", "--at", &at(d, "12:15")]);

    let published = nirvana.run(&["publish", "--all", "--target", "jira"]);
    assert!(
        published.contains("Published 3 slots (1h 30m)"),
        "{published}"
    );
    let j1_worklogs = vec![(timestamp(d, "09:00"), 3600)];
    let j2_worklogs = vec![(timestamp(d, "11:45"), 1800)];
    assert_eq!(jira.worklogs(&j1).await, j1_worklogs);
    assert_eq!(jira.worklogs(&j2).await, j2_worklogs);
    assert!(linear.comments(&l1).await.is_empty());
    assert!(linear.comments(&l2).await.is_empty());
    for (key, _, marker) in nirvana.list_rows(d) {
        let expected = if key == j1 || key == j2 { "✓" } else { "" };
        assert_eq!(marker, expected, "{key}");
    }

    let published = nirvana.run(&["publish", "--all"]);
    assert!(
        published.contains("Published 3 slots (1h 20m)"),
        "{published}"
    );
    assert_eq!(linear.comments(&l1).await, [comment(d, "09:40", "30m")]);
    assert_eq!(
        linear.comments(&l2).await,
        sorted([comment(d, "10:30", "30m"), comment(d, "11:25", "20m")])
    );
    assert_eq!(jira.worklogs(&j1).await, j1_worklogs);
    assert_eq!(jira.worklogs(&j2).await, j2_worklogs);
    assert!(all_published(nirvana.list_rows(d)));

    let published = nirvana.run(&["publish", "--all"]);
    assert!(published.contains("Nothing to publish"), "{published}");
}

#[tokio::test]
#[ignore]
async fn jira_squashes_per_ticket_and_day() {
    const TEST: &str = "jira_squashes_per_ticket_and_day";
    let config = Config::from_env();
    let (jira, linear) = (Jira::new(&config), Linear::new(&config));
    let (d2, d) = (day(2), day(1));

    let j1 = jira.create_issue(&title(TEST, "J1")).await;
    let j2 = jira.create_issue(&title(TEST, "J2")).await;
    let j3 = jira.create_issue(&title(TEST, "J3")).await;

    let nirvana = Nirvana::connected(&config, linear.workspace().await);
    nirvana.set("jira", "publish", "true");
    nirvana.set("jira", "squash", "true");

    nirvana.run(&["start", &j1, "--at", &at(d2, "09:00")]);
    nirvana.run(&["stop", "--at", &at(d2, "09:45")]);
    nirvana.run(&["add", &j2, "--start", &at(d2, "10:00"), "--duration", "1h"]);

    nirvana.run(&["start", &j1, "--at", &at(d, "09:00")]);
    nirvana.run(&["start", &j2, "--at", &at(d, "09:20")]);
    nirvana.run(&["switch", "--at", &at(d, "09:50")]);
    nirvana.run(&["stop", "--at", &at(d, "10:30")]);
    nirvana.run(&["resume", "--at", &at(d, "10:45")]);
    nirvana.run(&["start", &j3, "--at", &at(d, "11:00")]);
    nirvana.run(&["stop", "--at", &at(d, "11:30")]);

    let published = nirvana.run(&["publish", "--all"]);
    assert!(
        published.contains("Published 7 slots (4h 0m)"),
        "{published}"
    );

    let j1_worklogs = vec![
        (timestamp(d2, "09:00"), 2700),
        (timestamp(d, "09:00"), 4500),
    ];
    let j2_worklogs = vec![
        (timestamp(d2, "10:00"), 3600),
        (timestamp(d, "09:20"), 1800),
    ];
    assert_eq!(jira.worklogs(&j1).await, j1_worklogs);
    assert_eq!(jira.worklogs(&j2).await, j2_worklogs);
    assert_eq!(jira.worklogs(&j3).await, [(timestamp(d, "11:00"), 1800)]);
    for day in [d2, d] {
        assert!(all_published(nirvana.list_rows(day)));
    }

    nirvana.run(&["add", &j3, "--start", &at(d, "12:00"), "--duration", "10m"]);
    let published = nirvana.run(&["publish", "--all"]);
    assert!(published.contains("Published 1 slot (10m)"), "{published}");
    assert_eq!(
        jira.worklogs(&j3).await,
        [(timestamp(d, "11:00"), 1800), (timestamp(d, "12:00"), 600)]
    );

    let published = nirvana.run(&["publish", "--all"]);
    assert!(published.contains("Nothing to publish"), "{published}");
    assert_eq!(jira.worklogs(&j1).await, j1_worklogs);
    assert_eq!(jira.worklogs(&j2).await, j2_worklogs);
}

#[tokio::test]
#[ignore]
async fn linear_squashes_per_ticket_and_day() {
    const TEST: &str = "linear_squashes_per_ticket_and_day";
    let config = Config::from_env();
    let linear = Linear::new(&config);
    let (d2, d) = (day(2), day(1));

    let l1 = linear.create_issue(&title(TEST, "L1")).await;
    let l2 = linear.create_issue(&title(TEST, "L2")).await;
    let l3 = linear.create_issue(&title(TEST, "L3")).await;

    let nirvana = Nirvana::connected(&config, linear.workspace().await);
    nirvana.set("linear", "publish", "true");
    nirvana.set("linear", "squash", "true");

    nirvana.run(&["start", &l1, "--at", &at(d2, "09:00")]);
    nirvana.run(&["stop", "--at", &at(d2, "09:45")]);
    nirvana.run(&["add", &l2, "--start", &at(d2, "10:00"), "--duration", "1h"]);

    nirvana.run(&["start", &l1, "--at", &at(d, "09:00")]);
    nirvana.run(&["start", &l2, "--at", &at(d, "09:20")]);
    nirvana.run(&["switch", "--at", &at(d, "09:50")]);
    nirvana.run(&["stop", "--at", &at(d, "10:30")]);
    nirvana.run(&["resume", "--at", &at(d, "10:45")]);
    nirvana.run(&["start", &l3, "--at", &at(d, "11:00")]);
    nirvana.run(&["stop", "--at", &at(d, "11:30")]);

    let published = nirvana.run(&["publish", "--all"]);
    assert!(
        published.contains("Published 7 slots (4h 0m)"),
        "{published}"
    );

    let l1_comments = sorted([comment(d2, "09:00", "45m"), comment(d, "09:00", "1h 15m")]);
    let l2_comments = sorted([comment(d2, "10:00", "1h"), comment(d, "09:20", "30m")]);
    assert_eq!(linear.comments(&l1).await, l1_comments);
    assert_eq!(linear.comments(&l2).await, l2_comments);
    assert_eq!(linear.comments(&l3).await, [comment(d, "11:00", "30m")]);
    for day in [d2, d] {
        assert!(all_published(nirvana.list_rows(day)));
    }

    nirvana.run(&["add", &l3, "--start", &at(d, "12:00"), "--duration", "10m"]);
    let published = nirvana.run(&["publish", "--all"]);
    assert!(published.contains("Published 1 slot (10m)"), "{published}");
    assert_eq!(
        linear.comments(&l3).await,
        sorted([comment(d, "11:00", "30m"), comment(d, "12:00", "10m")])
    );

    let published = nirvana.run(&["publish", "--all"]);
    assert!(published.contains("Nothing to publish"), "{published}");
    assert_eq!(linear.comments(&l1).await, l1_comments);
    assert_eq!(linear.comments(&l2).await, l2_comments);
}
