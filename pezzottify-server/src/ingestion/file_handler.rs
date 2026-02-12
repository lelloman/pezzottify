//! File handling for ingestion uploads.

use super::models::UploadType;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Errors that can occur during file handling.
#[derive(Debug, Error)]
pub enum FileHandlerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid filename: {0}")]
    InvalidFilename(String),

    #[error("Unsupported file type: {0}")]
    UnsupportedFileType(String),

    #[error("File too large: {0} bytes (max: {1})")]
    FileTooLarge(u64, u64),

    #[error("Zip extraction error: {0}")]
    ZipError(String),
}

/// Supported audio file extensions.
const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "ogg", "m4a", "aac", "wma", "opus"];

/// File handler for managing uploaded files.
pub struct FileHandler {
    /// Base directory for temporary files.
    temp_dir: PathBuf,
    /// Maximum file size in bytes.
    max_file_size: u64,
}

impl FileHandler {
    /// Create a new file handler.
    pub fn new(temp_dir: impl Into<PathBuf>, max_file_size: u64) -> Self {
        Self {
            temp_dir: temp_dir.into(),
            max_file_size,
        }
    }

    /// Get the temp directory path.
    pub fn temp_dir(&self) -> &Path {
        &self.temp_dir
    }

    /// Initialize the file handler (creates temp directory).
    pub async fn init(&self) -> Result<(), FileHandlerError> {
        fs::create_dir_all(&self.temp_dir).await?;
        Ok(())
    }

    /// Create a job-specific temp directory.
    pub async fn create_job_dir(&self, job_id: &str) -> Result<PathBuf, FileHandlerError> {
        let job_dir = self.temp_dir.join(job_id);
        fs::create_dir_all(&job_dir).await?;
        Ok(job_dir)
    }

    /// Save uploaded bytes to a file in the job directory.
    pub async fn save_upload(
        &self,
        job_id: &str,
        filename: &str,
        data: &[u8],
    ) -> Result<PathBuf, FileHandlerError> {
        // Validate file size
        let size = data.len() as u64;
        if size > self.max_file_size {
            return Err(FileHandlerError::FileTooLarge(size, self.max_file_size));
        }

        // Sanitize filename
        let safe_filename = sanitize_filename(filename)?;

        // Create job directory
        let job_dir = self.create_job_dir(job_id).await?;

        // Write file
        let file_path = job_dir.join(&safe_filename);
        let mut file = fs::File::create(&file_path).await?;
        file.write_all(data).await?;
        file.flush().await?;

        Ok(file_path)
    }

    /// Check if a file is a supported audio format.
    pub fn is_supported_audio(filename: &str) -> bool {
        let ext = Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        ext.map(|e| SUPPORTED_EXTENSIONS.contains(&e.as_str()))
            .unwrap_or(false)
    }

    /// Check if a file is a zip archive.
    pub fn is_zip(filename: &str) -> bool {
        let ext = Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        ext.map(|e| e == "zip").unwrap_or(false)
    }

    /// Extract a zip file and return paths to audio files.
    /// Preserves directory structure from the zip for collection detection.
    pub async fn extract_zip(
        &self,
        job_id: &str,
        zip_path: &Path,
    ) -> Result<Vec<PathBuf>, FileHandlerError> {
        let job_dir = self.create_job_dir(job_id).await?;
        let extract_dir = job_dir.join("extracted");
        fs::create_dir_all(&extract_dir).await?;

        // Read the zip file
        let zip_data = fs::read(zip_path).await?;
        let cursor = std::io::Cursor::new(zip_data);

        // Extract using zip crate (sync operation)
        let mut archive =
            zip::ZipArchive::new(cursor).map_err(|e| FileHandlerError::ZipError(e.to_string()))?;

        let mut audio_files = Vec::new();

        for i in 0..archive.len() {
            // Extract data from archive (sync operation) before any await points
            let (is_dir, filename, content) = {
                let mut file = archive
                    .by_index(i)
                    .map_err(|e| FileHandlerError::ZipError(e.to_string()))?;

                let is_dir = file.is_dir();
                let filename = file.name().to_string();

                // Read content while holding the ZipFile
                let mut content = Vec::new();
                if !is_dir {
                    std::io::Read::read_to_end(&mut file, &mut content)
                        .map_err(|e| FileHandlerError::ZipError(e.to_string()))?;
                }
                // ZipFile is dropped here before the await
                (is_dir, filename, content)
            };

            // Skip directories and non-audio files
            if is_dir {
                continue;
            }

            if !Self::is_supported_audio(&filename) {
                continue;
            }

            // Sanitize and preserve the path structure from the zip
            let safe_path = sanitize_zip_path(&filename)?;
            let output_path = extract_dir.join(&safe_path);

            // Create parent directories if needed
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).await?;
            }

