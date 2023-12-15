use super::Artist;
use anyhow::{bail, Context, Result};
use regex::Regex;
use std::collections::HashMap;
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
    artists: HashMap<String, Artist>,
}

fn parse_artists(dir: &Path) -> Result<HashMap<String, Artist>> {
    let mut out = HashMap::new();
    let artist_filename_regex = Regex::new("artist_([A-z0-9]+)\\.json")
        .expect("Invalid Regex, this should be fixed at runtime.");
    for dir_entry_result in std::fs::read_dir(dir)? {
        let path = dir_entry_result?.path();
        let filename = path
            .file_name()
            .with_context(|| "Invalid file \"{path}\"")?
            .to_string_lossy();
        if artist_filename_regex.captures(&filename).is_none() {
            bail!("Invalid artist file name \"{filename}\"");
        }
        let filename_artist_id = filename
            .strip_prefix("artist_")
            .with_context(|| "Invalid artist file name \"{filename}\"")?
            .strip_suffix(".json")
            .with_context(|| "Invalid artist file name \"{filename}\"")?;

        let file_text = std::fs::read_to_string(&path)?;
        let parsed_artist: Artist = serde_json::from_str(&file_text)?;
        if parsed_artist.id != filename_artist_id {
            bail!("File name {filename} implies {filename_artist_id} artist id, but the parsed artist has id {}", parsed_artist.id);
        }
        out.insert(filename_artist_id.to_owned(), parsed_artist);
    }
    Ok(out)
}

impl Catalog {
    pub fn build(root_dir: &Path) -> Result<Catalog> {
        let dirs = Dirs::from_root(root_dir)?;
        let artists = parse_artists(&dirs.artists)?;
        Ok(Catalog { dirs, artists })
    }

    pub fn get_artists_count(&self) -> usize {
        self.artists.len()
    }
}
