//! Catalog Import Tool
//!
//! This binary imports a filesystem-based catalog into a SQLite database.
//! It reads the existing JSON catalog structure and transforms it into the new
//! SQLite schema.

use anyhow::Result;
use clap::Parser;
use pezzottify_catalog_server::catalog::{load_catalog, Album, Artist, Catalog, Image, Track};
use pezzottify_catalog_server::catalog_store::{
    ActivityPeriod as NewActivityPeriod, Album as NewAlbum, AlbumType as NewAlbumType,
    Artist as NewArtist, ArtistRole as NewArtistRole, Format as NewFormat, Image as NewImage,
    ImageSize as NewImageSize, ImageType, SqliteCatalogStore, Track as NewTrack,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "catalog-import")]
#[command(about = "Import a filesystem catalog into SQLite database")]
struct Args {
    /// Path to the catalog directory (containing albums/, artists/, images/)
    #[arg(value_name = "CATALOG_PATH")]
    catalog_path: PathBuf,

    /// Path to the output SQLite database file
    #[arg(value_name = "OUTPUT_DB")]
    output_db: PathBuf,

    /// Perform all catalog checks during import
    #[arg(long, default_value_t = false)]
    check_all: bool,

    /// Continue import even if some items fail to convert
    #[arg(long, default_value_t = true)]
    continue_on_error: bool,
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    info!("Catalog Import Tool");
    info!("===================");
    info!("Catalog path: {}", args.catalog_path.display());
    info!("Output database: {}", args.output_db.display());

    // Check if output database already exists
    if args.output_db.exists() {
        warn!(
            "Output database already exists: {}",
            args.output_db.display()
        );
        warn!("This will fail if the database already contains data.");
    }

    // Load the filesystem catalog
    info!("Loading filesystem catalog...");
    let catalog = load_catalog(&args.catalog_path, args.check_all)?;
    info!(
        "Loaded catalog: {} artists, {} albums, {} tracks",
        catalog.get_artists_count(),
        catalog.get_albums_count(),
        catalog.get_tracks_count()
    );

    // Create the SQLite store
    info!("Creating SQLite database...");
    let store = SqliteCatalogStore::new(&args.output_db, &args.catalog_path)?;

    // Begin import transaction
    info!("Starting import...");
    let tx = store.begin_import()?;

    // Track statistics
    let mut stats = ImportStats::default();

    // Import images first (needed for relationships)
    info!("Importing images...");
    let image_ids = import_images(&catalog, &store, &args, &mut stats)?;

    // Import artists
    info!("Importing artists...");
    import_artists(&catalog, &store, &args, &mut stats, &image_ids)?;

    // Import albums and tracks
    info!("Importing albums and tracks...");
    import_albums_and_tracks(&catalog, &store, &args, &mut stats, &image_ids)?;

    // Add artist relationships
    info!("Adding artist relationships...");
    add_artist_relationships(&catalog, &store, &args, &mut stats)?;

    // Commit the transaction
    tx.commit()?;

    // Print summary
    info!("");
    info!("Import Summary");
    info!("==============");
    info!("Artists imported: {}", stats.artists_imported);
    info!("Albums imported: {}", stats.albums_imported);
    info!("Tracks imported: {}", stats.tracks_imported);
    info!("Images imported: {}", stats.images_imported);
    if stats.errors > 0 {
        warn!("Errors encountered: {}", stats.errors);
    }

    // Verify counts
    let (artist_count, album_count, track_count, image_count) = store.get_counts()?;
    info!("");
    info!("Database contains:");
    info!("  {} artists", artist_count);
    info!("  {} albums", album_count);
    info!("  {} tracks", track_count);
    info!("  {} images", image_count);

    info!("");
    info!("Import completed successfully!");

    Ok(())
}

#[derive(Default)]
struct ImportStats {
    artists_imported: usize,
    albums_imported: usize,
    tracks_imported: usize,
    images_imported: usize,
    errors: usize,
}