            // Write to output (ZipFile already dropped, so this await is safe)
            fs::write(&output_path, &content).await?;
            audio_files.push(output_path);
        }

        Ok(audio_files)
    }

    /// Clean up job directory.
    pub async fn cleanup_job(&self, job_id: &str) -> Result<(), FileHandlerError> {
        let job_dir = self.temp_dir.join(job_id);
        if job_dir.exists() {
            fs::remove_dir_all(&job_dir).await?;
        }
        Ok(())
    }

    /// List audio files in a directory.
    pub async fn list_audio_files(&self, dir: &Path) -> Result<Vec<PathBuf>, FileHandlerError> {
        let mut audio_files = Vec::new();
        let mut entries = fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if Self::is_supported_audio(name) {
                        audio_files.push(path);
                    }
                }
            }
        }

        Ok(audio_files)
    }

    /// Detect upload type from extracted directory structure.
    ///
    /// - Single audio file → Track
    /// - Multiple audio files in root or single subdirectory → Album
    /// - Multiple directories containing audio files → Collection
    pub async fn detect_upload_type(
        &self,
        extracted_dir: &Path,
    ) -> Result<UploadType, FileHandlerError> {
        // Count audio files directly in the root
        let root_audio_files = self.list_audio_files(extracted_dir).await?;

        // Find subdirectories containing audio files
        let mut dirs_with_audio: HashSet<PathBuf> = HashSet::new();
        let mut total_audio_count = root_audio_files.len();

        let mut entries = fs::read_dir(extracted_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                let sub_audio = self.list_audio_files_recursive(&path).await?;
                if !sub_audio.is_empty() {
                    dirs_with_audio.insert(path);
                    total_audio_count += sub_audio.len();
                }
            }
        }

        // Determine upload type based on structure
        if total_audio_count == 1 {
            Ok(UploadType::Track)
        } else if dirs_with_audio.len() > 1 {
            // Multiple directories with audio → Collection
            Ok(UploadType::Collection)
        } else {
            // All files in root or single subdirectory → Album
            Ok(UploadType::Album)
        }
    }

    /// List audio files recursively in a directory.
    pub async fn list_audio_files_recursive(
        &self,
        dir: &Path,
    ) -> Result<Vec<PathBuf>, FileHandlerError> {
        let mut audio_files = Vec::new();
        let mut stack = vec![dir.to_path_buf()];

        while let Some(current_dir) = stack.pop() {
            let mut entries = fs::read_dir(&current_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.is_file() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if Self::is_supported_audio(name) {
                            audio_files.push(path);
                        }
                    }
                }
            }
        }

        Ok(audio_files)
    }

    /// Group audio files by their parent directory (for collection uploads).
    /// Returns a map of directory path to audio files in that directory.
    pub async fn group_files_by_album(
        &self,
        extracted_dir: &Path,
    ) -> Result<Vec<(PathBuf, Vec<PathBuf>)>, FileHandlerError> {
        let mut albums: Vec<(PathBuf, Vec<PathBuf>)> = Vec::new();

        // Check root directory for audio files
        let root_audio = self.list_audio_files(extracted_dir).await?;
        if !root_audio.is_empty() {
            albums.push((extracted_dir.to_path_buf(), root_audio));
        }

        // Check subdirectories
        let mut entries = fs::read_dir(extracted_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                let sub_audio = self.list_audio_files_recursive(&path).await?;
                if !sub_audio.is_empty() {
                    albums.push((path, sub_audio));
                }
            }
        }

        Ok(albums)
    }

    /// Get the output path for a converted track.
    /// Uses sharded directory structure: audio/{first2}/{next2}/{track_id}.ogg
    pub fn get_output_path(&self, media_path: &Path, track_id: &str) -> PathBuf {
        let (dir1, dir2) = Self::shard_dirs(track_id);
        media_path
            .join("audio")
            .join(dir1)
            .join(dir2)
            .join(format!("{}.ogg", track_id))
    }

    /// Compute shard directory components from track ID.
    /// Returns (first 2 chars, chars 2-4) for directory structure.
    pub fn shard_dirs(track_id: &str) -> (&str, &str) {
        let bytes = track_id.as_bytes();
        let dir1 = if bytes.len() >= 2 {
            &track_id[0..2]
        } else {
            "00"
        };
        let dir2 = if bytes.len() >= 4 {
            &track_id[2..4]
        } else {
            "00"
        };
        (dir1, dir2)
    }
}

