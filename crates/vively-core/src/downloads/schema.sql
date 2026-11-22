CREATE TABLE cached_track (
    full_id TEXT PRIMARY KEY NOT NULL,
    owner_id INTEGER NOT NULL,
    audio_id INTEGER NOT NULL,
    access_key TEXT,
    title TEXT NOT NULL,
    artist TEXT NOT NULL,
    duration INTEGER NOT NULL,
    album_title TEXT,
    artwork_url TEXT,
    artwork_file TEXT,
    metadata BLOB NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('queued', 'downloading', 'ready', 'failed')),
    manual_pin INTEGER NOT NULL DEFAULT 0,
    byte_size INTEGER NOT NULL DEFAULT 0,
    added_at INTEGER NOT NULL,
    last_played_at INTEGER,
    priority INTEGER NOT NULL DEFAULT 1 CHECK (priority IN (0, 1)),
    requested_at INTEGER NOT NULL,
    url_fetched_at INTEGER,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    source_kind TEXT,
    source_id TEXT,
    source_offset INTEGER
);
CREATE INDEX cached_track_state ON cached_track(state, priority, requested_at);
CREATE INDEX cached_track_lru ON cached_track(manual_pin, state, last_played_at, added_at);
CREATE TABLE download_collection (
    full_id TEXT PRIMARY KEY NOT NULL,
    owner_id INTEGER NOT NULL,
    collection_id INTEGER NOT NULL,
    access_key TEXT,
    metadata BLOB NOT NULL,
    artwork_file TEXT,
    added_at INTEGER NOT NULL
);
CREATE TABLE collection_track (
    collection_full_id TEXT NOT NULL REFERENCES download_collection(full_id) ON DELETE CASCADE,
    track_full_id TEXT NOT NULL REFERENCES cached_track(full_id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    PRIMARY KEY (collection_full_id, track_full_id)
);
CREATE INDEX collection_track_track ON collection_track(track_full_id);
PRAGMA user_version = 1;
