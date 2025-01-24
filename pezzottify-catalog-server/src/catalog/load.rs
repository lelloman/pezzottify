use tracing::info;
use anyhow::{bail, Result};
use super::Catalog;

pub fn load_catalog<P : AsRef<std::path::Path>>(path: P) -> Result<Catalog> {
    let catalog_result = Catalog::build(path.as_ref());
    let problems = catalog_result.problems;
    let catalog = catalog_result.catalog;

    if !problems.is_empty() {
        info!("Found {} problems:", problems.len());
        for problem in problems.iter() {
            info!("- {:?}", problem);
        }
        info!("");
    }

    match (&catalog, problems.is_empty()) {
        (Some(_), true) => info!("Catalog checked, no issues found."),
        (Some(_), false) => info!("Catalog was built, but check the {} non-fatal issues above.", problems.len()),
        (None, _) => {
            info!("Check the {} problems above, the catalog could not be initialized.", problems.len())
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