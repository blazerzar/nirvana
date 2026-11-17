use chrono::Days;

use crate::helpers::{
    Config, Jira, Linear, Nirvana, all_published, at, day, row, timestamp, title,
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
