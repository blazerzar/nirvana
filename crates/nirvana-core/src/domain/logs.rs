use std::collections::VecDeque;
use std::io::{self, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use same_file::Handle;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, BufReader};

use crate::errors::NirvanaError;

const POLL_INTERVAL: Duration = Duration::from_millis(250);
const TAIL_CHUNK_SIZE: usize = 8 * 1024;

/// Reads lines from the log file, see [`Nirvana::open_logs`](crate::Nirvana::open_logs).
pub struct LogReader {
    path: PathBuf,
    file: Option<LogFile>,
    position: u64,
    last_line: Vec<u8>,
    partial_line: Vec<u8>,
}

struct LogFile {
    reader: BufReader<File>,
    handle: Handle,
}

impl LogReader {
    pub(crate) async fn open(path: PathBuf) -> io::Result<Self> {
        let file = open_existing(&path).await?;
        Ok(Self {
            path,
            file,
            position: 0,
            last_line: Vec::new(),
            partial_line: Vec::new(),
        })
    }

    /// Reads to the end of the file and returns the last `n` complete lines,
    /// or all of them when `n` is `None`. Following continues from there.
    ///
    /// # Errors
    ///
    /// Returns an error if reading the file fails.
    pub async fn tail(&mut self, n: Option<usize>) -> Result<Vec<String>, NirvanaError> {
        if let (Some(n), Some(file)) = (n, &mut self.file) {
            self.position = last_lines_start(file.reader.get_mut(), n).await?;
            file.reader.seek(SeekFrom::Start(self.position)).await?;
            self.partial_line.clear();
        }
        Ok(self.read_available(n).await?)
    }

