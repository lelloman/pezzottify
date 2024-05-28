use super::{Album, Artist, Track};
use anyhow::{bail, Context, Result};
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

macro_rules! problemo {
    ($e:expr, $problems:expr, $problem_gen:expr) => {
        match $e {
            Ok(x) => x,
            Err(x) => {
                $problems.push($problem_gen(x));
                continue;
            }
        }
    };
}

#[derive(Debug)]
struct Dirs {
    root: PathBuf,
    albums: PathBuf,
    artists: PathBuf,
    images: PathBuf,
}

impl Dirs {
    fn from_root(root: &Path, problems: &mut Vec<Problem>) -> Result<Dirs> {
        if !root.is_dir() {
            problems.push(Problem::InvalidRootDir);
            bail!("{} is not a valid directory.", root.display());
        }

        let albums = root.join("albums");
        let artists = root.join("artists");
        let images = root.join("images");

        let previous_problems = problems.len();

        if !albums.is_dir() {
            problems.push(Problem::MissingCatalogDir("albums".to_owned()));
        }
        if !artists.is_dir() {
            problems.push(Problem::MissingCatalogDir("artists".to_owned()));
        }
        if !images.is_dir() {
            problems.push(Problem::MissingCatalogDir("images".to_owned()));
        }

        if previous_problems < problems.len() {
            bail!("Something is wrong with root dir layout.")
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

#[derive(Debug)]
pub enum Problem {
    InvalidRootDir,
    MissingCatalogDir(String),
    CantReadDir(String),
    InvalidArtistFile(String),
    InvalidAlbumDirName(String),
    InvalidAlbumFile(String),
    InvalidAlbumTracks(String),
}

fn get_artist_id_from_filename<'a>(filename: &'a Cow<'a, str>) -> Result<&'a str> {
    filename
        .strip_prefix("artist_")
        .with_context(|| "Invalid artist file name \"{filename}\"")?
        .strip_suffix(".json")
        .with_context(|| "Invalid artist file name \"{filename}\"")
}

fn parse_artists(dir: &Path, problems: &mut Vec<Problem>) -> HashMap<String, Artist> {
    let mut out = HashMap::new();
    let artist_filename_regex = Regex::new("artist_([A-z0-9]+)\\.json")
        .expect("Invalid Regex, this should be fixed at runtime.");
    let dir_entries = match std::fs::read_dir(dir) {
        Ok(x) => x,
        Err(x) => {
            let msg = format!("Error reading artists dir {}\n{}", dir.display(), x);
            problems.push(Problem::CantReadDir(msg));
            return out;
        }
    };
    for dir_entry_result in dir_entries {
        let path = problemo!(&dir_entry_result, problems, |e| {
            Problem::InvalidArtistFile(format!("{:?}\n{}", dir_entry_result, e))
        });
        let path = path.path();

        let filename = problemo!(path.file_name().context(""), problems, |e| {
            Problem::InvalidArtistFile(format!("Invalid file {:?}\n{}", dir_entry_result, e))
        });
        let filename = filename.to_string_lossy();

        if artist_filename_regex.captures(&filename).is_none() {
            problems.push(Problem::InvalidArtistFile(format!(
                "Invalid artist file name {}",
                filename
            )));
            continue;
        }

        let filename_artist_id = match get_artist_id_from_filename(&filename) {
            Ok(x) => x,
            Err(x) => {
                problems.push(Problem::InvalidArtistFile(format!(
                    "{:?}\n{}",
                    dir_entry_result, x
                )));
                continue;
            }
        };

        let file_text = std::fs::read_to_string(&path).unwrap_or_default();
        let parsed_artist: Artist = match serde_json::from_str(&file_text) {
            Ok(x) => x,
            Err(x) => {
                problems.push(Problem::InvalidArtistFile(format!(
                    "Could not deserialize {}\n{}",
                    path.display(), x
                )));
                continue;
            }
        };
        if parsed_artist.id != filename_artist_id {
            let msg = format!("File name {filename} implies {filename_artist_id} artist id, but the parsed artist has id {}", parsed_artist.id);
            problems.push(Problem::InvalidArtistFile(msg));
            continue;
        }
        out.insert(filename_artist_id.to_owned(), parsed_artist);
    }
    out
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

fn parse_albums_and_tracks(
    dir: &Path,
    problems: &mut Vec<Problem>,
) -> (HashMap<String, Album>, HashMap<String, Track>) {
    let mut albums = HashMap::new();
    let mut tracks = HashMap::new();
    let album_dirname_regex =
        Regex::new("album_([A-z0-9]+)").expect("Invalid Regex, this should be fixed at runtime.");
    let read_dir = std::fs::read_dir(dir).unwrap();
    for dir_entry_result in read_dir.into_iter() {
        let path = problemo!(dir_entry_result, problems, |e| Problem::CantReadDir(
            format!("Cant read album dir.\n{}", e)
        ));
        let path = path.path();

        let filename = problemo!(path.file_name().context(""), problems, |e| {
            Problem::InvalidAlbumDirName(path.display().to_string())
        });
        let filename = filename.to_string_lossy();

        let filename_match = album_dirname_regex.captures(&filename).context("");
        problemo!(filename_match, problems, |e| Problem::InvalidAlbumDirName(
            filename.into_owned()
        ));

        let stripped = filename.strip_prefix("album_").context("");
        let dirname_album_id = problemo!(stripped, problems, |e| {
            Problem::InvalidAlbumDirName(filename.into_owned())
        });

        let album_json_file = path.join(format!("album_{dirname_album_id}.json"));
        let file_read = std::fs::read_to_string(&album_json_file);
        let album_json_string =
            problemo!(file_read, problems, |e| Problem::InvalidAlbumFile(format!(
                "Could not read album json file {}",
                album_json_file.display()
            )));

        let json_read = serde_json::from_str(&album_json_string);
        let album: Album = problemo!(json_read, problems, |e| {
            Problem::InvalidAlbumFile(format!("Could not parse album.\n{}", e))
        });

        let mut parsed_tracks = problemo!(parse_tracks(&path, &album), problems, |e| {
            Problem::InvalidAlbumTracks(format!("{} - {}", album.id, album.name))
        });
        loop {
            if parsed_tracks.is_empty() {
                break;
            }
            let track = parsed_tracks.remove(0);
            tracks.insert(track.id.clone(), track);
        }
        albums.insert(album.id.clone(), album);
    }
    (albums, tracks)
}

pub struct CatalogBuildResult {
    pub catalog: Option<Catalog>,
    pub problems: Vec<Problem>,
}

impl CatalogBuildResult {
    fn only_problems(problems: Vec<Problem>) -> CatalogBuildResult {
        CatalogBuildResult {
            catalog: None,
            problems,
        }
    }
}

impl Catalog {
    pub fn build(root_dir: &Path) -> CatalogBuildResult {
        let mut problems = Vec::<Problem>::new();
        let dirs = match Dirs::from_root(root_dir, &mut problems) {
            Ok(x) => x,
            Err(x) => return CatalogBuildResult::only_problems(problems),
        };
        let artists = parse_artists(&dirs.artists, &mut problems);
        let (albums, tracks) = parse_albums_and_tracks(&dirs.albums, &mut problems);
        let catalog = Catalog {
            dirs,
            artists,
            albums,
            tracks,
        };
        CatalogBuildResult {
            catalog: Some(catalog),
            problems,
        }
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
