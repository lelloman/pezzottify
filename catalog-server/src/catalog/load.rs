//! Catalog loading functionality
#![allow(dead_code)] // Validation functions for future use

use super::{Catalog, LoadCatalogProblem, Track};
use anyhow::{bail, Context, Result};
use rayon::iter::ParallelIterator;
use std::{
    path::PathBuf,
    process::{Command, Output},
};
use tracing::info;

fn ffprobe_file(path: PathBuf) -> Result<()> {
    let output: Output = Command::new("ffprobe")
        .args([
            "-loglevel",
            "error",
            "-i",
            &path.to_string_lossy().to_string(),
            "-select_streams",
            "a:0",
        ])
        .output()
        .context("Failed to execute ffprobe.")?;
    if output.status.success() && output.stderr.is_empty() {
        return Ok(());
    }
    bail!("Ffprobe output was not good: {:?}", output)
}

fn check_track_file(catalog: &Catalog, album_id: &str, track: &Track) -> Result<()> {
    let file_path = match catalog.get_track_audio_path(album_id, &track.id) {
        None => bail!("Could not determine track file path."),
        Some(x) => x,
    };

    if !file_path.exists() {
        bail!("Audio file {} does not exist.", file_path.display());
    }

    ffprobe_file(file_path)
}

pub fn load_catalog<P: AsRef<std::path::Path>>(path: P, check_all: bool) -> Result<Catalog> {
    let catalog_result = Catalog::build(path.as_ref());
    let mut problems = catalog_result.problems;
    let catalog = catalog_result.catalog;

    if check_all {
        info!("Performing checks...");
        if let Some(catalog) = catalog.as_ref() {
            let mut tracks_problems: Vec<LoadCatalogProblem> = catalog
                .par_iter_tracks()
                .filter_map(|(_, t)| match check_track_file(&catalog, &t.album_id, t) {
                    Ok(()) => None,
                    Err(err) => Some(LoadCatalogProblem::FfprobeFailure(format!(
                        "{} - {} - {}",
                        &t.album_id, t.id, err
                    ))),
                })
                .collect();
            problems.append(&mut tracks_problems);
        }
    } else {
        info!("Skipping checks.");
    }

    if !problems.is_empty() {
        info!("Found {} problems:", problems.len());
        for problem in problems.iter() {
            info!("- {:?}", problem);
        }
        info!("");
    }
    match (&catalog, problems.is_empty()) {
        (Some(_), true) => info!("Catalog checked, no issues found."),
        (Some(_), false) => info!(
            "Catalog was built, but check the {} non-fatal issues above.",
            problems.len()
        ),
        (None, _) => {
            info!(
                "Check the {} problems above, the catalog could not be initialized.",
                problems.len()
            )
        }
    }
    if let Some(catalog) = catalog {
        info!(
            "Catalog has:\n{} artists\n{} albums\n{} tracks",
            catalog.get_artists_count(),
            catalog.get_albums_count(),
            catalog.get_tracks_count()
        );
        return Ok(catalog);
    }

    bail!("Could not load catalog");
}
