use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct Dirs {
    root: PathBuf,
    albums: PathBuf,
    artists: PathBuf,
    images: PathBuf,
}

impl Dirs {
    fn from_root(root: &Path) -> Result<Dirs> {
        if !root.is_dir() {
            bail!("{} is not a valid directory.", root.display());
        }

        let albums = root.join("albums");
        let artists = root.join("artists");
        let images = root.join("images");

        if !albums.is_dir() {
            bail!("No albums dir in {}", root.display());
        }

        if !artists.is_dir() {
            bail!("No artists dir in {}", root.display());
        }

        if !images.is_dir() {
            bail!("No images dir in {}", root.display());
        }

        Ok(Dirs {
            root: root.to_owned(),
            albums,
            images,
            artists,
        })
    }
}

#[derive(Debug)]
pub struct Catalog {
    dirs: Dirs,
}

impl Catalog {
    pub fn build(root_dir: &Path) -> Result<Catalog> {
        let dirs = Dirs::from_root(root_dir)?;

        Ok(Catalog { dirs })
    }
}
