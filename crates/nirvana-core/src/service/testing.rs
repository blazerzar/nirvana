use std::path::Path;

use nirvana_entity::ticket;

use crate::errors::NirvanaError;
use crate::integration::IssueInfo;
use crate::paths::AppPaths;
use crate::secrets::mock_store;
use crate::service::Nirvana;

impl Nirvana {
    /// Opens the config and database in `dir`, with an in-memory keyring.
    pub async fn open_in(dir: &Path) -> Result<Self, NirvanaError> {
        Self::open(AppPaths::in_dir(dir), mock_store()).await
    }

    /// Stores the ticket `key` of the connection named `connection` with
    /// `title`, without fetching it from the connection.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable,
    /// the connection does not exist or a database query fails.
    pub async fn store_test_ticket(
        &self,
        connection: &str,
        key: &str,
        title: &str,
    ) -> Result<ticket::Model, NirvanaError> {
        let connection = self.get_connection(connection).await?;
        let issue = IssueInfo {
            key: key.to_string(),
            title: title.to_string(),
        };
        self.store_ticket(&connection, issue).await
    }
}
