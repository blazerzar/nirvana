use std::env;

use crate::helpers::Nirvana;

#[tokio::test]
#[ignore]
async fn shows_info() {
    let nirvana = Nirvana::new();

    let info = nirvana.run(&["info"]);
    for text in [
        "Version",
        "SQLite",
        "Keyring",
        "config.toml",
        "nirvana.db",
        "nirvana.log",
        env::consts::OS,
    ] {
        assert!(info.contains(text), "{info}");
    }
    assert!(!info.contains("-dev"), "{info}");
}
