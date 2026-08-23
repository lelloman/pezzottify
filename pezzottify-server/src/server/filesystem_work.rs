use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use thiserror::Error;

use super::blocking_work::{BlockingWorkError, BoundedBlockingPool};

const DEFAULT_MAX_CONCURRENT: usize = 4;
const DEFAULT_QUEUE_TIMEOUT: Duration = Duration::from_secs(1);
const DEFAULT_EXECUTION_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub(super) struct FilesystemWorkPool {
    inner: BoundedBlockingPool,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error(transparent)]
pub(super) struct FilesystemWorkError(#[from] pub(super) BlockingWorkError);

impl Default for FilesystemWorkPool {
    fn default() -> Self {
        Self::with_limits(
            DEFAULT_MAX_CONCURRENT,
            DEFAULT_QUEUE_TIMEOUT,
            DEFAULT_EXECUTION_TIMEOUT,
        )
    }
}

impl FilesystemWorkPool {
    pub(super) fn with_limits(
        max_concurrent: usize,
        queue_timeout: Duration,
        execution_timeout: Duration,
    ) -> Self {
        Self {
            inner: BoundedBlockingPool::new(
                "filesystem",
                max_concurrent,
                queue_timeout,
                execution_timeout,
            ),
        }
    }

    pub(super) async fn run<T, F>(&self, work: F) -> Result<T, FilesystemWorkError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        self.inner.run(work).await.map_err(Into::into)
    }

    pub(super) async fn read(
        &self,
        path: PathBuf,
    ) -> Result<io::Result<Vec<u8>>, FilesystemWorkError> {
        self.run(move || std::fs::read(path)).await
    }

    pub(super) async fn write_atomic(
        &self,
        path: PathBuf,
        bytes: Vec<u8>,
    ) -> Result<io::Result<()>, FilesystemWorkError> {
        self.run(move || atomic_write(&path, &bytes)).await
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no parent"))?;
    std::fs::create_dir_all(parent)?;

    let mut temporary_name = OsString::from(".");
    temporary_name.push(
        path.file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?,
    );
    temporary_name.push(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let temporary_path = parent.join(temporary_name);

    let result = (|| {
        use std::io::Write;

        let mut temporary = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary_path)?;
        temporary.write_all(bytes)?;
        temporary.flush()?;
        std::fs::rename(&temporary_path, path)
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temporary_path);
    }
    result
}
