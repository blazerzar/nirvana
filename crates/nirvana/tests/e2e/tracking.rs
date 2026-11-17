use crate::helpers::{Config, Jira, Linear, Nirvana, at, day, row, title};

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
