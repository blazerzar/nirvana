/// A config key with its value in `config.toml` and its default.
#[derive(Debug, PartialEq)]
pub struct ConfigEntry {
    /// `section.key`
    pub key: &'static str,
    /// `None` if the key is not set.
    pub value: Option<String>,
    /// `None` if the key has no default.
    pub default: Option<String>,
}