    /// Waits until at least one new complete line is written and returns all
    /// new complete lines. Waits for a missing file to be created and starts
    /// from the beginning when the file is cleared, deleted or replaced.
    /// Dropping the future cancels it.
    ///
    /// # Errors
    ///
    /// Returns an error if opening or reading the file fails.
    pub async fn follow(&mut self) -> Result<Vec<String>, NirvanaError> {
        loop {
            if self.is_replaced()? {
                self.file = open_existing(&self.path).await?;
                self.restart();
            }
            self.rewind_if_rewritten().await?;
            let lines = self.read_available(None).await?;
            if !lines.is_empty() {
                return Ok(lines);
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    fn is_replaced(&self) -> io::Result<bool> {
        let current = match Handle::from_path(&self.path) {
            Ok(handle) => Some(handle),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        Ok(current.as_ref() != self.file.as_ref().map(|file| &file.handle))
    }

    /// Rewinds when the file is shorter than what was read or the last read
    /// line is no longer where it was read from.
    async fn rewind_if_rewritten(&mut self) -> io::Result<()> {
        let Some(file) = &mut self.file else {
            return Ok(());
        };
        let rewritten = file.reader.get_ref().metadata().await?.len() < self.position
            || !self.last_line.is_empty() && {
                let end = self.position - self.partial_line.len() as u64;
                let mut line = vec![0; self.last_line.len()];
                file.reader
                    .seek(SeekFrom::Start(end - line.len() as u64))
                    .await?;
                match file.reader.read_exact(&mut line).await {
                    Ok(_) => line != self.last_line,
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => true,
                    Err(e) => return Err(e),
                }
            };
        if rewritten {
            file.reader.seek(SeekFrom::Start(0)).await?;
            self.restart();
        } else {
            file.reader.seek(SeekFrom::Start(self.position)).await?;
        }
        Ok(())
    }

    fn restart(&mut self) {
        self.position = 0;
        self.last_line.clear();
        self.partial_line.clear();
    }

    /// Reads to the end of the file and returns the last `keep` complete
    /// lines. An incomplete last line is kept until its newline is written.
    async fn read_available(&mut self, keep: Option<usize>) -> io::Result<Vec<String>> {
        let Some(file) = &mut self.file else {
            return Ok(Vec::new());
        };
        let mut lines = VecDeque::new();
        loop {
            let read = file
                .reader
                .read_until(b'\n', &mut self.partial_line)
                .await?;
            if read == 0 {
                return Ok(lines.into());
            }
            self.position += read as u64;
            if self.partial_line.ends_with(b"\n") {
                self.last_line = std::mem::take(&mut self.partial_line);
                let line = String::from_utf8_lossy(&self.last_line);
                lines.push_back(line.trim_end_matches(['\r', '\n']).to_string());
                if keep.is_some_and(|n| lines.len() > n) {
                    lines.pop_front();
                }
            }
        }
    }
}

/// Returns the offset where the last `n` complete lines start, reading the
/// file backwards from its end.
async fn last_lines_start(file: &mut File, n: usize) -> io::Result<u64> {
    let mut end = file.metadata().await?.len();
    let mut chunk = vec![0; TAIL_CHUNK_SIZE];
    let mut newlines = 0;
    while end > 0 {
        let start = end.saturating_sub(TAIL_CHUNK_SIZE as u64);
        let chunk = &mut chunk[..(end - start) as usize];
        file.seek(SeekFrom::Start(start)).await?;
        file.read_exact(chunk).await?;
        for (i, byte) in chunk.iter().enumerate().rev() {
            if *byte == b'\n' {
                newlines += 1;
                if newlines > n {
                    return Ok(start + i as u64 + 1);
                }
            }
        }
        end = start;
    }
    Ok(0)
}

async fn open_existing(path: &Path) -> io::Result<Option<LogFile>> {
    match File::open(path).await {
        Ok(file) => {
            let handle = Handle::from_file(file.try_clone().await?.into_std().await)?;
            Ok(Some(LogFile {
                reader: BufReader::new(file),
                handle,
            }))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use tokio::time::timeout;

    use super::*;

    const WAIT: Duration = Duration::from_secs(2);

    fn log_path() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nirvana.log");
        (dir, path)
    }

    fn append(path: &Path, text: &str) {
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
    }

    #[tokio::test]
    async fn tail_returns_last_lines() {
        let (_dir, path) = log_path();
        append(&path, "one\ntwo\nthree\n");

        let mut reader = LogReader::open(path).await.unwrap();

        assert_eq!(reader.tail(Some(2)).await.unwrap(), ["two", "three"]);
    }

    #[tokio::test]
    async fn tail_skips_incomplete_last_line() {
        let (_dir, path) = log_path();
        append(&path, "one\ntwo\nthree");

        let mut reader = LogReader::open(path).await.unwrap();

        assert_eq!(reader.tail(Some(1)).await.unwrap(), ["two"]);
    }

    #[tokio::test]
    async fn tail_returns_more_lines_than_file_has() {
        let (_dir, path) = log_path();
        append(&path, "one\ntwo\n");

        let mut reader = LogReader::open(path).await.unwrap();

        assert_eq!(reader.tail(Some(5)).await.unwrap(), ["one", "two"]);
    }

    #[tokio::test]
    async fn follow_continues_after_tail_of_large_file() {
        let (_dir, path) = log_path();
        let lines: Vec<String> = (0..TAIL_CHUNK_SIZE).map(|i| format!("line {i}")).collect();
        append(&path, &(lines.join("\n") + "\n"));
        let mut reader = LogReader::open(path.clone()).await.unwrap();

        assert_eq!(
            reader.tail(Some(TAIL_CHUNK_SIZE / 2)).await.unwrap(),
            lines[TAIL_CHUNK_SIZE / 2..]
        );
        append(&path, "new\n");

        assert_eq!(
            timeout(WAIT, reader.follow()).await.unwrap().unwrap(),
            ["new"]
        );
    }

    #[tokio::test]
    async fn tail_without_limit_returns_all_lines() {
        let (_dir, path) = log_path();
        append(&path, "one\ntwo\nthree\n");

        let mut reader = LogReader::open(path).await.unwrap();

        assert_eq!(reader.tail(None).await.unwrap(), ["one", "two", "three"]);
    }

    #[tokio::test]
    async fn tail_of_missing_file_is_empty() {
        let (_dir, path) = log_path();

        let mut reader = LogReader::open(path).await.unwrap();

        assert!(reader.tail(None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn follow_returns_lines_appended_after_tail() {
        let (_dir, path) = log_path();
        append(&path, "old\n");
        let mut reader = LogReader::open(path.clone()).await.unwrap();
        reader.tail(None).await.unwrap();

        append(&path, "new\n");

        assert_eq!(
            timeout(WAIT, reader.follow()).await.unwrap().unwrap(),
            ["new"]
        );
    }

    #[tokio::test]
    async fn follow_waits_for_complete_line() {
        let (_dir, path) = log_path();
        append(&path, "partial");
        let mut reader = LogReader::open(path.clone()).await.unwrap();

        assert!(reader.tail(None).await.unwrap().is_empty());
        append(&path, " line\n");

        assert_eq!(
            timeout(WAIT, reader.follow()).await.unwrap().unwrap(),
            ["partial line"]
        );
    }

    #[tokio::test]
    async fn follow_restarts_after_truncate() {
        let (_dir, path) = log_path();
        append(&path, "first line\nsecond line\n");
        let mut reader = LogReader::open(path.clone()).await.unwrap();
        reader.tail(None).await.unwrap();

        fs::File::create(&path).unwrap();
        append(&path, "new\n");

        assert_eq!(
            timeout(WAIT, reader.follow()).await.unwrap().unwrap(),
            ["new"]
        );
    }

    #[tokio::test]
    async fn follow_restarts_after_truncate_and_longer_write() {
        let (_dir, path) = log_path();
        append(&path, "old\n");
        let mut reader = LogReader::open(path.clone()).await.unwrap();
        reader.tail(None).await.unwrap();

        fs::File::create(&path).unwrap();
        append(&path, "first line\nsecond line\n");

        assert_eq!(
            timeout(WAIT, reader.follow()).await.unwrap().unwrap(),
            ["first line", "second line"]
        );
    }

    #[tokio::test]
    async fn follow_reopens_replaced_file() {
        let (_dir, path) = log_path();
        append(&path, "old\n");
        let mut reader = LogReader::open(path.clone()).await.unwrap();
        reader.tail(None).await.unwrap();

        fs::remove_file(&path).unwrap();
        append(&path, "new\n");

        assert_eq!(
            timeout(WAIT, reader.follow()).await.unwrap().unwrap(),
            ["new"]
        );
    }

    #[tokio::test]
    async fn follow_waits_for_missing_file() {
        let (_dir, path) = log_path();
        let mut reader = LogReader::open(path.clone()).await.unwrap();
        reader.tail(None).await.unwrap();

        let writer = tokio::spawn(async move {
            tokio::time::sleep(POLL_INTERVAL).await;
            append(&path, "created\n");
        });

        assert_eq!(
            timeout(WAIT, reader.follow()).await.unwrap().unwrap(),
            ["created"]
        );
        writer.await.unwrap();
    }
}
