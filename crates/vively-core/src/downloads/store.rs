use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use rusqlite::{Connection, OptionalExtension};
use sha1::{Digest, Sha1};
use url::Url;

use super::*;
use crate::{
    Error, Result,
    models::{Audio, Playlist},
    session::atomic_write,
};

pub const SCHEMA_V1: &str = include_str!("schema.sql");

#[derive(Debug, Clone)]
pub struct OfflinePaths {
    pub container: PathBuf,
}

impl OfflinePaths {
    pub fn new(root: &Path, user_id: i64) -> Self {
        Self {
            container: root.join(user_id.to_string()),
        }
    }
    pub fn prepare(&self) -> Result<()> {
        fs::create_dir_all(self.tracks())?;
        fs::create_dir_all(self.artwork())?;
        Ok(())
    }
    pub fn database(&self) -> PathBuf {
        self.container.join("db.sqlite")
    }
    pub fn tracks(&self) -> PathBuf {
        self.container.join("tracks")
    }
    pub fn artwork(&self) -> PathBuf {
        self.container.join("artwork")
    }
    pub fn track_file(&self, full_id: &str) -> Result<PathBuf> {
        validate_full_id(full_id)?;
        Ok(self.tracks().join(format!("{full_id}.mp3")))
    }
    pub fn artwork_file(&self, url: &Url) -> PathBuf {
        self.artwork()
            .join(format!("{:x}.jpg", Sha1::digest(url.as_str().as_bytes())))
    }
    pub fn artwork_data(&self, url: &Url) -> Result<Option<Vec<u8>>> {
        match fs::read(self.artwork_file(url)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    pub fn store_artwork(&self, url: &Url, bytes: &[u8]) -> Result<PathBuf> {
        let file = self.artwork_file(url);
        atomic_write(&file, bytes)?;
        Ok(file)
    }
    pub fn size(&self) -> Result<u64> {
        directory_size(&self.container)
    }
}

fn directory_size(path: &Path) -> Result<u64> {
    let mut total: u64 = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            total = total.saturating_add(directory_size(&entry.path())?);
        } else if kind.is_file() {
            total = total.saturating_add(metadata.len());
        }
    }
    Ok(total)
}

fn validate_full_id(id: &str) -> Result<()> {
    let valid = id
        .split_once('_')
        .is_some_and(|(owner, audio)| owner.parse::<i64>().is_ok() && audio.parse::<i64>().is_ok());
    if !valid {
        return Err(Error::InvalidState("Invalid audio identifier".into()));
    }
    Ok(())
}

pub struct OfflineStore {
    connection: Mutex<Connection>,
    pub user_id: i64,
    pub paths: OfflinePaths,
}

impl OfflineStore {
    pub fn open(root: &Path, user_id: i64) -> Result<Self> {
        let paths = OfflinePaths::new(root, user_id);
        paths.prepare()?;
        let mut connection = Connection::open(paths.database())?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        match version {
            0 => {
                let tx = connection.transaction()?;
                tx.execute_batch(SCHEMA_V1)?;
                tx.commit()?;
            }
            1 => {}
            _ => {
                return Err(Error::InvalidState(format!(
                    "Unsupported offline database version {version}"
                )));
            }
        }
        Ok(Self {
            connection: Mutex::new(connection),
            user_id,
            paths,
        })
    }

