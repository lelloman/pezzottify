//! Catalog-independent, root-confined local media access.
use anyhow::{Context, Result};
#[cfg(unix)]
use std::ffi::CString;
use std::fs::File;
use std::path::{Path, PathBuf};

/// Validate the catalog representation of a media path. Catalog paths are URI-like,
/// relative identifiers; accepting platform-specific separators would make a catalog
/// safe on one host and unsafe after moving it to another.
pub(crate) fn normalized_media_identifier(identifier: &str) -> Result<PathBuf> {
    if identifier.is_empty()
        || identifier.starts_with('/')
        || identifier.contains('\\')
        || identifier.contains('\0')
    {
        anyhow::bail!("media identifier must be a non-empty relative path");
    }

    let mut normalized = PathBuf::new();
    for component in identifier.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            anyhow::bail!("media identifier contains a non-normal component");
        }
        normalized.push(component);
    }

    if normalized.is_absolute()
        || normalized
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        anyhow::bail!("media identifier is not a normalized relative path");
    }
    Ok(normalized)
}

#[cfg(any(not(unix), test))]
pub(crate) fn resolve_existing_media_path(media_root: &Path, identifier: &str) -> Result<PathBuf> {
    let relative = normalized_media_identifier(identifier)?;
    let canonical_root = media_root
        .canonicalize()
        .with_context(|| format!("failed to resolve media root {}", media_root.display()))?;
    let canonical_path = media_root
        .join(relative)
        .canonicalize()
        .with_context(|| format!("failed to resolve media identifier {identifier:?}"))?;
    if !canonical_path.starts_with(&canonical_root) {
        anyhow::bail!("media identifier resolves outside the configured media root");
    }
    if !canonical_path.is_file() {
        anyhow::bail!("media identifier does not resolve to a regular file");
    }
    Ok(canonical_path)
}

#[cfg(unix)]
pub(crate) fn open_media_file_beneath(
    media_root: &Path,
    identifier: &str,
) -> Result<(File, PathBuf)> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;

    let relative = normalized_media_identifier(identifier)?;
    let components = relative.components().collect::<Vec<_>>();
    let mut directory = File::open(media_root)
        .with_context(|| format!("failed to open media root {}", media_root.display()))?;

    for (index, component) in components.iter().enumerate() {
        let std::path::Component::Normal(name) = component else {
            anyhow::bail!("media identifier contains a non-normal component");
        };
        let name = CString::new(name.as_bytes()).context("media identifier contains NUL")?;
        let is_last = index + 1 == components.len();
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | if is_last { 0 } else { libc::O_DIRECTORY };
        // SAFETY: directory is a live directory fd, name is NUL-terminated, and a
        // successful fd is immediately owned by File. O_NOFOLLOW is applied at every level.
        let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error())
                .with_context(|| format!("failed to safely open media identifier {identifier:?}"));
        }
        // SAFETY: openat returned a new owned descriptor on success.
        let opened = unsafe { File::from_raw_fd(fd) };
        if is_last {
            if !opened.metadata()?.is_file() {
                anyhow::bail!("media identifier does not resolve to a regular file");
            }
            return Ok((opened, media_root.join(relative)));
        }
        directory = opened;
    }

    anyhow::bail!("media identifier must not be empty")
}

#[cfg(not(unix))]
pub(crate) fn open_media_file_beneath(
    media_root: &Path,
    identifier: &str,
) -> Result<(File, PathBuf)> {
    let path = resolve_existing_media_path(media_root, identifier)?;
    Ok((File::open(&path)?, path))
}
