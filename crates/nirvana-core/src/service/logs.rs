//! Log file viewing, following and clearing.

use std::io;
use std::path::Path;

use tokio::fs::OpenOptions;
use tracing::debug;

use crate::domain::LogReader;
use crate::errors::NirvanaError;
use crate::paths::AppPaths;
use crate::service::Nirvana;

impl Nirvana {
    /// Opens the log file for reading from its first line. A missing file
    /// reads as empty until it is created.
    ///
    /// # Errors
    ///
    /// Returns an error if the log file location cannot be determined or the
    /// file exists but cannot be opened.
    pub async fn open_logs() -> Result<LogReader, NirvanaError> {
        let paths = AppPaths::resolve()?;
        Ok(LogReader::open(paths.log_file).await?)
    }

    /// Truncates the log file. A missing file is skipped.
    ///
    /// The file is truncated, not deleted, because running writers keep it
    /// open in append mode and continue writing at its new end.
    ///
    /// # Errors
    ///
    /// Returns an error if the log file location cannot be determined or the
    /// file exists but cannot be truncated.
    pub async fn clear_logs() -> Result<(), NirvanaError> {
        let paths = AppPaths::resolve()?;
        Ok(clear(&paths.log_file).await?)
    }
}

async fn clear(path: &Path) -> io::Result<()> {
    match OpenOptions::new().write(true).open(path).await {
        Ok(file) => {
            file.set_len(0).await?;
            debug!(path = %path.display(), "log file cleared");
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[tokio::test]
    async fn clear_truncates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nirvana.log");
        fs::write(&path, "first\nsecond\nthird\n").unwrap();

        clear(&path).await.unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "");
    }

    #[tokio::test]
    async fn clear_skips_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nirvana.log");

        clear(&path).await.unwrap();

        assert!(!path.exists());
    }
}
