//! Snapshots and commands for the full Android UI: home, search, playlists,
//! artist, friends, downloads, lyrics, settings and queue editing.

use vively_app::{
    AppCore,
    models::{LyricsLoadState, RecentTargetKind, active_synced_lyric_index},
};
use vively_core::{
    catalog::{ArtistEntry, HomeBlockKind, MusicContent, MusicSection, TrackStyle},
    downloads::DownloadState,
    lyrics::{LyricsContent, LyricsSource},
    models::{ArtistInfo, Audio, Friend, Playlist},
    playback::PlaybackOrigin,
    settings::Appearance,
};

use crate::{FfiLoad, FfiTrack, VivelyCore, dto::TrackCtx};

// ---------------------------------------------------------------- records

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiPlaylist {
    /// Stable id; pass back to playlist commands.
    pub key: String,
    pub full_id: String,
    pub owner_id: i64,
    pub title: String,
    pub byline: String,
    pub count: i64,
    pub artwork_url: Option<String>,
    pub is_album: bool,
    pub editable: bool,
    pub following: bool,
    pub downloaded: bool,
    pub description: Option<String>,
    /// Not shown in search and recommendations.
    pub hidden: bool,
    pub can_copy: bool,
    pub year: Option<i64>,
    pub genre: Option<String>,
    pub plays: i64,
    /// Accent colour ("#rrggbb") for the hero banner.
    pub tint_hex: Option<String>,
    pub artist_id: Option<String>,
    pub artist_name: Option<String>,
}