/// Collect and import all images from the catalog directory.
fn import_images(
    catalog: &Catalog,
    store: &SqliteCatalogStore,
    args: &Args,
    stats: &mut ImportStats,
) -> Result<HashSet<String>> {
    let mut image_ids = HashSet::new();

    // Collect all unique image IDs from artists and albums
    for artist in catalog.iter_artists() {
        for image in &artist.portraits {
            image_ids.insert(image.id.clone());
        }
        for image in &artist.portrait_group {
            image_ids.insert(image.id.clone());
        }
    }

    for album in catalog.iter_albums() {
        for image in &album.covers {
            image_ids.insert(image.id.clone());
        }
        for image in &album.cover_group {
            image_ids.insert(image.id.clone());
        }
    }

    // Build a map of image metadata from all sources
    let mut image_map: std::collections::HashMap<String, Image> = std::collections::HashMap::new();

    for artist in catalog.iter_artists() {
        for image in &artist.portraits {
            image_map.insert(image.id.clone(), image.clone());
        }
        for image in &artist.portrait_group {
            image_map.insert(image.id.clone(), image.clone());
        }
    }

    for album in catalog.iter_albums() {
        for image in &album.covers {
            image_map.insert(image.id.clone(), image.clone());
        }
        for image in &album.cover_group {
            image_map.insert(image.id.clone(), image.clone());
        }
    }

    // Import each image
    for (id, old_image) in &image_map {
        match convert_and_insert_image(store, id, old_image, &args.catalog_path) {
            Ok(()) => stats.images_imported += 1,
            Err(e) => {
                error!("Failed to import image {}: {}", id, e);
                stats.errors += 1;
                if !args.continue_on_error {
                    return Err(e);
                }
            }
        }
    }

    Ok(image_ids)
}

fn convert_and_insert_image(
    store: &SqliteCatalogStore,
    id: &str,
    old_image: &Image,
    catalog_path: &Path,
) -> Result<()> {
    // The image file should be at images/{id}
    let uri = format!("images/{}", id);
    let full_path = catalog_path.join(&uri);

    if !full_path.exists() {
        warn!("Image file does not exist: {}", full_path.display());
    }

    let new_image = NewImage {
        id: id.to_string(),
        uri,
        size: convert_image_size(&old_image.size),
        width: old_image.width,
        height: old_image.height,
    };

    store.insert_image(&new_image)?;
    Ok(())
}

fn convert_image_size(old_size: &pezzottify_catalog_server::catalog::ImageSize) -> NewImageSize {
    match old_size {
        pezzottify_catalog_server::catalog::ImageSize::SMALL => NewImageSize::Small,
        pezzottify_catalog_server::catalog::ImageSize::DEFAULT => NewImageSize::Default,
        pezzottify_catalog_server::catalog::ImageSize::LARGE => NewImageSize::Large,
        pezzottify_catalog_server::catalog::ImageSize::XLARGE => NewImageSize::XLarge,
    }
}

