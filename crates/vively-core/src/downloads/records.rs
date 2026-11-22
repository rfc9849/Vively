use rusqlite::{
    ToSql,
    types::{FromSql, FromSqlError, ToSqlOutput, ValueRef},
};
use serde::{Deserialize, Serialize};

use crate::{
    Error, Result,
    models::{Audio, Playlist},
};

macro_rules! text_enum {
    ($name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name { $(#[serde(rename = $text)] $variant),+ }
        impl $name {
            pub fn as_str(self) -> &'static str { match self { $(Self::$variant => $text),+ } }
        }
        impl ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> { Ok(self.as_str().into()) }
        }
        impl FromSql for $name {
            fn column_result(value: ValueRef<'_>) -> std::result::Result<Self, FromSqlError> {
                match value.as_str()? { $($text => Ok(Self::$variant),)+ _ => Err(FromSqlError::InvalidType) }
            }
        }
    };
}

text_enum!(DownloadState { Queued => "queued", Downloading => "downloading", Ready => "ready", Failed => "failed" });
text_enum!(DownloadSourceKind { Library => "library", Playlist => "playlist", Track => "track" });

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DownloadPriority {
    Manual = 0,
    Auto = 1,
}

impl ToSql for DownloadPriority {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok((*self as i64).into())
    }
}

impl FromSql for DownloadPriority {
    fn column_result(value: ValueRef<'_>) -> std::result::Result<Self, FromSqlError> {
        match value.as_i64()? {
            0 => Ok(Self::Manual),
            1 => Ok(Self::Auto),
            _ => Err(FromSqlError::OutOfRange(value.as_i64()?)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub priority: DownloadPriority,
    pub manual_pin: bool,
    pub source: Option<DownloadSourceKind>,
    pub source_id: Option<String>,
    pub source_offset: Option<usize>,
}

impl Default for DownloadRequest {
    fn default() -> Self {
        Self {
            priority: DownloadPriority::Manual,
            manual_pin: true,
            source: Some(DownloadSourceKind::Track),
            source_id: None,
            source_offset: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachedTrack {
    pub full_id: String,
    pub owner_id: i64,
    pub audio_id: i64,
    pub access_key: Option<String>,
    pub title: String,
    pub artist: String,
    pub duration: i64,
    pub album_title: Option<String>,
    pub artwork_url: Option<String>,
    pub artwork_file: Option<String>,
    pub metadata: Vec<u8>,
    pub state: DownloadState,
    pub manual_pin: bool,
    pub byte_size: u64,
    pub added_at: i64,
    pub last_played_at: Option<i64>,
    pub priority: DownloadPriority,
    pub requested_at: i64,
    pub url_fetched_at: Option<i64>,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub source_kind: Option<DownloadSourceKind>,
    pub source_id: Option<String>,
    pub source_offset: Option<usize>,
}

impl CachedTrack {
    pub fn new(track: &Audio, request: &DownloadRequest, now: i64) -> Result<Self> {
        Ok(Self {
            full_id: track.full_id(),
            owner_id: track.owner_id,
            audio_id: track.id,
            access_key: track.access_key.clone(),
            title: track.title.clone(),
            artist: track.artist.clone(),
            duration: track.duration,
            album_title: track.album.as_ref().and_then(|a| a.title.clone()),
            artwork_url: track.artwork_url().map(ToString::to_string),
            artwork_file: None,
            metadata: serde_json::to_vec(track).map_err(|e| Error::decoding("cached track", e))?,
            state: DownloadState::Queued,
            manual_pin: request.manual_pin,
            byte_size: 0,
            added_at: now,
            last_played_at: None,
            priority: request.priority,
            requested_at: now,
            url_fetched_at: track.stream_url().map(|_| now),
            attempts: 0,
            last_error: None,
            source_kind: request.source,
            source_id: request.source_id.clone(),
            source_offset: request.source_offset,
        })
    }

    pub fn audio(&self) -> Result<Audio> {
        serde_json::from_slice(&self.metadata).map_err(|e| Error::decoding("cached track", e))
    }

    pub fn url_is_fresh(&self, now: i64) -> bool {
        self.url_fetched_at
            .is_some_and(|fetched| now >= fetched && now - fetched < 30 * 60)
    }

    pub(crate) fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            full_id: row.get("full_id")?,
            owner_id: row.get("owner_id")?,
            audio_id: row.get("audio_id")?,
            access_key: row.get("access_key")?,
            title: row.get("title")?,
            artist: row.get("artist")?,
            duration: row.get("duration")?,
            album_title: row.get("album_title")?,
            artwork_url: row.get("artwork_url")?,
            artwork_file: row.get("artwork_file")?,
            metadata: row.get("metadata")?,
            state: row.get("state")?,
            manual_pin: row.get("manual_pin")?,
            byte_size: row.get("byte_size")?,
            added_at: row.get("added_at")?,
            last_played_at: row.get("last_played_at")?,
            priority: row.get("priority")?,
            requested_at: row.get("requested_at")?,
            url_fetched_at: row.get("url_fetched_at")?,
            attempts: row.get("attempts")?,
            last_error: row.get("last_error")?,
            source_kind: row.get("source_kind")?,
            source_id: row.get("source_id")?,
            source_offset: row.get("source_offset")?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DownloadCollection {
    pub full_id: String,
    pub owner_id: i64,
    pub collection_id: i64,
    pub access_key: Option<String>,
    pub playlist: Playlist,
    pub artwork_file: Option<String>,
    pub added_at: i64,
}

impl DownloadCollection {
    pub fn new(playlist: &Playlist, now: i64) -> Self {
        Self {
            full_id: playlist.full_id(),
            owner_id: playlist.owner_id,
            collection_id: playlist.id,
            access_key: playlist.access_key.clone(),
            playlist: playlist.clone(),
            artwork_file: None,
            added_at: now,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectionTrack {
    pub collection_full_id: String,
    pub track_full_id: String,
    pub position: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheUsage {
    pub total: u64,
    pub pinned: u64,
    pub track_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkInterface {
    Wifi,
    Cellular,
    Wired,
    Other,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NetworkStatus {
    #[default]
    Unknown,
    Offline,
    Online(NetworkInterface),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NetworkState {
    pub status: NetworkStatus,
    pub expensive: bool,
    pub constrained: bool,
}

impl NetworkState {
    pub fn is_online(self) -> bool {
        matches!(self.status, NetworkStatus::Online(_))
    }
    pub fn allows_unmetered_transfer(self) -> bool {
        self.status == NetworkStatus::Online(NetworkInterface::Wifi)
            && !self.expensive
            && !self.constrained
    }
}
