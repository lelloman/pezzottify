use super::{Album, Artist, Track};
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
    albums: HashMap<String, Album>,
    tracks: HashMap<String, Track>,
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

fn parse_tracks(dir: &Path, album: &Album) -> Result<Vec<Track>> {
    let mut out = Vec::new();
    let filenames_in_dir: Vec<String> = std::fs::read_dir(dir)
        .with_context(|| format!("Could not read album dir {}", dir.display()))?
        .filter_map(|entry| {
            entry
                .ok()
                .and_then(|e| Some(e.file_name().to_string_lossy().into_owned()))
        })
        .collect();

    for disc in album.discs.iter() {
        for track_id in disc.tracks.iter() {
            let track_filename_prefix = format!("track_{track_id}");
            let track_json_file = dir.join(format!("{track_filename_prefix}.json"));

            if !filenames_in_dir
                .iter()
                .any(|x| !x.ends_with(".json") && x.starts_with(&track_filename_prefix))
            {
                bail!(
                    "Could not find an audio file for track {track_id} in {}",
                    dir.display()
                );
            }
            let track_json_string = std::fs::read_to_string(&track_json_file)
                .with_context(|| format!("Failed to read {}", track_json_file.display()))?;
            let track: Track = serde_json::from_str(&track_json_string).with_context(|| {
                format!(
                    "Could not parse track json file {}",
                    track_json_file.display()
                )
            })?;
            out.push(track);
        }
    }
    Ok(out)
}

fn parse_albums_and_tracks(dir: &Path) -> Result<(HashMap<String, Album>, HashMap<String, Track>)> {
    let mut albums = HashMap::new();
    let mut tracks = HashMap::new();
    let album_dirname_regex =
        Regex::new("album_([A-z0-9]+)").expect("Invalid Regex, this should be fixed at runtime.");
    for dir_entry_result in std::fs::read_dir(dir)
        .with_context(|| format!("Could not read album dir {}", dir.display()))?
    {
        let path = dir_entry_result?.path();
        let filename = path
            .file_name()
            .with_context(|| "Invalid file \"{path}\"")?
            .to_string_lossy();

        if album_dirname_regex.captures(&filename).is_none() {
            bail!("Invalid album dir name \"{filename}\"");
        }

        let dirname_album_id = filename
            .strip_prefix("album_")
            .with_context(|| "Invalid album file name \"{filename}\"")?;

        let album_json_file = path.join(format!("album_{dirname_album_id}.json"));
        let album_json_string = std::fs::read_to_string(&album_json_file).with_context(|| {
            format!(
                "Could not read album json file {}",
                album_json_file.display()
            )
        })?;
        let album: Album = serde_json::from_str(&album_json_string)
            .with_context(|| "Could not parse album from json")?;

        let mut parsed_tracks =
            parse_tracks(&path, &album).with_context(|| "Could not parse tracks")?;
        loop {
            if parsed_tracks.is_empty() {
                break;
            }
            let track = parsed_tracks.remove(0);
            tracks.insert(track.id.clone(), track);
        }
        albums.insert(album.id.clone(), album);
    }
    Ok((albums, tracks))
}

impl Catalog {
    pub fn build(root_dir: &Path) -> Result<Catalog> {
        let dirs = Dirs::from_root(root_dir)?;
        let artists = parse_artists(&dirs.artists)?;
        let (albums, tracks) = parse_albums_and_tracks(&dirs.albums)?;
        Ok(Catalog {
            dirs,
            artists,
            tracks,
            albums,
        })
    }

    pub fn get_artists_count(&self) -> usize {
        self.artists.len()
    }

    pub fn get_albums_count(&self) -> usize {
        self.albums.len()
    }

    pub fn get_tracks_count(&self) -> usize {
        self.tracks.len()
    }
}