/// Import all artists from the catalog.
fn import_artists(
    catalog: &Catalog,
    store: &SqliteCatalogStore,
    args: &Args,
    stats: &mut ImportStats,
    image_ids: &HashSet<String>,
) -> Result<()> {
    for old_artist in catalog.iter_artists() {
        match convert_and_insert_artist(store, old_artist, image_ids) {
            Ok(()) => stats.artists_imported += 1,
            Err(e) => {
                error!("Failed to import artist {}: {}", old_artist.id, e);
                stats.errors += 1;
                if !args.continue_on_error {
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}

fn convert_and_insert_artist(
    store: &SqliteCatalogStore,
    old_artist: &Artist,
    image_ids: &HashSet<String>,
) -> Result<()> {
    let new_artist = NewArtist {
        id: old_artist.id.clone(),
        name: old_artist.name.clone(),
        genres: old_artist.genre.clone(),
        activity_periods: old_artist
            .activity_periods
            .iter()
            .map(convert_activity_period)
            .collect(),
    };

    store.insert_artist(&new_artist)?;

    // Add portrait images
    for (position, image) in old_artist.portraits.iter().enumerate() {
        if image_ids.contains(&image.id) {
            store.add_artist_image(
                &old_artist.id,
                &image.id,
                &ImageType::Portrait,
                position as i32,
            )?;
        }
    }

    // Add portrait group images
    for (position, image) in old_artist.portrait_group.iter().enumerate() {
        if image_ids.contains(&image.id) {
            store.add_artist_image(
                &old_artist.id,
                &image.id,
                &ImageType::PortraitGroup,
                position as i32,
            )?;
        }
    }

    Ok(())
}

fn convert_activity_period(
    old: &pezzottify_catalog_server::catalog::ActivityPeriod,
) -> NewActivityPeriod {
    match old {
        pezzottify_catalog_server::catalog::ActivityPeriod::Timespan {
            start_year,
            end_year,
        } => NewActivityPeriod::Timespan {
            start_year: *start_year,
            end_year: *end_year,
        },
        pezzottify_catalog_server::catalog::ActivityPeriod::Decade(d) => NewActivityPeriod::Decade(*d),
    }
}

/// Import all albums and their tracks.
fn import_albums_and_tracks(
    catalog: &Catalog,
    store: &SqliteCatalogStore,
    args: &Args,
    stats: &mut ImportStats,
    image_ids: &HashSet<String>,
) -> Result<()> {
    for old_album in catalog.iter_albums() {
        match convert_and_insert_album(store, catalog, old_album, image_ids) {
            Ok(track_count) => {
                stats.albums_imported += 1;
                stats.tracks_imported += track_count;
            }
            Err(e) => {
                error!("Failed to import album {}: {}", old_album.id, e);
                stats.errors += 1;
                if !args.continue_on_error {
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}

fn convert_and_insert_album(
    store: &SqliteCatalogStore,
    catalog: &Catalog,
    old_album: &Album,
    image_ids: &HashSet<String>,
) -> Result<usize> {
    let new_album = NewAlbum {
        id: old_album.id.clone(),
        name: old_album.name.clone(),
        album_type: convert_album_type(&old_album.album_type),
        label: if old_album.label.is_empty() {
            None
        } else {
            Some(old_album.label.clone())
        },
        release_date: if old_album.date == 0 {
            None
        } else {
            Some(old_album.date)
        },
        genres: old_album.genres.clone(),
        original_title: if old_album.original_title.is_empty() {
            None
        } else {
            Some(old_album.original_title.clone())
        },
        version_title: if old_album.version_title.is_empty() {
            None
        } else {
            Some(old_album.version_title.clone())
        },
    };

    store.insert_album(&new_album)?;

    // Add album artists
    for (position, artist_id) in old_album.artists_ids.iter().enumerate() {
        store.add_album_artist(&old_album.id, artist_id, position as i32)?;
    }

    // Add cover images
    for (position, image) in old_album.covers.iter().enumerate() {
        if image_ids.contains(&image.id) {
            store.add_album_image(&old_album.id, &image.id, &ImageType::Cover, position as i32)?;
        }
    }

    // Add cover group images
    for (position, image) in old_album.cover_group.iter().enumerate() {
        if image_ids.contains(&image.id) {
            store.add_album_image(
                &old_album.id,
                &image.id,
                &ImageType::CoverGroup,
                position as i32,
            )?;
        }
    }

    // Import tracks for this album
    let mut track_count = 0;
    for disc in &old_album.discs {
        for track_id in &disc.tracks {
            if let Some(old_track) = catalog.get_track(track_id) {
                convert_and_insert_track(store, &old_track, &old_album.id, disc.number)?;
                track_count += 1;
            } else {
                warn!(
                    "Track {} referenced by album {} not found",
                    track_id, old_album.id
                );
            }
        }
    }

    Ok(track_count)
}

fn convert_album_type(old: &pezzottify_catalog_server::catalog::AlbumType) -> NewAlbumType {
    match old {
        pezzottify_catalog_server::catalog::AlbumType::ALBUM => NewAlbumType::Album,
        pezzottify_catalog_server::catalog::AlbumType::SINGLE => NewAlbumType::Single,
        pezzottify_catalog_server::catalog::AlbumType::EP => NewAlbumType::Ep,
        pezzottify_catalog_server::catalog::AlbumType::COMPILATION => NewAlbumType::Compilation,
        pezzottify_catalog_server::catalog::AlbumType::AUDIOBOOK => NewAlbumType::Audiobook,
        pezzottify_catalog_server::catalog::AlbumType::PODCAST => NewAlbumType::Podcast,
    }
}

fn convert_and_insert_track(
    store: &SqliteCatalogStore,
    old_track: &Track,
    album_id: &str,
    disc_number: i32,
) -> Result<()> {
    // Find the audio file for this track
    // The format key tells us the format, and the files HashMap contains the relative paths
    let (format, audio_uri) = if let Some((fmt, path)) = old_track.files.iter().next() {
        (convert_format(fmt), path.clone())
    } else {
        // Fallback: construct expected path
        let track_id_suffix = old_track.id.strip_prefix('T').unwrap_or(&old_track.id);
        let album_id_suffix = album_id.strip_prefix('A').unwrap_or(album_id);
        (
            NewFormat::Unknown,
            format!("albums/album_{}/track_{}.mp3", album_id_suffix, track_id_suffix),
        )
    };

    let new_track = NewTrack {
        id: old_track.id.clone(),
        name: old_track.name.clone(),
        album_id: album_id.to_string(),
        disc_number,
        track_number: old_track.number,
        duration_secs: if old_track.duration == 0 {
            None
        } else {
            Some(old_track.duration)
        },
        is_explicit: old_track.is_explicit,
        audio_uri,
        format,
        tags: old_track.tags.clone(),
        has_lyrics: old_track.has_lyrics,
        languages: old_track.language_of_performance.clone(),
        original_title: if old_track.original_title.is_empty() {
            None
        } else {
            Some(old_track.original_title.clone())
        },
        version_title: if old_track.version_title.is_empty() {
            None
        } else {
            Some(old_track.version_title.clone())
        },
    };

    store.insert_track(&new_track)?;

    // Add track artists with roles
    for (position, artist_with_role) in old_track.artists_with_role.iter().enumerate() {
        let role = convert_artist_role(&artist_with_role.role);
        store.add_track_artist(
            &old_track.id,
            &artist_with_role.artist_id,
            &role,
            position as i32,
        )?;
    }

    // If no artists_with_role, fall back to artists_ids
    if old_track.artists_with_role.is_empty() {
        for (position, artist_id) in old_track.artists_ids.iter().enumerate() {
            store.add_track_artist(
                &old_track.id,
                artist_id,
                &NewArtistRole::MainArtist,
                position as i32,
            )?;
        }
    }

    Ok(())
}

fn convert_format(old: &pezzottify_catalog_server::catalog::Format) -> NewFormat {
    match old {
        pezzottify_catalog_server::catalog::Format::OGG_VORBIS_96 => NewFormat::OggVorbis96,
        pezzottify_catalog_server::catalog::Format::OGG_VORBIS_160 => NewFormat::OggVorbis160,
        pezzottify_catalog_server::catalog::Format::OGG_VORBIS_320 => NewFormat::OggVorbis320,
        pezzottify_catalog_server::catalog::Format::MP3_96 => NewFormat::Mp3_96,
        pezzottify_catalog_server::catalog::Format::MP3_160 => NewFormat::Mp3_160,
        pezzottify_catalog_server::catalog::Format::MP3_160_ENC => NewFormat::Mp3_160,
        pezzottify_catalog_server::catalog::Format::MP3_256 => NewFormat::Mp3_256,
        pezzottify_catalog_server::catalog::Format::MP3_320 => NewFormat::Mp3_320,
        pezzottify_catalog_server::catalog::Format::AAC_24 => NewFormat::Aac24,
        pezzottify_catalog_server::catalog::Format::AAC_48 => NewFormat::Aac48,
        pezzottify_catalog_server::catalog::Format::AAC_160 => NewFormat::Aac160,
        pezzottify_catalog_server::catalog::Format::AAC_320 => NewFormat::Aac320,
        pezzottify_catalog_server::catalog::Format::FLAC_FLAC => NewFormat::Flac,
        _ => NewFormat::Unknown,
    }
}

fn convert_artist_role(old: &pezzottify_catalog_server::catalog::ArtistRole) -> NewArtistRole {
    match old {
        pezzottify_catalog_server::catalog::ArtistRole::ARTIST_ROLE_MAIN_ARTIST => {
            NewArtistRole::MainArtist
        }
        pezzottify_catalog_server::catalog::ArtistRole::ARTIST_ROLE_FEATURED_ARTIST => {
            NewArtistRole::FeaturedArtist
        }
        pezzottify_catalog_server::catalog::ArtistRole::ARTIST_ROLE_REMIXER => {
            NewArtistRole::Remixer
        }
        pezzottify_catalog_server::catalog::ArtistRole::ARTIST_ROLE_COMPOSER => {
            NewArtistRole::Composer
        }
        pezzottify_catalog_server::catalog::ArtistRole::ARTIST_ROLE_CONDUCTOR => {
            NewArtistRole::Conductor
        }
        pezzottify_catalog_server::catalog::ArtistRole::ARTIST_ROLE_ORCHESTRA => {
            NewArtistRole::Orchestra
        }
        pezzottify_catalog_server::catalog::ArtistRole::ARTIST_ROLE_ACTOR => NewArtistRole::Actor,
        pezzottify_catalog_server::catalog::ArtistRole::ARTIST_ROLE_UNKNOWN => {
            NewArtistRole::Unknown
        }
    }
}

/// Add related artist relationships.
fn add_artist_relationships(
    catalog: &Catalog,
    store: &SqliteCatalogStore,
    args: &Args,
    stats: &mut ImportStats,
) -> Result<()> {
    for old_artist in catalog.iter_artists() {
        for related_id in &old_artist.related {
            // Only add if the related artist exists (was imported)
            if catalog.get_artist(related_id).is_some() {
                match store.add_related_artist(&old_artist.id, related_id) {
                    Ok(()) => {}
                    Err(e) => {
                        error!(
                            "Failed to add related artist {} -> {}: {}",
                            old_artist.id, related_id, e
                        );
                        stats.errors += 1;
                        if !args.continue_on_error {
                            return Err(e);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
