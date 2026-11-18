use crate::helpers::{Config, Jira, Linear, Nirvana, at, day, row, shown, title};

#[tokio::test]
#[ignore]
async fn jira_tracks_slots() {
    const TEST: &str = "jira_tracks_slots";
    let config = Config::from_env();
    let jira = Jira::new(&config);
    let d = day(1);

    let j1 = jira.create_issue(&title(TEST, "J1")).await;
    let j2 = jira.create_issue(&title(TEST, "J2")).await;
    let j3 = jira.create_issue(&title(TEST, "J3")).await;

    let nirvana = Nirvana::new();
    nirvana.setup();
    nirvana.add_jira(&config);

    let tracking_j1 = format!("Tracking ticket {j1} {} ", title(TEST, "J1"));

    nirvana.run(&["start", &j1, "--at", &at(d, "09:00")]);
    let status = nirvana.run(&["status"]);
    assert!(status.starts_with(&tracking_j1), "{status}");
    nirvana.run(&["start", &j2, "--at", &at(d, "09:30")]);
    nirvana.run(&["switch", "--at", &at(d, "10:15")]);
    let status = nirvana.run(&["status"]);
    assert!(status.starts_with(&tracking_j1), "{status}");
    nirvana.run(&["stop", "--at", &at(d, "10:45")]);
    let status = nirvana.run(&["status"]);
    assert!(status.contains("Not tracking"), "{status}");
    nirvana.run(&["resume", "--at", &at(d, "11:00")]);
    nirvana.run(&["stop", "--at", &at(d, "11:20")]);
    nirvana.run(&["start", &j3, "--at", &at(d, "13:00")]);
    nirvana.run(&["stop", "--at", &at(d, "13:10")]);

    assert_eq!(
        nirvana.list_rows(d),
        [
            row(&j1, at(d, "09:00"), ""),
            row(&j2, at(d, "09:30"), ""),
            row(&j1, at(d, "10:15"), ""),
            row(&j1, at(d, "11:00"), ""),
            row(&j3, at(d, "13:00"), ""),
        ]
    );
    let tickets = nirvana.run(&["ticket", "list"]);
    for name in ["J1", "J2", "J3"] {
        assert!(tickets.contains(&shown(&title(TEST, name))), "{tickets}");
    }
}

#[tokio::test]
#[ignore]
async fn edits_adds_and_deletes_slots() {
    const TEST: &str = "edits_adds_and_deletes_slots";
    let config = Config::from_env();
    let (jira, linear) = (Jira::new(&config), Linear::new(&config));
    let d = day(1);

    let j1 = jira.create_issue(&title(TEST, "J1")).await;
    let j2 = jira.create_issue(&title(TEST, "J2")).await;
    let l1 = linear.create_issue(&title(TEST, "L1")).await;

    let nirvana = Nirvana::connected(&config, linear.workspace().await);

    nirvana.run(&["add", &j1, "--start", &at(d, "09:00"), "--duration", "1h"]);
    let (add_start, add_stop) = (at(d, "10:00"), at(d, "10:30"));
    nirvana.run(&["add", &l1, "--start", &add_start, "--stop", &add_stop]);
    nirvana.run(&["add", &j2, "--start", &at(d, "11:00"), "--duration", "15m"]);
    nirvana.fail(
        &["add", &j2, "--start", &at(d, "09:30"), "--duration", "10m"],
        "slot overlaps an existing slot",
    );

    nirvana.run(&["edit", "--at", &at(d, "09:30"), "--start", "09:15"]);
    nirvana.run(&["edit", "--at", &at(d, "10:10"), "--ticket", &j2]);
    let id = &nirvana.slot_ids(d)[2];
    nirvana.run(&["edit", id, "--duration", "30m"]);

    nirvana.run(&["add", &l1, "--start", &at(d, "13:00"), "--duration", "20m"]);
    nirvana.run(&["delete", "--at", &at(d, "13:10"), "-y"]);
    nirvana.run(&["add", &j1, "--start", &at(d, "14:00"), "--duration", "5m"]);
    let id = &nirvana.slot_ids(d)[3];
    nirvana.run(&["delete", id, "-y"]);

    assert_eq!(
        nirvana.list_rows(d),
        [
            row(&j1, at(d, "09:15"), ""),
            row(&j2, at(d, "10:00"), ""),
            row(&j2, at(d, "11:00"), ""),
        ]
    );

    nirvana.run(&["edit", "--at", &at(d, "11:10"), "--stop", ""]);
    let status = nirvana.run(&["status"]);
    let tracking_j2 = format!("Tracking ticket {j2} {} ", title(TEST, "J2"));
    assert!(status.starts_with(&tracking_j2), "{status}");
    nirvana.run(&["stop", "--at", &at(d, "11:30")]);
    let status = nirvana.run(&["status"]);
    assert!(status.contains("Not tracking"), "{status}");
}

