CREATE TABLE IF NOT EXISTS genre_recording_policy (
    genre TEXT PRIMARY KEY NOT NULL
);
CREATE TABLE IF NOT EXISTS genre_recordings (
    genre TEXT NOT NULL,
    track_id TEXT NOT NULL,
    evidence TEXT NOT NULL,
    UNIQUE (genre, track_id)
);
INSERT OR IGNORE INTO genre_recording_policy (genre) VALUES ('opera');
