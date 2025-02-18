use super::album::{ResolvedAlbum, ResolvedTrack};
use super::{album, Album, Artist, Image, Track, TrackFormat};
use anyhow::{bail, Context, Result};
use rayon::iter::IntoParallelRefIterator;
use regex::Regex;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

#[derive(Debug, Clone)]
pub struct Dirs {
    root: PathBuf,
    pub albums: PathBuf,
    pub artists: PathBuf,
    pub images: PathBuf,
}

impl Dirs {
    #[cfg(test)]
    fn dummy() -> Dirs {
        let p = PathBuf::from("/tmp/foo/bar");
        Dirs {
            root: p.clone(),
            albums: p.clone(),
            artists: p.clone(),
            images: p.clone(),
        }
    }

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

    pub fn get_image_path(&self, id: String) -> PathBuf {
        let mut output = self.images.clone();
        output.push(id);
        output
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
#[allow(dead_code)]
pub enum Problem {
    InvalidRootDir,
    MissingCatalogDir(String),
    CantReadDir(String),
    InvalidArtistFile(String),
    InvalidAlbumDirName(String),
    InvalidAlbumFile(String),
    InvalidAlbumTracks(String),
    MissingReferencedId(String),
    MissingTrackArtistId(String),
    MissingImage(String),
    FfprobeFailure(String),
}

fn get_artist_id_from_filename<'a>(filename: &'a Cow<'a, str>) -> Result<&'a str> {
    filename
        .strip_prefix("artist_")
        .with_context(|| "Invalid artist file name \"{filename}\"")?
        .strip_suffix(".json")
        .with_context(|| "Invalid artist file name \"{filename}\"")
}

struct IdPresenceChecker {
    dirs: Dirs,
}

impl IdPresenceChecker {
    pub fn new(dirs: &Dirs) -> IdPresenceChecker {
        IdPresenceChecker { dirs: dirs.clone() }
    }
    pub fn is_id_present<T: AsRef<str>>(&self, id: T) -> bool {
        self.dirs
            .artists
            .join(format!("artist_{}.json", id.as_ref()))
            .exists()
            || self
                .dirs
                .albums
                .join(format!("album_{}.json", id.as_ref()))
                .exists()
            || self.dirs.images.join(id.as_ref()).exists()
    }

    pub fn check_images_exist<F>(&self, source: F, images: &Vec<Image>, problems: &mut Vec<Problem>)
    where
        F: Fn() -> String,
    {
        let mut reified_source: Option<String> = None;
        for image in images.iter() {
            if !self.is_id_present(&image.id) {
                if let None = reified_source {
                    reified_source = Some(source());
                }
                problems.push(Problem::MissingImage(format!(
                    "{} has missing image {}.",
                    &reified_source.as_ref().unwrap(),
                    image.id
                )));
            }
        }
    }
}

fn parse_artists(dirs: &Dirs, problems: &mut Vec<Problem>) -> HashMap<String, Artist> {
    let mut out = HashMap::new();
    let artist_filename_regex = Regex::new("artist_([A-z0-9]+)\\.json")
        .expect("Invalid Regex, this should be fixed at runtime.");
    let dir_entries = match std::fs::read_dir(&dirs.artists) {
        Ok(x) => x,
        Err(x) => {
            let msg = format!(
                "Error reading artists dir {}\n{}",
                &dirs.artists.display(),
                x
            );
            problems.push(Problem::CantReadDir(msg));
            return out;
        }
    };
    let id_checker = IdPresenceChecker::new(dirs);
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
                    path.display(),
                    x
                )));
                continue;
            }
        };
        if parsed_artist.id != filename_artist_id {
            let msg = format!("File name {filename} implies {filename_artist_id} artist id, but the parsed artist has id {}", parsed_artist.id);
            problems.push(Problem::InvalidArtistFile(msg));
            continue;
        }
        for related in parsed_artist.related.iter() {
            if !id_checker.is_id_present(related) {
                problems.push(Problem::MissingReferencedId(format!(
                    "Artist {} related id {} is missing.",
                    parsed_artist.id, related
                )));
            }
        }
        id_checker.check_images_exist(
            || {
                format!(
                    "Artist {} - {} portrait",
                    &parsed_artist.id, &parsed_artist.name
                )
            },
            &parsed_artist.portrait_group,
            problems,
        );
        id_checker.check_images_exist(
            || {
                format!(
                    "Artist {} - {} portrait",
                    &parsed_artist.id, &parsed_artist.name
                )
            },
            &parsed_artist.portraits,
            problems,
        );
        out.insert(filename_artist_id.to_owned(), parsed_artist);
    }
    out
}