#[tokio::test]
#[ignore]
async fn lists_slots() {
    const TEST: &str = "lists_slots";
    let config = Config::from_env();
    let (jira, linear) = (Jira::new(&config), Linear::new(&config));
    let (d2, d) = (day(2), day(1));

    let j1 = jira.create_issue(&title(TEST, "J1")).await;
    let j2 = jira.create_issue(&title(TEST, "J2")).await;
    let l1 = linear.create_issue(&title(TEST, "L1")).await;

    let nirvana = Nirvana::connected(&config, linear.workspace().await);
    let empty = nirvana.run(&["list"]);
    assert!(empty.contains("No slots in this range"), "{empty}");

    nirvana.run(&["add", &j1, "--start", &at(d2, "09:00"), "--duration", "30m"]);
    nirvana.run(&["add", &j1, "--start", &at(d, "09:00"), "--duration", "30m"]);
    nirvana.run(&["add", &l1, "--start", &at(d, "10:00"), "--duration", "30m"]);
    nirvana.run(&["add", &j2, "--start", &at(d, "10:30"), "--duration", "15m"]);
    nirvana.run(&["add", &j1, "--start", &at(d, "11:00"), "--duration", "15m"]);

    assert_eq!(nirvana.list_rows(d2), [row(&j1, at(d2, "09:00"), "")]);
    assert_eq!(
        nirvana.list_rows(d),
        [
            row(&j1, at(d, "09:00"), ""),
            row(&l1, at(d, "10:00"), ""),
            row(&j2, at(d, "10:30"), ""),
            row(&j1, at(d, "11:00"), ""),
        ]
    );
    assert_eq!(
        nirvana.list_rows_by_ticket(d),
        [
            row(&j1, at(d, "09:00"), ""),
            row("", at(d, "11:00"), ""),
            row(&l1, at(d, "10:00"), ""),
            row(&j2, at(d, "10:30"), ""),
        ]
    );
    let ids = nirvana.slot_ids(d);
    assert_eq!(ids.len(), 4);
    assert!(ids.iter().all(|id| id.parse::<i64>().is_ok()), "{ids:?}");

    let slot = nirvana.run(&["show", &ids[1]]);
    assert!(slot.contains(&l1), "{slot}");
    assert!(
        slot.contains(&format!("{} - {}", at(d, "10:00"), at(d, "10:30"))),
        "{slot}"
    );
    assert!(slot.contains("Not published"), "{slot}");
}

#[tokio::test]
#[ignore]
async fn tracks_tickets_of_a_connection() {
    const TEST: &str = "tracks_tickets_of_a_connection";
    let config = Config::from_env();
    let (jira, linear) = (Jira::new(&config), Linear::new(&config));
    let d = day(1);

    let j1 = jira.create_issue(&title(TEST, "J1")).await;
    let j2 = jira.create_issue(&title(TEST, "J2")).await;
    let l1 = linear.create_issue(&title(TEST, "L1")).await;

    let nirvana = Nirvana::connected(&config, linear.workspace().await);

    let start = ["start", &j1, "--at", &at(d, "09:00")];
    nirvana.fail(
        &[&start[..], &["--connection", "linear"]].concat(),
        &format!("ticket not found: {j1}"),
    );
    nirvana.run(&[&start[..], &["--connection", "jira"]].concat());
    nirvana.run(&["stop", "--at", &at(d, "09:30")]);

    let add_start = at(d, "10:00");
    let add = ["add", &l1, "--start", &add_start, "--duration", "30m"];
    nirvana.fail(
        &[&add[..], &["--connection", "jira"]].concat(),
        &format!("ticket not found: {l1}"),
    );
    nirvana.run(&[&add[..], &["--connection", "linear"]].concat());

    let edit = ["edit", "--at", &at(d, "10:10"), "--ticket", &j2];
    nirvana.fail(
        &[&edit[..], &["--connection", "linear"]].concat(),
        &format!("ticket not found: {j2}"),
    );
    nirvana.run(&[&edit[..], &["--connection", "jira"]].concat());

    assert_eq!(
        nirvana.list_rows(d),
        [row(&j1, at(d, "09:00"), ""), row(&j2, at(d, "10:00"), "")]
    );
    let tickets = nirvana.run(&["ticket", "list", "--connection", "linear"]);
    assert!(tickets.contains(&l1), "{tickets}");
}