impl FfiPlaylist {
    pub(crate) fn from_playlist(core: &AppCore, p: &Playlist) -> Self {
        let full_id = p.full_id();
        let following = p.is_following.unwrap_or(false)
            || core.playlists.iter().any(|x| x.full_id() == full_id);
        let main = p.main_artists.as_ref().and_then(|a| a.first());
        Self {
            key: p.stable_id(),
            following,
            downloaded: core.is_collection_downloaded(p),
            full_id,
            owner_id: p.owner_id,
            title: p.title.clone(),
            byline: p.byline(),
            count: p.count,
            artwork_url: p.artwork_url().map(ToString::to_string),
            is_album: p.is_album(),
            editable: p.is_editable(),
            description: p.description.clone().filter(|d| !d.trim().is_empty()),
            hidden: p.no_discover == Some(true),
            can_copy: p
                .permissions
                .as_ref()
                .and_then(|x| x.save_as_copy)
                .unwrap_or(false),
            year: p.year,
            genre: p.genres.as_ref().and_then(|g| g.first()).map(|g| g.name.clone()),
            plays: p.plays.unwrap_or(0),
            tint_hex: p.main_color.clone().filter(|h| !h.trim().is_empty()),
            artist_id: main.map(|a| a.id.clone()).filter(|id| !id.is_empty()),
            artist_name: p.artist_line().or_else(|| p.author_name().map(str::to_owned)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiSection {
    pub title: String,
    pub subtitle: Option<String>,
    /// Catalog section id for "Показать все" (`open_section`).
    pub show_all_id: Option<String>,
    /// Tracks laid out in columns on desktop; a compact grid on phones.
    pub columns: bool,
    pub tracks: Vec<FfiTrack>,
    pub playlists: Vec<FfiPlaylist>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiArtistRef {
    pub id: String,
    pub name: String,
    pub photo_url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct FfiSearch {
    pub query: String,
    pub state: FfiLoad,
    pub history: Vec<String>,
    pub tracks: Vec<FfiTrack>,
    pub artists: Vec<FfiArtistRef>,
    pub playlists: Vec<FfiPlaylist>,
    pub sections: Vec<FfiSection>,
}

#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct FfiPlaylists {
    pub state: FfiLoad,
    pub items: Vec<FfiPlaylist>,
    /// Add-to-playlist picker: full ids being added / already added.
    pub adding: Vec<String>,
    pub added: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiPlaylistDetail {
    pub playlist: FfiPlaylist,
    pub state: FfiLoad,
    pub tracks: Vec<FfiTrack>,
    pub total: i64,
    pub loading_more: bool,
    pub more_failed: bool,
    pub downloaded: bool,
    /// Owner whose music is open; `open_playlist_owner` navigates there.
    pub owner_music_id: Option<i64>,
    pub total_secs: i64,
}

#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct FfiArtist {
    pub id: String,
    pub name: String,
    pub photo_url: Option<String>,
    pub followed: bool,
    pub state: FfiLoad,
    pub tracks: Vec<FfiTrack>,
    pub playlists: Vec<FfiPlaylist>,
    pub sections: Vec<FfiSection>,
    pub related: Vec<FfiArtistRef>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiFriend {
    pub id: i64,
    pub name: String,
    pub photo_url: Option<String>,
    /// Deactivated or private profile without access.
    pub closed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct FfiFriends {
    pub state: FfiLoad,
    pub total: i64,
    pub items: Vec<FfiFriend>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiFriendMusic {
    pub friend: FfiFriend,
    pub state: FfiLoad,
    pub tracks: Vec<FfiTrack>,
    pub total_tracks: i64,
    pub playlists: Vec<FfiPlaylist>,
    pub total_playlists: i64,
    pub playlists_error: Option<String>,
    pub loading_more: bool,
    pub following: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiDownloadState {
    Queued,
    Downloading,
    Ready,
    Failed,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiPendingDownload {
    pub full_id: String,
    pub title: String,
    pub artist: String,
    pub artwork_url: Option<String>,
    pub state: FfiDownloadState,
    pub progress: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct FfiDownloads {
    pub ready: Vec<FfiTrack>,
    pub pending: Vec<FfiPendingDownload>,
    pub collections: Vec<FfiPlaylist>,
    pub total_bytes: i64,
    pub pinned_bytes: i64,
    pub track_count: i64,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiLyricLine {
    /// -1 for unsynced lyrics.
    pub begin_ms: i64,
    /// `None` = instrumental break.
    pub text: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct FfiLyrics {
    pub state: FfiLoad,
    pub synced: bool,
    pub source: Option<String>,
    pub lines: Vec<FfiLyricLine>,
    pub active: Option<i64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, uniffi::Enum)]
pub enum FfiAppearance {
    #[default]
    System,
    Light,
    Dark,
}

impl From<Appearance> for FfiAppearance {
    fn from(a: Appearance) -> Self {
        match a {
            Appearance::System => Self::System,
            Appearance::Light => Self::Light,
            Appearance::Dark => Self::Dark,
        }
    }
}

impl From<FfiAppearance> for Appearance {
    fn from(a: FfiAppearance) -> Self {
        match a {
            FfiAppearance::System => Self::System,
            FfiAppearance::Light => Self::Light,
            FfiAppearance::Dark => Self::Dark,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct FfiSettings {
    pub appearance: FfiAppearance,
    pub crossfade_secs: f64,
    pub broadcast: bool,
    pub reporting: bool,
    pub genius_fallback: bool,
    pub auto_cache: bool,
    pub wifi_only: bool,
    pub cache_limit_bytes: i64,
    pub volume: f32,
    pub muted: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum FfiRecentKind {
    Playlist { key: String },
    Artist { id: String, name: String },
    Library,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiRecent {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub artwork_url: Option<String>,
    pub kind: FfiRecentKind,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiMixOption {
    pub id: String,
    pub title: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiMixCategory {
    pub id: String,
    pub title: String,
    pub options: Vec<FfiMixOption>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiLink {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub artwork_url: Option<String>,
    pub is_library: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiRecommended {
    pub playlist: FfiPlaylist,
    pub match_label: String,
    pub color_hex: Option<String>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiMixTile {
    pub id: String,
    pub title: String,
    pub image_url: Option<String>,
    pub mix_id: String,
    pub options_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiArtistMix {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub photo_url: Option<String>,
    pub mix_id: String,
    pub entity_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum FfiHomeBlock {
    Section {
        section: FfiSection,
    },
    Recommended {
        title: String,
        show_all_id: Option<String>,
        items: Vec<FfiRecommended>,
    },
    MixTiles {
        title: String,
        tiles: Vec<FfiMixTile>,
    },
    ArtistMixes {
        title: String,
        cards: Vec<FfiArtistMix>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct FfiHome {
    pub state: FfiLoad,
    pub mix_title: String,
    pub mix_id: String,
    /// The home mix is the current queue.
    pub mix_active: bool,
    pub mix_playing: bool,
    pub mix_categories: Vec<FfiMixCategory>,
    pub has_picks: bool,
    pub links: Vec<FfiLink>,
    pub recents: Vec<FfiRecent>,
    pub blocks: Vec<FfiHomeBlock>,
}

// ---------------------------------------------------------------- builders

fn section_tracks(s: &MusicSection) -> &[Audio] {
    match &s.content {
        MusicContent::Tracks(t) => t,
        MusicContent::Playlists(_) => &[],
    }
}

fn section_playlists(s: &MusicSection) -> &[Playlist] {
    match &s.content {
        MusicContent::Playlists(p) => p,
        MusicContent::Tracks(_) => &[],
    }
}

pub(crate) fn tracks(core: &AppCore, list: &[Audio]) -> Vec<FfiTrack> {
    let ctx = TrackCtx::new(core);
    list.iter()
        .map(|t| FfiTrack::from_audio(core, &ctx, t))
        .collect()
}

pub(crate) fn playlists(core: &AppCore, list: &[Playlist]) -> Vec<FfiPlaylist> {
    list.iter()
        .map(|p| FfiPlaylist::from_playlist(core, p))
        .collect()
}

pub(crate) fn section(core: &AppCore, s: &MusicSection) -> FfiSection {
    FfiSection {
        title: s.title.clone(),
        subtitle: s.subtitle.clone(),
        show_all_id: s.show_all_section_id.clone().filter(|id| !id.is_empty()),
        columns: s.track_style == TrackStyle::Columns,
        tracks: tracks(core, section_tracks(s)),
        playlists: playlists(core, section_playlists(s)),
    }
}

fn artist_entry(a: &ArtistEntry) -> FfiArtistRef {
    FfiArtistRef {
        id: a.id.clone(),
        name: a.name.clone(),
        photo_url: a.avatar.as_ref().map(ToString::to_string),
    }
}

fn artist_info(a: &ArtistInfo) -> FfiArtistRef {
    FfiArtistRef {
        id: a.id.clone(),
        name: a.name.clone(),
        photo_url: a.best_photo().map(ToString::to_string),
    }
}

fn friend(f: &Friend) -> FfiFriend {
    FfiFriend {
        id: f.id,
        name: format!("{} {}", f.first_name, f.last_name).trim().to_owned(),
        photo_url: f.photo_base.as_ref().map(ToString::to_string),
        closed: f.deactivated.is_some()
            || (f.is_closed == Some(true) && f.can_access_closed != Some(true)),
    }
}

/// Any track the UI can currently see, by full id.
pub(crate) fn find_track(core: &AppCore, full_id: &str) -> Option<Audio> {
    let mut slices: Vec<&[Audio]> = vec![
        core.queue.tracks(),
        core.library_tracks.as_slice(),
        core.search_results.tracks.as_slice(),
        core.ready_downloads.as_slice(),
        core.editor_draft_tracks.as_slice(),
        core.editor_picker_tracks.as_slice(),
        core.editor_search_tracks.as_slice(),
        core.restorable_own_track.as_slice(),
    ];
    for p in core.catalog_pages.values() {
        slices.extend(p.sections.iter().map(section_tracks));
    }
    slices.extend(core.search_results.sections.iter().map(section_tracks));
    slices.extend(core.playlist_details.values().map(|d| d.tracks.as_slice()));
    for a in core.artist_pages.values() {
        slices.push(a.tracks.as_slice());
        slices.extend(a.sections.iter().map(section_tracks));
    }
    slices.extend(core.friend_pages.values().map(|f| f.tracks.as_slice()));
    if let Some(ps) = &core.profile_search {
        slices.push(ps.results.audios.as_slice());
    }
    for b in &core.home_blocks {
        if let HomeBlockKind::Music(s) = &b.kind {
            slices.push(section_tracks(s));
        }
    }
    slices
        .into_iter()
        .flatten()
        .find(|t| t.full_id() == full_id)
        .cloned()
}

/// Any playlist the UI can currently see, by stable or full id.
pub(crate) fn find_playlist(core: &AppCore, id: &str) -> Option<Playlist> {
    let hit = |p: &&Playlist| p.stable_id() == id || p.full_id() == id;
    let mut slices: Vec<&[Playlist]> = vec![
        core.playlists.as_slice(),
        core.search_results.playlists.as_slice(),
        core.editor_picker_playlists.as_slice(),
    ];
    for p in core.catalog_pages.values() {
        slices.extend(p.sections.iter().map(section_playlists));
    }
    slices.extend(core.search_results.sections.iter().map(section_playlists));
    for a in core.artist_pages.values() {
        slices.push(a.playlists.as_slice());
        slices.extend(a.sections.iter().map(section_playlists));
    }
    slices.extend(core.friend_pages.values().map(|f| f.playlists.as_slice()));
    if let Some(ps) = &core.profile_search {
        slices.push(ps.results.playlists.items.as_slice());
    }
    for b in &core.home_blocks {
        if let HomeBlockKind::Music(s) = &b.kind {
            slices.push(section_playlists(s));
        }
    }
    let found = core
        .playlist_details
        .values()
        .map(|d| &d.playlist)
        .chain(slices.into_iter().flatten())
        .chain(core.offline_collections.iter().map(|c| &c.playlist))
        .chain(core.recent_items.iter().filter_map(|r| match &r.kind {
            RecentTargetKind::Playlist(p) => Some(p),
            _ => None,
        }))
        .find(hit)
        .cloned();
    found.or_else(|| {
        core.home_blocks.iter().find_map(|b| match &b.kind {
            HomeBlockKind::Recommended { playlists, .. } => playlists
                .iter()
                .map(|rp| rp.as_playlist())
                .find(|p| hit(&p)),
            _ => None,
        })
    })
}

fn detail(core: &AppCore, key: &str) -> Option<FfiPlaylistDetail> {
    let d = core.playlist_details.get(key).or_else(|| {
        let p = find_playlist(core, key)?;
        core.playlist_details.get(&p.stable_id())
    })?;
    Some(FfiPlaylistDetail {
        playlist: FfiPlaylist::from_playlist(core, &d.playlist),
        state: (&d.state).into(),
        tracks: tracks(core, &d.tracks),
        total: d.total_count,
        loading_more: d.loading_more,
        more_failed: d.more_failed,
        downloaded: core.is_collection_downloaded(&d.playlist),
        owner_music_id: d
            .owner
            .as_ref()
            .filter(|_| d.owner_audio_open)
            .map(|o| o.id),
        total_secs: d.tracks.iter().map(|t| t.duration.max(0)).sum(),
    })
}

fn home(core: &AppCore) -> FfiHome {
    let mut blocks = Vec::new();
    let mut links = Vec::new();
    for b in &core.home_blocks {
        match &b.kind {
            HomeBlockKind::Music(s) => blocks.push(FfiHomeBlock::Section {
                section: section(core, s),
            }),
            HomeBlockKind::Recommended {
                title,
                show_all_section_id,
                playlists,
            } => {
                if playlists.is_empty() {
                    continue;
                }
                let items = playlists
                    .iter()
                    .map(|rp| FfiRecommended {
                        playlist: FfiPlaylist::from_playlist(core, &rp.as_playlist()),
                        match_label: rp
                            .match_text()
                            .map(|pct| format!("Совпадение {pct}"))
                            .or_else(|| rp.percentage_title.clone())
                            .unwrap_or_else(|| "Рекомендация".into()),
                        color_hex: rp.color.clone(),
                    })
                    .collect();
                blocks.push(FfiHomeBlock::Recommended {
                    title: title.clone(),
                    show_all_id: show_all_section_id.clone().filter(|s| !s.is_empty()),
                    items,
                });
            }
            HomeBlockKind::MixTiles { title, tiles } => {
                if tiles.is_empty() {
                    continue;
                }
                blocks.push(FfiHomeBlock::MixTiles {
                    title: title.clone(),
                    tiles: tiles
                        .iter()
                        .map(|t| FfiMixTile {
                            id: t.id.clone(),
                            title: t.title.clone(),
                            image_url: t.image.as_ref().map(ToString::to_string),
                            mix_id: t.mix_id.clone(),
                            options_json: t.options_json.clone(),
                        })
                        .collect(),
                });
            }
            HomeBlockKind::ArtistMixes { title, cards } => {
                if cards.is_empty() {
                    continue;
                }
                blocks.push(FfiHomeBlock::ArtistMixes {
                    title: title.clone(),
                    cards: cards
                        .iter()
                        .map(|c| FfiArtistMix {
                            id: c.id.clone(),
                            title: c.title.clone(),
                            description: c.description.clone(),
                            photo_url: c.photo.as_ref().map(ToString::to_string),
                            mix_id: c.mix_id.clone(),
                            entity_id: c.entity_id.clone(),
                        })
                        .collect(),
                });
            }
            HomeBlockKind::Links(list) => {
                if links.is_empty() {
                    links = list
                        .iter()
                        .take(4)
                        .map(|l| FfiLink {
                            id: l.id.clone(),
                            title: l.title.clone(),
                            subtitle: l.subtitle.clone().filter(|s| !s.is_empty()),
                            artwork_url: l.artwork_url().map(ToString::to_string),
                            is_library: l.is_my_audios(),
                        })
                        .collect();
                }
            }
            HomeBlockKind::StreamMixes(_) => {}
        }
    }
    let recents = core
        .recent_items
        .iter()
        .map(|r| FfiRecent {
            id: r.id.clone(),
            title: r.title.clone(),
            subtitle: r.subtitle.clone(),
            artwork_url: r.artwork_url.as_ref().map(ToString::to_string),
            kind: match &r.kind {
                RecentTargetKind::Playlist(p) => FfiRecentKind::Playlist { key: p.stable_id() },
                RecentTargetKind::Artist { id, name } => FfiRecentKind::Artist {
                    id: id.clone(),
                    name: name.clone(),
                },
                RecentTargetKind::Library => FfiRecentKind::Library,
            },
        })
        .collect();
    let mix_categories = core
        .mix_settings
        .mix_categories
        .iter()
        .map(|cat| {
            let picked = core.mix_picks.get(&cat.id);
            FfiMixCategory {
                id: cat.id.clone(),
                title: cat.title.clone(),
                options: cat
                    .options
                    .iter()
                    .map(|o| FfiMixOption {
                        id: o.id.clone(),
                        title: o.title.clone(),
                        selected: picked.is_some_and(|v| v.contains(&o.id)),
                    })
                    .collect(),
            }
        })
        .collect();
    let mix_active = core.active_mix_session.is_some() && core.queue.current().is_some();
    FfiHome {
        state: (&core.home_state).into(),
        mix_title: core
            .primary_mix
            .as_ref()
            .map(|m| m.display_title().to_owned())
            .unwrap_or_else(|| "VK Микс".into()),
        mix_id: core
            .primary_mix
            .as_ref()
            .map(|m| m.id.clone())
            .unwrap_or_else(|| "common".into()),
        mix_active,
        mix_playing: mix_active && core.audio_engine.is_playing(),
        mix_categories,
        has_picks: core.mix_picks.values().any(|v| !v.is_empty()),
        links,
        recents,
        blocks,
    }
}

fn lyrics(core: &AppCore) -> FfiLyrics {
    let Some(cur) = core.queue.current() else {
        return FfiLyrics::default();
    };
    match core.lyrics_states.get(&cur.stable_id()) {
        None => FfiLyrics::default(),
        Some(LyricsLoadState::Loading) => FfiLyrics {
            state: FfiLoad::Loading,
            ..Default::default()
        },
        Some(LyricsLoadState::Failed(m)) => FfiLyrics {
            state: FfiLoad::Failed { message: m.clone() },
            ..Default::default()
        },
        Some(LyricsLoadState::Loaded(None)) => FfiLyrics {
            state: FfiLoad::Loaded,
            ..Default::default()
        },
        Some(LyricsLoadState::Loaded(Some(l))) => {
            let source = Some(
                match l.source {
                    LyricsSource::Vk => "VK",
                    LyricsSource::Musixmatch => "Musixmatch",
                    LyricsSource::Genius => "Genius",
                }
                .to_owned(),
            );
            match &l.content {
                LyricsContent::Synced(lines) => {
                    let ms = (core.playback_position() * 1000.0).round() as i64;
                    FfiLyrics {
                        state: FfiLoad::Loaded,
                        synced: true,
                        source,
                        active: active_synced_lyric_index(lines, ms).map(|i| i as i64),
                        lines: lines
                            .iter()
                            .map(|line| FfiLyricLine {
                                begin_ms: line.begin,
                                text: line.text.clone(),
                            })
                            .collect(),
                    }
                }
                LyricsContent::Plain(lines) => FfiLyrics {
                    state: FfiLoad::Loaded,
                    synced: false,
                    source,
                    active: None,
                    lines: lines
                        .iter()
                        .map(|t| FfiLyricLine {
                            begin_ms: -1,
                            text: Some(t.clone()),
                        })
                        .collect(),
                },
            }
        }
    }
}

fn download_state(s: DownloadState) -> FfiDownloadState {
    #[allow(unreachable_patterns)]
    match s {
        DownloadState::Queued => FfiDownloadState::Queued,
        DownloadState::Downloading => FfiDownloadState::Downloading,
        DownloadState::Ready => FfiDownloadState::Ready,
        DownloadState::Failed => FfiDownloadState::Failed,
        _ => FfiDownloadState::Queued,
    }
}

// ---------------------------------------------------------------- exports

#[uniffi::export]
impl VivelyCore {
    // ---- home

    pub fn home(&self) -> FfiHome {
        self.query(|c| home(c)).unwrap_or_default()
    }

    pub fn load_home(&self, force: bool) {
        self.run(move |c| c.load_home(force));
    }

    pub fn start_home_mix(&self) {
        self.run(|c| c.toggle_or_start_home_mix());
    }

    pub fn start_track_mix(&self, full_id: String) {
        self.with_track(full_id, |c, t| c.start_track_mix(t));
    }

    pub fn toggle_dislike(&self, full_id: String) {
        self.with_track(full_id, |c, t| c.toggle_dislike_track(t));
    }

    pub fn restore_removed_track(&self) {
        self.run(|c| c.restore_own_removed_track());
    }

    /// Play `full_ids` as a queue starting at `start_index`.
    pub fn play_tracks(&self, full_ids: Vec<String>, start_index: u32, label: String) {
        self.run(move |c| {
            let start_id = full_ids.get(start_index as usize).cloned();
            let list: Vec<Audio> = full_ids.iter().filter_map(|id| find_track(c, id)).collect();
            let start = start_id
                .and_then(|id| list.iter().position(|t| t.full_id() == id))
                .unwrap_or(0);
            c.play_tracks(list, start, PlaybackOrigin::Other, label, None);
        });
    }

    // ---- search

    pub fn search(&self) -> FfiSearch {
        self.query(|c| {
            let r = &c.search_results;
            FfiSearch {
                query: c.search_query.clone(),
                state: (&c.search_state).into(),
                history: c.search_history.entries.clone(),
                tracks: tracks(c, &r.tracks),
                artists: r.artists.iter().map(artist_entry).collect(),
                playlists: playlists(c, &r.playlists),
                sections: r.sections.iter().map(|s| section(c, s)).collect(),
            }
        })
        .unwrap_or_default()
    }

    /// Live typing (debounced).
    pub fn edit_search(&self, query: String) {
        self.run(move |c| c.edit_search_query(query));
    }

    /// Enter / search button.
    pub fn commit_search(&self, query: String) {
        self.run(move |c| c.commit_search(query));
    }

    pub fn pick_search_history(&self, entry: String) {
        self.run(move |c| c.pick_search(entry));
    }

    pub fn clear_search(&self) {
        self.run(|c| c.clear_search());
    }

    pub fn delete_search_history(&self, entry: String) {
        self.run(move |c| c.delete_search_history(&entry));
    }

    pub fn clear_search_history(&self) {
        self.run(|c| c.clear_search_history());
    }

    // ---- playlists

    pub fn playlists(&self) -> FfiPlaylists {
        self.query(|c| FfiPlaylists {
            state: (&c.playlists_state).into(),
            items: playlists(c, &c.playlists),
            adding: c.add_pl_busy.iter().cloned().collect(),
            added: c.add_pl_added.iter().cloned().collect(),
        })
        .unwrap_or_default()
    }

    pub fn load_playlists(&self, force: bool) {
        self.run(move |c| c.load_playlists(force));
    }

    pub fn open_playlist(&self, key: String) {
        self.run(move |c| {
            if let Some(p) = find_playlist(c, &key) {
                c.record_recent_playlist(&p);
                c.ensure_playlist_entry(&p);
                c.load_playlist_detail(p, false);
            }
        });
    }

    pub fn reload_playlist(&self, key: String) {
        self.run(move |c| {
            if let Some(p) = find_playlist(c, &key) {
                c.load_playlist_detail(p, true);
            }
        });
    }

    pub fn playlist_detail(&self, key: String) -> Option<FfiPlaylistDetail> {
        self.query(move |c| detail(c, &key)).flatten()
    }

    pub fn load_more_playlist_tracks(&self, key: String) {
        self.run(move |c| c.load_more_playlist_tracks(&key));
    }

    pub fn play_playlist(&self, key: String, offline_only: bool) {
        self.run(move |c| {
            if let Some(p) = find_playlist(c, &key) {
                c.play_playlist_immediately(p, offline_only);
            }
        });
    }

    pub fn toggle_follow_playlist(&self, key: String) {
        self.run(move |c| {
            if let Some(p) = find_playlist(c, &key) {
                c.toggle_follow_playlist(p);
            }
        });
    }

    pub fn delete_playlist(&self, key: String) {
        self.run(move |c| {
            if let Some(p) = find_playlist(c, &key) {
                c.delete_own_playlist(p);
            }
        });
    }

    pub fn save_playlist_copy(&self, key: String) {
        self.run(move |c| {
            if let Some(p) = find_playlist(c, &key) {
                c.save_playlist_copy(p);
            }
        });
    }

    // ---- artist

    pub fn open_artist(&self, id: String) {
        self.run(move |c| c.load_artist(&id, false));
    }

    pub fn reload_artist(&self, id: String) {
        self.run(move |c| c.load_artist(&id, true));
    }

    pub fn artist(&self, id: String) -> FfiArtist {
        self.query(move |c| {
            let Some(page) = c.artist_pages.get(&id) else {
                return FfiArtist {
                    id,
                    ..Default::default()
                };
            };
            let info = page.info.as_ref();
            FfiArtist {
                name: info.map(|i| i.name.clone()).unwrap_or_default(),
                photo_url: info.and_then(|i| i.best_photo()).map(ToString::to_string),
                followed: info.and_then(|i| i.is_followed).unwrap_or(false),
                state: (&page.state).into(),
                tracks: tracks(c, &page.tracks),
                playlists: playlists(c, &page.playlists),
                sections: page.sections.iter().map(|s| section(c, s)).collect(),
                related: page.related.iter().map(artist_info).collect(),
                id,
            }
        })
        .unwrap_or_default()
    }

    pub fn toggle_artist_follow(&self, id: String) {
        self.run(move |c| c.toggle_artist_follow(id));
    }

    // ---- friends

    pub fn friends(&self) -> FfiFriends {
        self.query(|c| FfiFriends {
            state: (&c.friends_state).into(),
            total: c.friends_total,
            items: c.friends.iter().map(friend).collect(),
        })
        .unwrap_or_default()
    }

    pub fn load_friends(&self, force: bool) {
        self.run(move |c| c.load_friends(force));
    }

    pub fn open_friend(&self, id: i64, force: bool) {
        self.run(move |c| {
            if let Some(f) = c.friend_profile(id) {
                c.load_friend_music(f, force);
            }
        });
    }

    pub fn friend_music(&self, id: i64) -> Option<FfiFriendMusic> {
        self.query(move |c| {
            let page = c.friend_pages.get(&id)?;
            Some(FfiFriendMusic {
                friend: friend(&page.friend),
                state: (&page.state).into(),
                tracks: tracks(c, &page.tracks),
                total_tracks: page.total_tracks,
                playlists: playlists(c, &page.playlists),
                total_playlists: page.total_playlists,
                playlists_error: page.playlists_error.clone(),
                loading_more: page.loading_more || page.playlists_loading_more,
                following: c.followed_owners.contains(&id),
            })
        })
        .flatten()
    }

    pub fn load_more_friend_tracks(&self, id: i64) {
        self.run(move |c| c.load_more_friend_tracks(id));
    }

    pub fn load_more_friend_playlists(&self, id: i64) {
        self.run(move |c| c.load_more_friend_playlists(id));
    }

    pub fn toggle_owner_follow(&self, id: i64) {
        self.run(move |c| c.toggle_owner_follow(id));
    }

    // ---- downloads

    pub fn downloads(&self) -> FfiDownloads {
        self.query(|c| FfiDownloads {
            ready: tracks(c, &c.ready_downloads),
            pending: c
                .pending_downloads
                .iter()
                .map(|p| FfiPendingDownload {
                    full_id: p.full_id.clone(),
                    title: p.title.clone(),
                    artist: p.artist.clone(),
                    artwork_url: p.artwork_url.clone(),
                    state: download_state(p.state),
                    progress: c.download_progress.get(&p.full_id).copied(),
                })
                .collect(),
            collections: c
                .offline_collections
                .iter()
                .map(|col| FfiPlaylist::from_playlist(c, &col.playlist))
                .collect(),
            total_bytes: c.cache_usage.total as i64,
            pinned_bytes: c.cache_usage.pinned as i64,
            track_count: c.cache_usage.track_count as i64,
        })
        .unwrap_or_default()
    }

    pub fn download_track(&self, full_id: String) {
        self.with_track(full_id, |c, t| c.download_single_track(t));
    }

    pub fn remove_download(&self, full_id: String) {
        self.run(move |c| c.remove_downloaded_track(&full_id));
    }

    pub fn toggle_download_playlist(&self, key: String) {
        self.run(move |c| {
            if let Some(p) = find_playlist(c, &key) {
                let tracks = c
                    .playlist_details
                    .get(&p.stable_id())
                    .map(|d| d.tracks.clone())
                    .unwrap_or_default();
                c.toggle_download_collection(p, tracks);
            }
        });
    }

    pub fn download_library(&self, max_tracks: Option<u32>) {
        self.run(move |c| c.bulk_download_library(max_tracks.map(|m| m as usize)));
    }

    pub fn cancel_pending_downloads(&self) {
        self.run(|c| c.cancel_all_pending_downloads());
    }

    pub fn clear_auto_cache(&self) {
        self.run(|c| c.clear_auto_cache());
    }

    pub fn clear_all_downloads(&self) {
        self.run(|c| c.clear_all_downloads());
    }

    // ---- lyrics

    /// Lyrics of the current track, with the active synced line.
    pub fn lyrics(&self) -> FfiLyrics {
        self.query(|c| lyrics(c)).unwrap_or_default()
    }

    // ---- queue

    pub fn remove_upcoming(&self, offset: u32) {
        self.run(move |c| c.remove_upcoming_at(offset as usize));
    }

    pub fn move_upcoming(&self, from: u32, to: u32) {
        self.run(move |c| c.move_upcoming_item(from as usize, to as usize));
    }

    pub fn clear_upcoming(&self) {
        self.run(|c| c.clear_upcoming_queue());
    }

    // ---- settings

    pub fn settings(&self) -> FfiSettings {
        self.query(|c| {
            let s = &c.settings;
            FfiSettings {
                appearance: s.appearance.into(),
                crossfade_secs: s.crossfade_duration,
                broadcast: s.broadcast_enabled,
                reporting: s.playback_reporting_enabled,
                genius_fallback: s.genius_fallback_enabled,
                auto_cache: s.auto_cache_enabled,
                wifi_only: s.downloads_wifi_only,
                cache_limit_bytes: s.cache_limit_bytes as i64,
                volume: s.volume,
                muted: s.muted,
            }
        })
        .unwrap_or_default()
    }

    pub fn set_appearance(&self, appearance: FfiAppearance) {
        self.run(move |c| c.set_appearance(appearance.into()));
    }

    pub fn set_crossfade(&self, secs: f64) {
        self.run(move |c| c.set_crossfade_secs(secs));
    }

    pub fn toggle_broadcast(&self) {
        self.run(|c| c.toggle_broadcast_setting());
    }

    pub fn toggle_reporting(&self) {
        self.run(|c| c.toggle_reporting_setting());
    }

    pub fn toggle_genius_fallback(&self) {
        self.run(|c| c.toggle_genius_fallback());
    }

    pub fn toggle_auto_cache(&self) {
        self.run(|c| c.toggle_auto_cache());
    }

    pub fn toggle_wifi_only(&self) {
        self.run(|c| c.toggle_wifi_only());
    }

    pub fn set_cache_limit(&self, bytes: i64) {
        self.run(move |c| c.set_cache_limit(bytes.max(0) as u64));
    }

    pub fn toggle_mute(&self) {
        self.run(|c| c.toggle_mute());
    }
}
