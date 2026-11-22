use crate::helpers::Nirvana;

#[tokio::test]
#[ignore]
async fn manages_config() {
    let nirvana = Nirvana::new();
    let config = nirvana.run(&["config", "get"]);
    assert!(config.contains("No config values set"), "{config}");
    nirvana.setup();

    let config = nirvana.run(&["config", "get"]);
    assert!(config.contains("core.profile"), "{config}");
    assert_eq!(
        nirvana.run(&["config", "get", "core.profile"]).trim(),
        "e2e"
    );

    nirvana.run(&["config", "set", "log.level", "debug"]);
    assert_eq!(nirvana.run(&["config", "get", "log.level"]).trim(), "debug");
    let default = nirvana.run(&["config", "get", "log.level", "--show-default"]);
    assert_eq!(default.trim(), "info");
    let default = nirvana.run(&["config", "get", "core.port", "--show-default"]);
    assert_eq!(default.trim(), "5167");
    nirvana.run(&["config", "set", "core.port", "8080"]);
    assert_eq!(nirvana.run(&["config", "get", "core.port"]).trim(), "8080");
    nirvana.fail(
        &["config", "set", "core.port", "70000"],
        "invalid value for config key 'core.port'",
    );
    nirvana.run(&["config", "set", "gui.server", "false"]);
    assert_eq!(
        nirvana.run(&["config", "get", "gui.server"]).trim(),
        "false"
    );
    nirvana.fail(
        &["config", "set", "gui.theme", "blue"],
        "invalid value for config key 'gui.theme'",
    );
    nirvana.fail(
        &["config", "set", "foo.bar", "1"],
        "unknown config key 'foo.bar'",
    );

    nirvana.run(&["config", "unset", "log.level"]);
    assert_eq!(nirvana.run(&["config", "get", "log.level"]).trim(), "");
    let unset = nirvana.run(&["config", "get", "--show-unset"]);
    assert!(unset.contains("log.level"), "{unset}");
}
