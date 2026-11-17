use crate::helpers::{
    Config, Jira, Linear, Nirvana, at, day, first_column, line_with, shown, sorted, title,
};

#[tokio::test]
#[ignore]
async fn syncs_ticket_titles() {
    const TEST: &str = "syncs_ticket_titles";
    let config = Config::from_env();
    let (jira, linear) = (Jira::new(&config), Linear::new(&config));
    let d = day(1);

    let j1 = jira.create_issue(&title(TEST, "J1")).await;
    let j2 = jira.create_issue(&title(TEST, "J2")).await;
    let l1 = linear.create_issue(&title(TEST, "L1")).await;

    let nirvana = Nirvana::connected(&config, linear.workspace().await);
    nirvana.run(&["ticket", "load", &j1]);
    nirvana.run(&["add", &l1, "--start", &at(d, "09:00"), "--duration", "30m"]);

    let (j1_renamed, l1_renamed) = (title(TEST, "J1 renamed"), title(TEST, "L1 renamed"));
    jira.rename_issue(&j1, &j1_renamed).await;
    linear.rename_issue(&l1, &l1_renamed).await;
    let tickets = nirvana.run(&["ticket", "list"]);
    for name in ["J1", "L1"] {
        assert!(tickets.contains(&shown(&title(TEST, name))), "{tickets}");
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
        assert!(!tickets.contains(&shown(&title(TEST, old))), "{tickets}");
    }
    let slots = nirvana.run(&["list", "--from", &at(d, "00:00"), "--to", &at(d, "23:59")]);
    assert!(slots.contains(&shown(&l1_renamed)), "{slots}");
}

#[tokio::test]
#[ignore]
async fn loads_and_lists_tickets() {
    const TEST: &str = "loads_and_lists_tickets";
    let config = Config::from_env();
    let (jira, linear) = (Jira::new(&config), Linear::new(&config));

    let j1 = jira.create_issue(&title(TEST, "J1")).await;
    let j2 = jira.create_issue(&title(TEST, "J2")).await;
    let l1 = linear.create_issue(&title(TEST, "L1")).await;

    let nirvana = Nirvana::connected(&config, linear.workspace().await);
    let loaded = nirvana.run(&["ticket", "load", &j1]);
    assert!(loaded.contains(&title(TEST, "J1")), "{loaded}");
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
        assert!(tickets.contains(&shown(&title(TEST, name))), "{tickets}");
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