fn parse_tracks(
    dirs: &Dirs,
    album_dir: &Path,
    album: &Album,
    problems: &mut Vec<Problem>,
) -> Result<Vec<Track>> {
    let mut out = Vec::new();
    let id_checker = IdPresenceChecker::new(dirs);
    let filenames_in_dir: Vec<String> = std::fs::read_dir(album_dir)
        .with_context(|| format!("Could not read album dir {}", album_dir.display()))?
        .filter_map(|entry| {
            entry
                .ok()
                .and_then(|e| Some(e.file_name().to_string_lossy().into_owned()))
        })
        .collect();

    for disc in album.discs.iter() {
        for track_id in disc.tracks.iter() {
            let track_filename_prefix = format!("track_{track_id}");
            let track_json_file = album_dir.join(format!("{track_filename_prefix}.json"));

            if !filenames_in_dir
                .iter()
                .any(|x| !x.ends_with(".json") && x.starts_with(&track_filename_prefix))
            {
                bail!(
                    "Could not find an audio file for track {track_id} in {}",
                    album_dir.display()
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
            for artist_id in track.artists_ids.iter() {
                if !id_checker.is_id_present(artist_id) {
                    problems.push(Problem::MissingTrackArtistId(format!(
                        "Track {} in album {} has missing artist {}.",
                        track.id, track.album_id, artist_id
                    )));
                }
            }
            out.push(track);
        }
    }
    Ok(out)
}

fn parse_albums_and_tracks(
    dirs: &Dirs,
    problems: &mut Vec<Problem>,
) -> (HashMap<String, Album>, HashMap<String, Track>) {
    let mut albums = HashMap::new();
    let mut tracks = HashMap::new();
    let album_dirname_regex = Regex::new("album_([A-z0-9]+)")
        .expect("Invalid Regex, this should be fixed at compile time.");
    let read_dir = std::fs::read_dir(&dirs.albums).unwrap();
    for dir_entry_result in read_dir.into_iter() {
        let path = problemo!(dir_entry_result, problems, |e| Problem::CantReadDir(
            format!("Cant read album dir.\n{}", e)
        ));
        let path = path.path();

        let filename = problemo!(path.file_name().context(""), problems, |e| {
            Problem::InvalidAlbumDirName(format!(
                "Invalid name {}, {}",
                path.display().to_string(),
                e
            ))
        });
        let filename = filename.to_string_lossy();

        let filename_match = album_dirname_regex.captures(&filename).context("");
        problemo!(filename_match, problems, |e| Problem::InvalidAlbumDirName(
            format!("Invalid name {}, {}", filename.into_owned(), e)
        ));

        let stripped = filename.strip_prefix("album_").context("");
        let dirname_album_id = problemo!(stripped, problems, |e| {
            Problem::InvalidAlbumDirName(format!("Invalid name {}, {}", filename.into_owned(), e))
        });

        let album_json_file = path.join(format!("album_{dirname_album_id}.json"));
        let file_read = std::fs::read_to_string(&album_json_file);
        let album_json_string =
            problemo!(file_read, problems, |e| Problem::InvalidAlbumFile(format!(
                "Could not read album json file {}, {}",
                album_json_file.display(),
                e
            )));

        let json_read = serde_json::from_str(&album_json_string);
        let album: Album = problemo!(json_read, problems, |e| {
            Problem::InvalidAlbumFile(format!("Could not parse album.\n{}", e))
        });

        let mut parsed_tracks =
            problemo!(parse_tracks(dirs, &path, &album, problems), problems, |e| {
                Problem::InvalidAlbumTracks(format!("{} - {} - {}", album.id, album.name, e))
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

fn get_track_audio_path(dirs: &Dirs, album_id: &str, track_id: &str) -> Option<PathBuf> {
    let album_dir = dirs.albums.join(format!("album_{}", album_id));

    let track_file_prefix = format!("track_{}", &track_id);
    if let Ok(entries) = std::fs::read_dir(album_dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() {
                    if let Some(file_name) = path.file_name() {
                        let name = file_name.to_string_lossy();
                        if name.starts_with(&track_file_prefix) {
                            if !name.to_lowercase().ends_with("json") {
                                return Some(path);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

impl Catalog {
    #[cfg(test)]
    pub fn dummy() -> Catalog {
        Catalog {
            dirs: Dirs::dummy(),
            albums: HashMap::new(),
            artists: HashMap::new(),
            tracks: HashMap::new(),
        }
    }

    pub fn build(root_dir: &Path) -> CatalogBuildResult {
        let mut problems = Vec::<Problem>::new();
        let dirs = match Dirs::from_root(root_dir, &mut problems) {
            Ok(x) => x,
            Err(_) => return CatalogBuildResult::only_problems(problems),
        };
        let artists = parse_artists(&dirs, &mut problems);
        let (albums, tracks) = parse_albums_and_tracks(&dirs, &mut problems);
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

    pub fn infer_path() -> Option<PathBuf> {
        let mut current_dir = std::env::current_dir().ok()?;

        loop {
            if let Ok(entries) = std::fs::read_dir(&current_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();

                    if let Ok(d) = Dirs::from_root(&path, &mut vec![]) {
                        return Some(path);
                    }
                }
            }

            if let Some(parent) = current_dir.parent() {
                current_dir = parent.to_path_buf();
            } else {
                break;
            }
        }

        None
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

    pub fn get_artist(&self, artist_id: &str) -> Option<Artist> {
        self.artists.get(artist_id).cloned()
    }

    pub fn get_track(&self, track_id: &str) -> Option<Track> {
        self.tracks.get(track_id).cloned()
    }

    pub fn get_album(&self, album_id: &str) -> Option<Album> {
        self.albums.get(album_id).cloned()
    }

    pub fn iter_artists(&self) -> impl Iterator<Item = &Artist> {
        self.artists.values()
    }

    pub fn iter_albums(&self) -> impl Iterator<Item = &Album> {
        self.albums.values()
    }

    pub fn iter_tracks(&self) -> impl Iterator<Item = &Track> {
        self.tracks.values()
    }

    pub fn par_iter_tracks(
        &self,
    ) -> rayon::collections::hash_map::Iter<'_, std::string::String, Track> {
        self.tracks.par_iter()
    }

    pub fn get_image_path(&self, id: String) -> PathBuf {
        self.dirs.get_image_path(id)
    }

    pub fn get_track_audio_path(&self, album_id: &str, track_id: &str) -> Option<PathBuf> {
        get_track_audio_path(&self.dirs, album_id, track_id)
    }

    pub fn get_artist_albums(&self, artist_id: String) -> Option<Vec<String>> {
        if let None = self.get_artist(&artist_id) {
            return None;
        }

        let album_ids = self
            .albums
            .values()
            .filter_map(|album| {
                if album.artists_ids.contains(&artist_id) {
                    Some(album.id.clone())
                } else {
                    None
                }
            })
            .collect();
        Some(album_ids)
    }

    pub fn get_resolved_track(&self, track_id: &str) -> Result<Option<ResolvedTrack>> {
        let track = match self.get_track(&track_id) {
            None => return Ok(None),
            Some(x) => x,
        };

        let album = match self.get_album(&track.album_id) {
            None => bail!("Could not find album {}", &track.album_id),
            Some(album) => album,
        };

        let mut artists: HashMap<String, Artist> = HashMap::new();
        for artist_id in track.artists_ids.iter() {
            let artist = match self.get_artist(&artist_id) {
                None => bail!("Could not find artist {}", &artist_id),
                Some(a) => a,
            };
            artists.insert(artist_id.to_owned(), artist);
        }

        let mut tracks = HashMap::new();
        tracks.insert(track_id.to_owned(), track);
        Ok(Some(ResolvedTrack {
            tracks,
            album,
            artists,
        }))
    }

    pub fn get_resolved_album(&self, album_id: &str) -> Result<Option<ResolvedAlbum>> {
        let album = match self.get_album(album_id) {
            Some(album) => album,
            None => return Ok(None),
        };

        let mut needed_artists_ids: HashSet<String> = HashSet::new();

        for id in album.artists_ids.iter() {
            needed_artists_ids.insert(id.to_string());
        }

        let mut tracks: HashMap<String, Track> = HashMap::new();
        for disc in album.discs.iter() {
            for track_id in disc.tracks.iter() {
                match self.get_track(&track_id) {
                    Some(track) => {
                        let artists_ids = &track.artists_ids;
                        for artist_id in artists_ids.iter() {
                            needed_artists_ids.insert(artist_id.to_owned());
                        }
                        tracks.insert(track_id.clone(), track.clone());
                    }
                    None => bail!("Could not find track {}", track_id),
                }
            }
        }

        let mut artists: HashMap<String, Artist> = HashMap::new();
        for artist_id in needed_artists_ids.iter() {
            match self.get_artist(artist_id) {
                Some(artist) => {
                    artists.insert(artist_id.to_string(), artist);
                }
                None => bail!("Could not find artist {}", artist_id),
            }
        }

        let resolved_album = ResolvedAlbum {
            album,
            artists,
            tracks,
        };
        Ok(Some(resolved_album))
    }
}
