pub use nirvana_entity::credential::SecretStore;

/// Where the token of a connection is stored. Tokens are never returned.
#[derive(Debug, PartialEq, Eq)]
pub enum SecretLocation {
    Plaintext,
    Keyring { account: String },
}

impl SecretLocation {
    pub fn store(&self) -> SecretStore {
        match self {
            Self::Plaintext => SecretStore::Plaintext,
            Self::Keyring { .. } => SecretStore::Keyring,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Credential {
    pub connection: String,
    pub location: SecretLocation,
}
