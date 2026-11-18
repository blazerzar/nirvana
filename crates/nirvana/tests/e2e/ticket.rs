use crate::helpers::{E2e, at, day, first_column, line_with, shown, sorted};

#[tokio::test]
#[ignore]
async fn syncs_ticket_titles() {
    let e2e = E2e::connected("syncs_ticket_titles");
    let (jira, linear, nirvana) = (&e2e.jira, &e2e.linear, &e2e.nirvana);
    let d = day(1);

    let j1 = e2e.jira_issue("J1").await;
    let j2 = e2e.jira_issue("J2").await;
    let l1 = e2e.linear_issue("L1").await;

    nirvana.run(&["ticket", "load", &j1]);
    nirvana.run(&["add", &l1, "--start", &at(d, "09:00"), "--duration", "30m"]);

    let (j1_renamed, l1_renamed) = (e2e.title("J1 renamed"), e2e.title("L1 renamed"));
    jira.rename_issue(&j1, &j1_renamed).await;
    linear.rename_issue(&l1, &l1_renamed).await;
    let tickets = nirvana.run(&["ticket", "list"]);
    for name in ["J1", "L1"] {
        assert!(tickets.contains(&shown(&e2e.title(name))), "{tickets}");
    }

    let synced = nirvana.run(&["ticket", "sync", &j1]);
    assert!(synced.contains(&j1_renamed), "{synced}");
    let synced = nirvana.run(&["ticket", "sync", &l1, "--connection", "linear"]);
    assert!(synced.contains(&l1_renamed), "{synced}");
    nirvana.fail(
        &["ticket", "sync", &j1, "--connection", "linear"],
        format!("ticket not found: {j1}").as_str(),
    );
    nirvana.fail(
        &["ticket", "sync", &j2],
        format!("ticket not found: {j2}").as_str(),
    );

    let tickets = nirvana.run(&["ticket", "list"]);
    for (old, new) in [("J1", &j1_renamed), ("L1", &l1_renamed)] {
        assert!(tickets.contains(&shown(new)), "{tickets}");
        assert!(!tickets.contains(&shown(&e2e.title(old))), "{tickets}");
    }
    let slots = nirvana.run(&["list", "--from", &at(d, "00:00"), "--to", &at(d, "23:59")]);
    assert!(slots.contains(&shown(&l1_renamed)), "{slots}");
}

#[tokio::test]
#[ignore]
async fn loads_and_lists_tickets() {
    let e2e = E2e::connected("loads_and_lists_tickets");
    let (config, nirvana) = (&e2e.config, &e2e.nirvana);

    let j1 = e2e.jira_issue("J1").await;
    let j2 = e2e.jira_issue("J2").await;
    let l1 = e2e.linear_issue("L1").await;

    let loaded = nirvana.run(&["ticket", "load", &j1]);
    assert!(loaded.contains(&e2e.title("J1")), "{loaded}");
    nirvana.run(&["ticket", "load", &l1, "--connection", "linear"]);
    nirvana.fail(
        &["ticket", "load", &j2, "--connection", "linear"],
        format!("ticket not found: {j2}").as_str(),
    );
    let missing = format!("{}-999999", config.jira_project);
    let missing_not_found = format!("ticket not found: {missing}");
    nirvana.fail(&["ticket", "load", &missing], &missing_not_found);
    nirvana.run(&["ticket", "load", &j2, "--connection", "jira"]);

    let tickets = nirvana.run(&["ticket", "list"]);
    for name in ["J1", "J2", "L1"] {
        assert!(tickets.contains(&shown(&e2e.title(name))), "{tickets}");
    }
    let jira_tickets = nirvana.run(&["ticket", "list", "--connection", "jira"]);
    assert_eq!(
        sorted(first_column(&jira_tickets)),
        sorted([j1.clone(), j2.clone()]),
        "{jira_tickets}"
    );
    let linear_tickets = nirvana.run(&["ticket", "list", "--connection", "linear"]);
    assert_eq!(
        first_column(&linear_tickets),
        [l1.as_str()],
        "{linear_tickets}"
    );
    let tickets = nirvana.run(&["ticket", "list", "--no-connection"]);
    assert!(tickets.contains("No tickets found"), "{tickets}");

    let by_key = first_column(&nirvana.run(&["ticket", "list", "--sort", "key"]));
    assert_eq!(by_key, sorted([j1.clone(), j2.clone(), l1.clone()]));
    let by_title = first_column(&nirvana.run(&["ticket", "list", "--sort", "title"]));
    assert_eq!(by_title, [j1.clone(), j2.clone(), l1.clone()]);

    let with_ids = nirvana.run(&["ticket", "list", "--show-ids"]);
    let id = line_with(&with_ids, &l1).split_whitespace().next().unwrap();
    assert!(id.parse::<i64>().is_ok(), "{with_ids}");
}

#[tokio::test]
#[ignore]
async fn deletes_tickets() {
    let e2e = E2e::connected("deletes_tickets");
    let nirvana = &e2e.nirvana;
    let d = day(1);

    let j1 = e2e.jira_issue("J1").await;
    let j2 = e2e.jira_issue("J2").await;
    let l1 = e2e.linear_issue("L1").await;

    nirvana.run(&["ticket", "load", &j1]);
    nirvana.run(&["ticket", "load", &l1]);
    nirvana.run(&["add", &j2, "--start", &at(d, "09:00"), "--duration", "30m"]);

    nirvana.run(&["ticket", "delete", &j1]);
    let has_slots = format!("ticket {j2} has 1 slots");
    nirvana.fail(&["ticket", "delete", &j2], &has_slots);
    assert_eq!(nirvana.list_rows(d).len(), 1);
    nirvana.run(&["ticket", "delete", &j2, "--with-slots"]);
    assert!(nirvana.list_rows(d).is_empty());

    let with_ids = nirvana.run(&["ticket", "list", "--show-ids"]);
    let id = line_with(&with_ids, &l1).split_whitespace().next().unwrap();
    nirvana.run(&["ticket", "delete", "--id", id]);
    let tickets = nirvana.run(&["ticket", "list"]);
    assert!(tickets.contains("No tickets found"), "{tickets}");
}