/// Sanitize a filename to prevent path traversal attacks.
fn sanitize_filename(filename: &str) -> Result<String, FileHandlerError> {
    // Get just the filename part (no path)
    let name = Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| FileHandlerError::InvalidFilename(filename.to_string()))?;

    // Check for suspicious patterns:
    // - Null bytes are never allowed
    // - Hidden files (starting with .) are not allowed
    // - Exact ".." is path traversal (but "..." as ellipsis in a name is fine)
    if name.contains('\0') || name.starts_with('.') || name == ".." {
        return Err(FileHandlerError::InvalidFilename(filename.to_string()));
    }

    // Replace problematic characters (keep Unicode letters/symbols)
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();

    if sanitized.is_empty() {
        return Err(FileHandlerError::InvalidFilename(filename.to_string()));
    }

    Ok(sanitized)
}

/// Sanitize a zip file path while preserving directory structure.
/// Prevents path traversal attacks while keeping album subdirectories intact.
fn sanitize_zip_path(zip_path: &str) -> Result<PathBuf, FileHandlerError> {
    let mut result = PathBuf::new();

    // Normalize path separators and split into components
    let normalized = zip_path.replace('\\', "/");

    for component in normalized.split('/') {
        // Skip empty components (from leading/trailing/double slashes)
        if component.is_empty() {
            continue;
        }

        // Reject path traversal attempts
        if component == ".." {
            return Err(FileHandlerError::InvalidFilename(zip_path.to_string()));
        }

        // Skip hidden files/directories (starting with .)
        if component.starts_with('.') {
            continue;
        }

        // Check for null bytes
        if component.contains('\0') {
            return Err(FileHandlerError::InvalidFilename(zip_path.to_string()));
        }

        // Sanitize problematic characters in this component
        let sanitized: String = component
            .chars()
            .map(|c| match c {
                ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                _ => c,
            })
            .collect();

        if !sanitized.is_empty() {
            result.push(sanitized);
        }
    }

    if result.as_os_str().is_empty() {
        return Err(FileHandlerError::InvalidFilename(zip_path.to_string()));
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported_audio() {
        assert!(FileHandler::is_supported_audio("track.mp3"));
        assert!(FileHandler::is_supported_audio("track.MP3"));
        assert!(FileHandler::is_supported_audio("track.flac"));
        assert!(FileHandler::is_supported_audio("track.FLAC"));
        assert!(FileHandler::is_supported_audio("track.wav"));
        assert!(FileHandler::is_supported_audio("track.ogg"));
        assert!(FileHandler::is_supported_audio("track.m4a"));
        assert!(!FileHandler::is_supported_audio("track.txt"));
        assert!(!FileHandler::is_supported_audio("track.exe"));
        assert!(!FileHandler::is_supported_audio("track"));
    }

    #[test]
    fn test_is_zip() {
        assert!(FileHandler::is_zip("album.zip"));
        assert!(FileHandler::is_zip("album.ZIP"));
        assert!(!FileHandler::is_zip("track.mp3"));
        assert!(!FileHandler::is_zip("file"));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("track.mp3").unwrap(), "track.mp3");
        // Path components are stripped, leaving just the filename
        assert_eq!(
            sanitize_filename("/path/to/track.mp3").unwrap(),
            "track.mp3"
        );
        // Path traversal is stripped, leaving just the filename
        assert_eq!(sanitize_filename("../track.mp3").unwrap(), "track.mp3");
        assert_eq!(
            sanitize_filename("track:name.mp3").unwrap(),
            "track_name.mp3"
        );

        // Hidden files (starting with .) should fail
        assert!(sanitize_filename(".hidden").is_err());
        // Pure path traversal with no filename should fail
        assert!(sanitize_filename("..").is_err());
    }
}
