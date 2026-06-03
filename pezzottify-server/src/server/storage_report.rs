use serde::Serialize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use super::ServerConfig;

#[derive(Debug, Clone, Serialize)]
pub struct StorageReport {
    pub total_bytes: u64,
    pub database_total_bytes: u64,
    pub filesystem_total_bytes: u64,
    pub databases: Vec<DatabaseStorage>,
    pub components: Vec<StorageComponent>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatabaseStorage {
    pub id: String,
    pub label: String,
    pub path: String,
    pub main_bytes: u64,
    pub wal_bytes: u64,
    pub shm_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StorageComponent {
    pub id: String,
    pub label: String,
    pub path: String,
    pub bytes: u64,
}

pub fn collect_storage_report(config: &ServerConfig, mut db_paths: Vec<PathBuf>) -> StorageReport {
    db_paths.sort();
    db_paths.dedup();

    let databases: Vec<DatabaseStorage> =
        db_paths.iter().map(|path| database_storage(path)).collect();
    let database_total_bytes = databases.iter().map(|db| db.total_bytes).sum();

    let components = vec![
        directory_component(
            "catalog_audio",
            "Catalog audio",
            &config.media_path.join("audio"),
        ),
        directory_component(
            "catalog_images",
            "Catalog images",
            &config.media_path.join("images"),
        ),
        directory_component("shows_media", "Shows media", &config.shows_media_dir()),
        directory_component(
            "ingestion_uploads",
            "Ingestion uploads",
            &config.ingestion_temp_dir(),
        ),
    ];
    let filesystem_total_bytes = components.iter().map(|component| component.bytes).sum();

    StorageReport {
        total_bytes: database_total_bytes + filesystem_total_bytes,
        database_total_bytes,
        filesystem_total_bytes,
        databases,
        components,
    }
}

fn database_storage(path: &Path) -> DatabaseStorage {
    let main_bytes = file_size(path);
    let wal_bytes = file_size(&sidecar_path(path, "-wal"));
    let shm_bytes = file_size(&sidecar_path(path, "-shm"));
    let total_bytes = main_bytes + wal_bytes + shm_bytes;
    let id = path
        .file_stem()
        .map(|name| name.to_string_lossy().replace('-', "_"))
        .unwrap_or_else(|| "database".to_string());

    DatabaseStorage {
        label: database_label(&id),
        id,
        path: path.to_string_lossy().to_string(),
        main_bytes,
        wal_bytes,
        shm_bytes,
        total_bytes,
    }
}

fn database_label(id: &str) -> String {
    match id {
        "catalog" => "Catalog database".to_string(),
        "user" => "User database".to_string(),
        "server" => "Server database".to_string(),
        "download_queue" => "Download queue database".to_string(),
        "search" => "Search index database".to_string(),
        "enrichment" => "Enrichment database".to_string(),
        "ingestion" => "Ingestion database".to_string(),
        "shows" => "Shows database".to_string(),
        other => format!("{} database", humanize_id(other)),
    }
}

fn humanize_id(id: &str) -> String {
    let mut chars = id.replace('_', " ").chars().collect::<Vec<_>>();
    if let Some(first) = chars.first_mut() {
        first.make_ascii_uppercase();
    }
    chars.into_iter().collect()
}

fn directory_component(id: &str, label: &str, path: &Path) -> StorageComponent {
    StorageComponent {
        id: id.to_string(),
        label: label.to_string(),
        path: path.to_string_lossy().to_string(),
        bytes: directory_size(path),
    }
}

fn directory_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }

    WalkDir::new(path)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| entry.metadata().ok())
        .map(|metadata| metadata.len())
        .sum()
}

fn file_size(path: &Path) -> u64 {
    path.metadata().map(|metadata| metadata.len()).unwrap_or(0)
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut sidecar = path.as_os_str().to_os_string();
    sidecar.push(suffix);
    PathBuf::from(sidecar)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn storage_report_counts_database_sidecars_and_media_components() {
        let temp = tempdir().unwrap();
        let db_dir = temp.path().join("db");
        let media_dir = temp.path().join("media");
        fs::create_dir_all(&db_dir).unwrap();
        fs::create_dir_all(media_dir.join("audio")).unwrap();
        fs::create_dir_all(media_dir.join("images")).unwrap();

        let catalog_db = db_dir.join("catalog.db");
        fs::write(&catalog_db, [0_u8; 10]).unwrap();
        fs::write(sidecar_path(&catalog_db, "-wal"), [0_u8; 5]).unwrap();
        fs::write(sidecar_path(&catalog_db, "-shm"), [0_u8; 3]).unwrap();
        fs::write(media_dir.join("audio").join("track.mp3"), [0_u8; 20]).unwrap();
        fs::write(media_dir.join("images").join("cover.jpg"), [0_u8; 7]).unwrap();

        let config = ServerConfig {
            db_dir,
            media_path: media_dir,
            ..ServerConfig::default()
        };

        let report = collect_storage_report(&config, vec![catalog_db]);

        assert_eq!(report.database_total_bytes, 18);
        assert_eq!(report.filesystem_total_bytes, 27);
        assert_eq!(report.total_bytes, 45);
        assert_eq!(report.databases[0].label, "Catalog database");
    }
}
