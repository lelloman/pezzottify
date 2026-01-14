//! File handling for ingestion uploads.

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

            // Sanitize the filename from the zip
            let safe_name = sanitize_filename(&filename)?;
            let output_path = extract_dir.join(&safe_name);

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

    /// Get the output path for a converted track.
    pub fn get_output_path(&self, media_path: &Path, track_id: &str) -> PathBuf {
        // Media files are stored as: {media_path}/tracks/{track_id}.ogg
        media_path.join("tracks").join(format!("{}.ogg", track_id))
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
