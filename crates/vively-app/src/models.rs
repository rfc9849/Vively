use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use url::Url;
use vively_core::{
    catalog::{ArtistEntry, MusicSection},
    lyrics::{SyncedLine, TrackLyrics},
    models::{
        ArtistInfo, Audio, Friend, MixCategory, MixOption, MixSettings, Playlist,
        ProfileSearchResults,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadState {
    Idle,
    Loading,
    Loaded,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct PlaylistDetailData {
    pub playlist: Playlist,
    pub tracks: Vec<Audio>,
    pub total_count: i64,
    pub owner: Option<Friend>,
    pub owner_audio_open: bool,
    pub loading_more: bool,
    pub more_failed: bool,
    pub state: LoadState,
}

#[derive(Debug, Clone)]
pub struct ArtistPageData {
    pub info: Option<ArtistInfo>,
    pub tracks: Vec<Audio>,
    pub playlists: Vec<Playlist>,
    pub sections: Vec<MusicSection>,
    pub related: Vec<ArtistInfo>,
    pub state: LoadState,
}

#[derive(Debug, Clone)]
pub struct ProfileSearchData {
    pub owner_id: i64,
    pub query: String,
    pub results: ProfileSearchResults,
    pub state: LoadState,
}

#[derive(Debug, Clone)]
pub struct CatalogPageData {
    pub title: String,
    pub sections: Vec<MusicSection>,
    pub next_from: Option<String>,
    pub loading_more: bool,
    pub more_failed: bool,
    pub state: LoadState,
}

#[derive(Debug, Clone)]
pub struct FriendMusicData {
    pub friend: Friend,
    pub tracks: Vec<Audio>,
    pub total_tracks: i64,
    pub playlists: Vec<Playlist>,
    pub total_playlists: i64,
    pub playlists_error: Option<String>,
    pub playlists_loading_more: bool,
    pub loading_more: bool,
    pub more_failed: bool,
    pub state: LoadState,
}

#[derive(Debug, Clone, Default)]
pub struct SearchResultsData {
    pub tracks: Vec<Audio>,
    pub artists: Vec<ArtistEntry>,
    pub playlists: Vec<Playlist>,
    pub sections: Vec<MusicSection>,
}

impl SearchResultsData {
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
            && self.artists.is_empty()
            && self.playlists.is_empty()
            && self.sections.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LyricsLoadState {
    Loading,
    Loaded(Option<TrackLyrics>),
    Failed(String),
}

pub fn active_synced_lyric_index(lines: &[SyncedLine], elapsed_ms: i64) -> Option<usize> {
    lines.iter().rposition(|line| elapsed_ms >= line.begin)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QrStage {
    Loading,
    Ready {
        url: String,
        size: usize,
        modules: Vec<bool>,
    },
    Scanned {
        url: String,
        size: usize,
        modules: Vec<bool>,
        user_name: Option<String>,
    },
    ConfirmCode {
        url: String,
    },
    Expired,
    Declined,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AuthStep {
    Phone,
    Qr(QrStage),
    Otp {
        method_label: String,
        subtitle: String,
        placeholder: String,
        resend_label: String,
        code_len: usize,
    },
    Password {
        had_otp: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadItemStatus {
    Pending,
    Uploading,
    Done,
    Failed(String),
}

/// UI-agnostic upload row; each front-end keeps its own editable inputs
/// for artist/title and syncs them in before submitting.
#[derive(Debug, Clone)]
pub struct UploadItem {
    pub path: PathBuf,
    pub file_name: String,
    pub artist: String,
    pub title: String,
    pub status: UploadItemStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionTarget {
    Id { section_id: String, title: String },
    Url { url: String, title: String },
}

impl SectionTarget {
    pub fn key(&self) -> String {
        match self {
            Self::Id { section_id, .. } => format!("id:{section_id}"),
            Self::Url { url, .. } => format!("url:{url}"),
        }
    }

    pub fn fallback_title(&self) -> &str {
        match self {
            Self::Id { title, .. } | Self::Url { title, .. } => title,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentQuickItem {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub artwork_url: Option<Url>,
    pub kind: RecentTargetKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum RecentTargetKind {
    Playlist(Playlist),
    Artist { id: String, name: String },
    Library,
}

pub fn default_mix_settings() -> MixSettings {
    fn opt(id: &str, title: &str) -> MixOption {
        MixOption {
            id: id.into(),
            title: title.into(),
            selected: None,
        }
    }
    fn cat(id: &str, title: &str, options: Vec<MixOption>) -> MixCategory {
        MixCategory {
            id: id.into(),
            title: title.into(),
            kind: None,
            options,
        }
    }
    MixSettings {
        title: "Настроить микс".into(),
        subtitle: None,
        multi_select: Some(true),
        mix_categories: vec![
            cat(
                "vibes",
                "Настроение",
                vec![
                    opt("active", "Бодрое"),
                    opt("calm", "Спокойное"),
                    opt("sad", "Грустное"),
                    opt("happy", "Весёлое"),
                    opt("love", "Романтичное"),
                ],
            ),
            cat(
                "character",
                "Характер",
                vec![
                    opt("unknown", "Незнакомое"),
                    opt("known", "Знакомое"),
                    opt("fresh", "Новинки"),
                ],
            ),
            cat(
                "langs",
                "Язык",
                vec![
                    opt("ru", "Русский"),
                    opt("international", "Иностранный"),
                    opt("instrumental", "Без слов"),
                ],
            ),
        ],
    }
}
