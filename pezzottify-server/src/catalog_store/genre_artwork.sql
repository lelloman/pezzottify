CREATE TABLE IF NOT EXISTS genre_artwork (
    genre TEXT PRIMARY KEY NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    selected_at INTEGER NOT NULL
);
