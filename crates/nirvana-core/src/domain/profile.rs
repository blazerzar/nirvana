pub use nirvana_entity::profile::Model as Profile;

use super::Connection;

#[derive(Debug)]
pub struct ActiveProfile {
    pub profile: Profile,
    pub connections: Vec<Connection>,
}