    fn conn(&self) -> Result<MutexGuard<'_, Connection>> {
        self.connection
            .lock()
            .map_err(|_| Error::InvalidState("Offline database lock is poisoned".into()))
    }

    pub fn track(&self, full_id: &str) -> Result<Option<CachedTrack>> {
        find_track(&*self.conn()?, full_id)
    }

    pub fn tracks(&self) -> Result<Vec<CachedTrack>> {
        let conn = self.conn()?;
        let mut query =
            conn.prepare("SELECT * FROM cached_track ORDER BY priority, requested_at, full_id")?;
        Ok(query
            .query_map([], CachedTrack::from_row)?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn enqueue(
        &self,
        track: &Audio,
        request: &DownloadRequest,
        now: i64,
    ) -> Result<DownloadState> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let state = enqueue_track(&tx, track, request, now)?;
        tx.commit()?;
        Ok(state)
    }

    pub fn save_collection(&self, playlist: &Playlist, tracks: &[Audio], now: i64) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let metadata =
            serde_json::to_vec(playlist).map_err(|e| Error::decoding("download collection", e))?;
        let full_id = playlist.full_id();
        tx.execute("INSERT INTO download_collection (full_id, owner_id, collection_id, access_key, metadata, added_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(full_id) DO UPDATE SET owner_id = excluded.owner_id, collection_id = excluded.collection_id, access_key = excluded.access_key, metadata = excluded.metadata, added_at = excluded.added_at",
            rusqlite::params![full_id, playlist.owner_id, playlist.id, playlist.access_key, metadata, now])?;
        tx.execute(
            "DELETE FROM collection_track WHERE collection_full_id = ?1",
            [&full_id],
        )?;
        for (position, track) in tracks.iter().enumerate().filter(|(_, t)| t.is_playable()) {
            let request = DownloadRequest {
                priority: DownloadPriority::Manual,
                manual_pin: false,
                source: Some(DownloadSourceKind::Playlist),
                source_id: Some(full_id.clone()),
                source_offset: Some(position),
            };
            enqueue_track(&tx, track, &request, now)?;
            tx.execute("INSERT INTO collection_track (collection_full_id, track_full_id, position) VALUES (?1, ?2, ?3) ON CONFLICT(collection_full_id, track_full_id) DO UPDATE SET position = excluded.position",
                rusqlite::params![full_id, track.full_id(), position])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn collections(&self) -> Result<Vec<DownloadCollection>> {
        let conn = self.conn()?;
        let mut query = conn.prepare("SELECT full_id, owner_id, collection_id, access_key, metadata, artwork_file, added_at FROM download_collection ORDER BY added_at DESC")?;
        let rows = query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Vec<u8>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (full_id, owner_id, collection_id, access_key, metadata, artwork_file, added_at) =
                row?;
            let playlist = serde_json::from_slice(&metadata)
                .map_err(|e| Error::decoding("download collection", e))?;
            result.push(DownloadCollection {
                full_id,
                owner_id,
                collection_id,
                access_key,
                playlist,
                artwork_file,
                added_at,
            });
        }
        Ok(result)
    }

    pub fn collection_tracks(&self, full_id: &str) -> Result<Vec<Audio>> {
        let conn = self.conn()?;
        let mut query = conn.prepare("SELECT c.* FROM cached_track c JOIN collection_track ct ON ct.track_full_id = c.full_id WHERE ct.collection_full_id = ?1 ORDER BY ct.position")?;
        let records = query
            .query_map([full_id], CachedTrack::from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        records.iter().map(CachedTrack::audio).collect()
    }

    pub fn remove_collection(&self, full_id: &str) -> Result<()> {
        self.conn()?.execute(
            "DELETE FROM download_collection WHERE full_id = ?1",
            [full_id],
        )?;
        Ok(())
    }

    pub fn ready_tracks(&self) -> Result<Vec<Audio>> {
        let conn = self.conn()?;
        let mut query = conn.prepare("SELECT c.* FROM cached_track c WHERE c.state = 'ready' AND NOT EXISTS (SELECT 1 FROM collection_track ct WHERE ct.track_full_id = c.full_id) ORDER BY added_at DESC")?;
        let records = query
            .query_map([], CachedTrack::from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        records.iter().map(CachedTrack::audio).collect()
    }

    pub fn pending_tracks(&self) -> Result<Vec<CachedTrack>> {
        Ok(self
            .tracks()?
            .into_iter()
            .filter(|r| r.state != DownloadState::Ready)
            .collect())
    }

    pub fn local_file(&self, track: &Audio) -> Result<Option<PathBuf>> {
        let file = self.paths.track_file(&track.full_id())?;
        Ok(self
            .track(&track.full_id())?
            .filter(|r| r.state == DownloadState::Ready)
            .and_then(|_| file.is_file().then_some(file)))
    }

    pub fn mark_played(&self, full_id: &str, now: i64) -> Result<()> {
        self.conn()?.execute(
            "UPDATE cached_track SET last_played_at = ?1 WHERE full_id = ?2",
            rusqlite::params![now, full_id],
        )?;
        Ok(())
    }

    pub fn recover_interrupted(&self) -> Result<()> {
        self.conn()?.execute(
            "UPDATE cached_track SET state = 'queued' WHERE state = 'downloading'",
            [],
        )?;
        for record in self.tracks()? {
            if record.state == DownloadState::Ready
                && !self.paths.track_file(&record.full_id)?.is_file()
            {
                self.conn()?.execute(
                    "UPDATE cached_track SET state = 'queued', byte_size = 0 WHERE full_id = ?1",
                    [&record.full_id],
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn claim_next(&self, include_auto: bool) -> Result<Option<CachedTrack>> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let job = tx.query_row("SELECT * FROM cached_track WHERE state = 'queued' AND (priority = 0 OR ?1) ORDER BY priority, requested_at, full_id LIMIT 1", [include_auto], CachedTrack::from_row).optional()?;
        if let Some(job) = &job {
            tx.execute(
                "UPDATE cached_track SET state = 'downloading' WHERE full_id = ?1",
                [&job.full_id],
            )?;
        }
        tx.commit()?;
        Ok(job)
    }

    pub(crate) fn update_live_track(&self, track: &Audio, now: i64) -> Result<()> {
        let metadata = serde_json::to_vec(track).map_err(|e| Error::decoding("cached track", e))?;
        self.conn()?.execute("UPDATE cached_track SET metadata = ?1, access_key = ?2, url_fetched_at = ?3 WHERE full_id = ?4",
            rusqlite::params![metadata, track.access_key, now, track.full_id()])?;
        Ok(())
    }

    pub(crate) fn complete(
        &self,
        full_id: &str,
        byte_size: u64,
        artwork_file: Option<&str>,
    ) -> Result<()> {
        self.conn()?.execute("UPDATE cached_track SET state = 'ready', byte_size = ?1, artwork_file = ?2, attempts = 0, last_error = NULL WHERE full_id = ?3",
            rusqlite::params![byte_size, artwork_file, full_id])?;
        Ok(())
    }

    pub(crate) fn fail(&self, full_id: &str, attempts: u32, error: &str) -> Result<()> {
        self.conn()?.execute("UPDATE cached_track SET state = 'failed', attempts = ?1, last_error = ?2 WHERE full_id = ?3",
            rusqlite::params![attempts, error, full_id])?;
        Ok(())
    }

    pub fn remove(&self, full_id: &str) -> Result<()> {
        let file = self.paths.track_file(full_id)?;
        remove_file_if_present(&file)?;
        self.conn()?
            .execute("DELETE FROM cached_track WHERE full_id = ?1", [full_id])?;
        Ok(())
    }

    pub fn usage(&self) -> Result<CacheUsage> {
        let conn = self.conn()?;
        Ok(conn.query_row("SELECT COALESCE(SUM(byte_size), 0), COALESCE(SUM(CASE WHEN c.manual_pin = 1 OR EXISTS (SELECT 1 FROM collection_track ct WHERE ct.track_full_id = c.full_id) THEN byte_size ELSE 0 END), 0), COUNT(*) FROM cached_track c WHERE state = 'ready'",
            [], |row| Ok(CacheUsage { total: row.get(0)?, pinned: row.get(1)?, track_count: row.get(2)? }))?)
    }

    pub fn enforce_limit(&self, limit: u64, protected: &HashSet<String>) -> Result<Vec<String>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        self.evict(Some(limit), protected)
    }

    pub fn clear_auto_cache(&self, protected: &HashSet<String>) -> Result<Vec<String>> {
        self.evict(None, protected)
    }

    fn evict(&self, limit: Option<u64>, protected: &HashSet<String>) -> Result<Vec<String>> {
        let mut total = self.usage()?.total;
        let candidates = {
            let conn = self.conn()?;
            let mut query = conn.prepare("SELECT full_id, byte_size FROM cached_track c WHERE state = 'ready' AND manual_pin = 0 AND NOT EXISTS (SELECT 1 FROM collection_track ct WHERE ct.track_full_id = c.full_id) ORDER BY COALESCE(last_played_at, added_at), full_id")?;
            query
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut removed = Vec::new();
        for (id, bytes) in candidates {
            if limit.is_some_and(|limit| total <= limit) {
                break;
            }
            if protected.contains(&id) {
                continue;
            }
            self.remove(&id)?;
            total = total.saturating_sub(bytes);
            removed.push(id);
        }
        Ok(removed)
    }

    pub fn clear_all(&self) -> Result<()> {
        for record in self.tracks()? {
            self.remove(&record.full_id)?;
        }
        self.conn()?
            .execute("DELETE FROM download_collection", [])?;
        for entry in fs::read_dir(self.paths.artwork())? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }
}

fn remove_file_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn find_track(conn: &Connection, full_id: &str) -> Result<Option<CachedTrack>> {
    Ok(conn
        .query_row(
            "SELECT * FROM cached_track WHERE full_id = ?1",
            [full_id],
            CachedTrack::from_row,
        )
        .optional()?)
}

fn enqueue_track(
    conn: &Connection,
    track: &Audio,
    request: &DownloadRequest,
    now: i64,
) -> Result<DownloadState> {
    let mut record = if let Some(existing) = find_track(conn, &track.full_id())? {
        let mut updated = existing;
        updated.manual_pin |= request.manual_pin;
        updated.priority = updated.priority.min(request.priority);
        if track.stream_url().is_some() {
            updated.metadata =
                serde_json::to_vec(track).map_err(|e| Error::decoding("cached track", e))?;
            updated.access_key = track.access_key.clone();
            updated.url_fetched_at = Some(now);
        }
        if updated.state == DownloadState::Failed {
            updated.state = DownloadState::Queued;
            updated.attempts = 0;
            updated.last_error = None;
            updated.requested_at = now;
        }
        updated
    } else {
        CachedTrack::new(track, request, now)?
    };
    if request.source == Some(DownloadSourceKind::Playlist) {
        record.source_kind = request.source;
        record.source_id = request.source_id.clone();
        record.source_offset = request.source_offset;
    }
    save_track(conn, &record)?;
    Ok(record.state)
}

fn save_track(conn: &Connection, record: &CachedTrack) -> Result<()> {
    conn.execute(
        "INSERT INTO cached_track (full_id, owner_id, audio_id, access_key, title, artist, duration, album_title, artwork_url, artwork_file, metadata, state, manual_pin, byte_size, added_at, last_played_at, priority, requested_at, url_fetched_at, attempts, last_error, source_kind, source_id, source_offset) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24) ON CONFLICT(full_id) DO UPDATE SET owner_id = excluded.owner_id, audio_id = excluded.audio_id, access_key = excluded.access_key, title = excluded.title, artist = excluded.artist, duration = excluded.duration, album_title = excluded.album_title, artwork_url = excluded.artwork_url, artwork_file = excluded.artwork_file, metadata = excluded.metadata, state = excluded.state, manual_pin = excluded.manual_pin, byte_size = excluded.byte_size, added_at = excluded.added_at, last_played_at = excluded.last_played_at, priority = excluded.priority, requested_at = excluded.requested_at, url_fetched_at = excluded.url_fetched_at, attempts = excluded.attempts, last_error = excluded.last_error, source_kind = excluded.source_kind, source_id = excluded.source_id, source_offset = excluded.source_offset",
        rusqlite::params![record.full_id, record.owner_id, record.audio_id, record.access_key, record.title, record.artist, record.duration, record.album_title, record.artwork_url, record.artwork_file, record.metadata, record.state, record.manual_pin, record.byte_size, record.added_at, record.last_played_at, record.priority, record.requested_at, record.url_fetched_at, record.attempts, record.last_error, record.source_kind, record.source_id, record.source_offset],
    )?;
    Ok(())
}
