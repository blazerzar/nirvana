use crate::helpers::Nirvana;

#[tokio::test]
#[ignore]
async fn manages_logs() {
    let nirvana = Nirvana::new();
    nirvana.setup();

    let logs = nirvana.run(&["logs", "view"]);
    assert!(logs.contains("profile"), "{logs}");
    let last = nirvana.run(&["logs", "view", "-n", "1"]);
    assert_eq!(last.lines().count(), 1, "{last}");

    nirvana.run(&["logs", "clear", "-y"]);
    let logs = nirvana.run(&["logs", "view"]);
    assert!(!logs.contains("profile created"), "{logs}");
}
