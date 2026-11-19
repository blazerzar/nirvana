use crate::helpers::{Config, Nirvana, line_with};

#[tokio::test]
#[ignore]
async fn manages_profiles() {
    let config = Config::from_env();
    let nirvana = Nirvana::new();

    nirvana.fail(&["status"], "no active profile");
    let commands: [&[&str]; 4] = [
        &["profile", "list"],
        &["profile", "switch"],
        &["profile", "delete"],
        &["profile", "rename"],
    ];
    for command in commands {
        let output = nirvana.run(command);
        assert!(output.contains("No profiles found"), "{output}");
    }
    nirvana.setup();
    let status = nirvana.run(&["status"]);
    assert!(status.contains("Not tracking"), "{status}");
    nirvana.fail(
        &["profile", "create", "e2e"],
        "profile 'e2e' already exists",
    );
    let switched = nirvana.run(&["profile", "switch", "e2e"]);
    assert!(
        switched.contains("Profile e2e is already active"),
        "{switched}"
    );
    nirvana.fail(
        &["profile", "delete", "nope", "-y"],
        "profile 'nope' not found",
    );

    nirvana.run(&["profile", "create", "other"]);
    let profiles = nirvana.run(&["profile", "list"]);
    assert!(line_with(&profiles, "e2e").contains('*'), "{profiles}");
    assert!(!line_with(&profiles, "other").contains('*'), "{profiles}");

    nirvana.add_jira(&config);
    let connections = nirvana.run(&["connection", "list"]);
    assert!(
        line_with(&connections, "jira-cloud").starts_with("jira"),
        "{connections}"
    );

    nirvana.run(&["profile", "switch", "other"]);
    let profiles = nirvana.run(&["profile", "list"]);
    assert!(!line_with(&profiles, "e2e").contains('*'), "{profiles}");
    assert!(line_with(&profiles, "other").contains('*'), "{profiles}");
    let connections = nirvana.run(&["connection", "list"]);
    assert!(
        connections.contains("No connections found"),
        "{connections}"
    );

    nirvana.run(&["profile", "rename", "other", "renamed"]);
    let profiles = nirvana.run(&["profile", "list"]);
    assert!(line_with(&profiles, "renamed").contains('*'), "{profiles}");
    assert!(!profiles.contains("other"), "{profiles}");

    let deleted = nirvana.run(&["profile", "delete", "renamed", "-y"]);
    assert!(deleted.contains("Switched"), "{deleted}");
    let profiles = nirvana.run(&["profile", "list"]);
    assert!(line_with(&profiles, "e2e").contains('*'), "{profiles}");
    assert!(!profiles.contains("renamed"), "{profiles}");
}
