//! Commands beyond the basic screens: mixes, catalog links and sections,
//! playlist editing, add-to-playlist, uploads, exports, profile search and
//! offline collections.

use std::path::PathBuf;

use vively_app::{
    AppCore, Dirty, fmt,
    models::{SectionTarget, UploadItemStatus},
    palette::{DEFAULT_TINT, PaletteTokens, RgbColor, mix_oklab, parse_hex_color},
};
use vively_core::{
    catalog::HomeBlockKind,
    library::StreamMixSession,
    models::{MusicDestination, Playlist},
    playback::PlaybackOrigin,
    settings::CACHE_LIMIT_OPTIONS,
};

use crate::{
    FfiLoad, FfiPlaylist, FfiPlaylistDetail, FfiSection, FfiTrack, VivelyCore,
    screens::{find_playlist, find_track, playlists, section, tracks},
};

/// Where the UI should go after a navigation command.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum FfiNav {
    None,
    Library,
    Playlist { key: String },
    Artist { id: String, name: String },
    Friend { id: i64 },
    Section { key: String },
}

#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct FfiCatalogPage {
    pub key: String,
    pub title: String,
    pub state: FfiLoad,
    pub sections: Vec<FfiSection>,
    pub can_load_more: bool,
    pub loading_more: bool,
    pub more_failed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct FfiEditor {
    pub saving: bool,
    pub error: Option<String>,
    pub hidden: bool,
    pub cover_path: Option<String>,
    pub draft: Vec<FfiTrack>,
    /// Playlist drilled into in the picker; `None` lists playlists.
    pub picker_playlist: Option<FfiPlaylist>,
    pub picker_state: FfiLoad,
    pub picker_more: bool,
    pub picker_playlists: Vec<FfiPlaylist>,
    pub picker_tracks: Vec<FfiTrack>,
    pub search_state: FfiLoad,
    pub search_tracks: Vec<FfiTrack>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum FfiUploadStatus {
    Pending,
    Uploading,
    Done,
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiUploadItem {
    pub file_name: String,
    pub artist: String,
    pub title: String,
    pub status: FfiUploadStatus,
}

#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct FfiUpload {
    pub items: Vec<FfiUploadItem>,
    pub in_flight: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiProfileSearch {
    pub owner_id: i64,
    pub query: String,
    pub state: FfiLoad,
    pub tracks: Vec<FfiTrack>,
    pub playlists: Vec<FfiPlaylist>,
}

fn open_playlist_now(c: &mut AppCore, p: Playlist) -> String {
    let key = p.stable_id();
    c.record_recent_playlist(&p);
    c.ensure_playlist_entry(&p);
    c.load_playlist_detail(p, false);
    key
}

fn open_target(c: &mut AppCore, target: SectionTarget) -> String {
    let key = target.key();
    c.load_section(target, false);
    key
}

fn section_target(c: &AppCore, key: &str) -> Option<SectionTarget> {
    let title = c
        .catalog_pages
        .get(key)
        .map(|p| p.title.clone())
        .unwrap_or_default();
    if let Some(id) = key.strip_prefix("id:") {
        return Some(SectionTarget::Id {
            section_id: id.to_owned(),
            title,
        });
    }
    key.strip_prefix("url:").map(|url| SectionTarget::Url {
        url: url.to_owned(),
        title,
    })
}

fn link_nav(c: &mut AppCore, id: &str) -> FfiNav {
    let link = c.home_blocks.iter().find_map(|b| match &b.kind {
        HomeBlockKind::Links(list) => list.iter().find(|l| l.id == id).cloned(),
        _ => None,
    });
    let Some(link) = link else {
        return FfiNav::None;
    };
    if link.is_my_audios() {
        return FfiNav::Library;
    }
    if let Some(domain) = link.artist_domain() {
        let domain = domain.to_owned();
        c.load_artist(&domain, false);
        return FfiNav::Artist {
            id: domain,
            name: link.title.clone(),
        };
    }
    match link.destination() {
        Some(MusicDestination::Playlist(p)) => FfiNav::Playlist {
            key: open_playlist_now(c, *p),
        },
        Some(MusicDestination::CatalogUrl { url, title }) => FfiNav::Section {
            key: open_target(c, SectionTarget::Url { url, title }),
        },
        None => FfiNav::None,
    }
}

fn load_state(s: &vively_app::models::LoadState) -> FfiLoad {
    s.into()
}

#[uniffi::export]
impl VivelyCore {
    // ---- mixes

    pub fn toggle_mix_option(&self, category_id: String, option_id: String) {
        self.run(move |c| c.toggle_mix_option(&category_id, &option_id));
    }

    pub fn clear_mix_picks(&self) {
        self.run(|c| c.clear_mix_picks());
    }

    /// Start `mix_id` with the current mood picks.
    pub fn start_tuned_mix(&self, mix_id: String) {
        self.run(move |c| {
            let options = StreamMixSession::tuned(mix_id.clone(), c.mix_picks.clone())
                .ok()
                .and_then(|s| s.options_json);
            c.start_vk_mix(mix_id, None, options, "VK Микс".into());
        });
    }

    pub fn start_mix(
        &self,
        mix_id: String,
        entity_id: Option<String>,
        options_json: Option<String>,
        label: String,
    ) {
        self.run(move |c| c.start_vk_mix(mix_id, entity_id, options_json, label));
    }

    pub fn start_artist_mix(&self, artist_id: String, name: String) {
        self.run(move |c| {
            c.start_vk_mix("artist".into(), Some(artist_id), None, format!("Микс: {name}"))
        });
    }

    // ---- navigation

    /// Open a home quick link.
    pub fn open_link(&self, id: String) -> FfiNav {
        self.query(move |c| link_nav(c, &id)).unwrap_or(FfiNav::None)
    }

    /// Open the album of a visible track; returns the playlist key.
    pub fn open_track_album(&self, full_id: String) -> Option<String> {
        self.query(move |c| {
            let track = find_track(c, &full_id)?;
            let album = track.album.as_ref().filter(|a| a.id.unwrap_or(0) != 0)?;
            Some(open_playlist_now(c, Playlist::from_album(album)))
        })
        .flatten()
    }

    /// Owner of an open playlist: the library for the signed-in user.
    pub fn open_playlist_owner(&self, key: String) -> FfiNav {
        self.query(move |c| {
            let detail = c.playlist_details.get(&key).or_else(|| {
                let p = find_playlist(c, &key)?;
                c.playlist_details.get(&p.stable_id())
            })?;
            let owner = detail.owner.clone().filter(|_| detail.owner_audio_open)?;
            if c.signed_in_user_id() == Some(owner.id) {
                return Some(FfiNav::Library);
            }
            let id = owner.id;
            c.load_friend_music(owner, false);
            Some(FfiNav::Friend { id })
        })
        .flatten()
        .unwrap_or(FfiNav::None)
    }

    /// "Показать все": returns the section key.
    pub fn open_section(&self, section_id: String, title: String) -> String {
        self.query(move |c| open_target(c, SectionTarget::Id { section_id, title }))
            .unwrap_or_default()
    }

    pub fn section(&self, key: String) -> Option<FfiCatalogPage> {
        self.query(move |c| {
            let p = c.catalog_pages.get(&key)?;
            Some(FfiCatalogPage {
                title: p.title.clone(),
                state: load_state(&p.state),
                sections: p.sections.iter().map(|s| section(c, s)).collect(),
                can_load_more: key.starts_with("id:")
                    && p.next_from.as_deref().is_some_and(|n| !n.is_empty()),
                loading_more: p.loading_more,
                more_failed: p.more_failed,
                key,
            })
        })
        .flatten()
    }

    pub fn reload_section(&self, key: String) {
        self.run(move |c| {
            if let Some(t) = section_target(c, &key) {
                c.load_section(t, true);
            }
        });
    }

    pub fn load_more_section(&self, key: String) {
        self.run(move |c| {
            if let Some(t) = section_target(c, &key) {
                c.load_more_section(t);
            }
        });
    }

    // ---- playback variants

    pub fn play_tracks_shuffled(&self, full_ids: Vec<String>, label: String) {
        self.run(move |c| {
            let list: Vec<_> = full_ids.iter().filter_map(|id| find_track(c, id)).collect();
            if list.is_empty() {
                return;
            }
            c.queue.set_shuffled(true);
            c.play_tracks(list, 0, PlaybackOrigin::Other, label, None);
        });
    }

    pub fn play_library_shuffled(&self) {
        self.run(|c| {
            let tracks = c.library_tracks.clone();
            if tracks.is_empty() {
                return;
            }
            c.queue.set_shuffled(true);
            c.play_tracks(tracks, 0, PlaybackOrigin::Library, "Моя музыка", None);
        });
    }

    pub fn play_downloads(&self, start_index: u32, shuffle: bool) {
        self.run(move |c| {
            let tracks = c.ready_downloads.clone();
            if tracks.is_empty() {
                return;
            }
            if shuffle {
                c.queue.set_shuffled(true);
            }
            c.play_tracks(
                tracks,
                start_index as usize,
                PlaybackOrigin::Other,
                "Загруженное",
                None,
            );
        });
    }

    // ---- playlist track management

    pub fn remove_from_playlist(&self, full_id: String, key: String) {
        self.run(move |c| {
            if let (Some(t), Some(p)) = (find_track(c, &full_id), find_playlist(c, &key)) {
                c.remove_track_from_playlist(t, p);
            }
        });
    }

    pub fn move_in_playlist(&self, full_id: String, key: String, up: bool) {
        self.run(move |c| {
            if let (Some(t), Some(p)) = (find_track(c, &full_id), find_playlist(c, &key)) {
                c.move_track_in_playlist(t, p, up);
            }
        });
    }

    /// Reset the add-to-playlist picker state and refresh own playlists.
    pub fn begin_add_to_playlist(&self) {
        self.run(|c| {
            c.add_pl_added.clear();
            c.add_pl_busy.clear();
            c.load_playlists(false);
            c.mark(Dirty::PLAYLISTS);
        });
    }

    pub fn add_to_playlist(&self, full_id: String, key: String) {
        self.run(move |c| {
            if let (Some(t), Some(p)) = (find_track(c, &full_id), find_playlist(c, &key)) {
                c.add_track_to_playlist(t, p);
            }
        });
    }

    // ---- playlist editor

    /// Create (`editing_key == None`) or edit a playlist.
    pub fn begin_editor(&self, editing_key: Option<String>, seed_full_ids: Vec<String>) {
        self.run(move |c| {
            let hidden = match editing_key {
                Some(k) => find_playlist(c, &k).and_then(|p| p.no_discover),
                None => None,
            }
            .unwrap_or(false);
            let draft = seed_full_ids
                .iter()
                .filter_map(|id| find_track(c, id))
                .collect();
            c.begin_playlist_editor(draft, hidden);
        });
    }

    /// New playlist seeded with the current queue.
    pub fn begin_editor_from_queue(&self) {
        self.run(|c| {
            let draft = c.queue_tracks_for_export();
            c.begin_playlist_editor(draft, false);
        });
    }

    pub fn begin_add_tracks(&self) {
        self.run(|c| c.begin_add_tracks());
    }

    pub fn editor(&self) -> FfiEditor {
        self.query(|c| FfiEditor {
            saving: c.editor_saving,
            error: c.editor_error.clone(),
            hidden: c.editor_hidden,
            cover_path: c
                .editor_cover_preview
                .as_ref()
                .map(|p| p.display().to_string()),
            draft: tracks(c, &c.editor_draft_tracks),
            picker_playlist: c
                .editor_picker_playlist
                .as_ref()
                .map(|p| FfiPlaylist::from_playlist(c, p)),
            picker_state: load_state(&c.editor_picker_state),
            picker_more: c.editor_picker_more,
            picker_playlists: playlists(c, &c.editor_picker_playlists),
            picker_tracks: tracks(c, &c.editor_picker_tracks),
            search_state: load_state(&c.editor_search_state),
            search_tracks: tracks(c, &c.editor_search_tracks),
        })
        .unwrap_or_default()
    }

    /// Picker source: own playlists (`None`) or one playlist's tracks.
    pub fn editor_pick_playlist(&self, key: Option<String>) {
        self.run(move |c| {
            let p = match key {
                Some(k) => find_playlist(c, &k),
                None => None,
            };
            c.reset_editor_picker(p);
            c.load_editor_picker_page();
        });
    }

    pub fn editor_load_more(&self) {
        self.run(|c| c.load_editor_picker_page());
    }

    pub fn editor_toggle_track(&self, full_id: String) {
        self.with_track(full_id, |c, t| c.toggle_editor_draft_track(t));
    }

    pub fn editor_add_tracks(&self, full_ids: Vec<String>) {
        self.run(move |c| {
            let list = full_ids.iter().filter_map(|id| find_track(c, id)).collect();
            c.add_editor_tracks_bulk(list);
        });
    }

    pub fn editor_remove_tracks(&self, full_ids: Vec<String>) {
        self.run(move |c| {
            let list: Vec<_> = full_ids.iter().filter_map(|id| find_track(c, id)).collect();
            c.remove_editor_tracks_bulk(&list);
        });
    }

    pub fn editor_move_track(&self, from: u32, to: u32) {
        self.run(move |c| c.move_editor_draft_track(from as usize, to as usize));
    }

    pub fn editor_clear_tracks(&self) {
        self.run(|c| c.clear_editor_draft_tracks());
    }

    pub fn editor_set_hidden(&self, hidden: bool) {
        self.run(move |c| {
            if !c.editor_saving {
                c.editor_hidden = hidden;
                c.mark(Dirty::EDITOR);
            }
        });
    }

    /// `jpeg` is the encoded cover; `preview_path` is shown in the editor.
    pub fn editor_set_cover(&self, jpeg: Vec<u8>, preview_path: String) {
        self.run(move |c| c.set_editor_cover(jpeg, PathBuf::from(preview_path)));
    }

    pub fn editor_clear_cover(&self) {
        self.run(|c| c.clear_editor_cover());
    }

    pub fn editor_search(&self, query: String, immediate: bool) {
        self.run(move |c| c.search_editor_tracks(query, immediate));
    }

    pub fn editor_clear_search(&self) {
        self.run(|c| c.clear_editor_search());
    }

    /// Emits `PlaylistSaved` on success.
    pub fn save_editor(&self, editing_key: Option<String>, title: String, description: String) {
        self.run(move |c| {
            let editing = match editing_key {
                Some(k) => find_playlist(c, &k),
                None => None,
            };
            c.save_playlist_editor(editing, &title, &description);
        });
    }

    /// Append the draft to a playlist. Emits `EditorClosed` on success.
    pub fn submit_add_tracks(&self, key: String) {
        self.run(move |c| {
            if let Some(p) = find_playlist(c, &key) {
                c.submit_add_tracks_to_playlist(p);
            }
        });
    }

    // ---- upload

    pub fn set_upload_files(&self, paths: Vec<String>) {
        self.run(move |c| c.set_upload_files(paths.into_iter().map(PathBuf::from).collect()));
    }

    pub fn set_upload_tags(&self, index: u32, artist: String, title: String) {
        self.run(move |c| {
            if let Some(item) = c.upload_items.get_mut(index as usize) {
                item.artist = artist;
                item.title = title;
            }
        });
    }

    /// Emits `UploadFinished`.
    pub fn confirm_upload(&self) {
        self.run(|c| c.confirm_audio_upload());
    }

    pub fn clear_upload(&self) {
        self.run(|c| {
            if !c.upload_in_flight {
                c.upload_items.clear();
                c.mark(Dirty::UPLOAD);
            }
        });
    }

    pub fn upload(&self) -> FfiUpload {
        self.query(|c| FfiUpload {
            items: c
                .upload_items
                .iter()
                .map(|i| FfiUploadItem {
                    file_name: i.file_name.clone(),
                    artist: i.artist.clone(),
                    title: i.title.clone(),
                    status: match &i.status {
                        UploadItemStatus::Pending => FfiUploadStatus::Pending,
                        UploadItemStatus::Uploading => FfiUploadStatus::Uploading,
                        UploadItemStatus::Done => FfiUploadStatus::Done,
                        UploadItemStatus::Failed(m) => FfiUploadStatus::Failed { message: m.clone() },
                    },
                })
                .collect(),
            in_flight: c.upload_in_flight,
        })
        .unwrap_or_default()
    }

    // ---- export

    /// Writes an MP3 into the exports dir. Emits `ExportFinished`.
    pub fn export_track(&self, full_id: String) {
        self.with_track(full_id, |c, t| c.export_track_mp3(t));
    }

    // ---- friend profile search

    /// Empty query clears the results.
    pub fn search_in_profile(&self, owner_id: i64, query: String) {
        self.run(move |c| c.search_in_profile(owner_id, query));
    }

    pub fn profile_search(&self) -> Option<FfiProfileSearch> {
        self.query(|c| {
            let ps = c.profile_search.as_ref()?;
            Some(FfiProfileSearch {
                owner_id: ps.owner_id,
                query: ps.query.clone(),
                state: load_state(&ps.state),
                tracks: tracks(c, &ps.results.audios),
                playlists: playlists(c, &ps.results.playlists.items),
            })
        })
        .flatten()
    }

    // ---- offline collections

    pub fn offline_collection(&self, full_id: String) -> Option<FfiPlaylistDetail> {
        self.query(move |c| {
            let col = c.offline_collections.iter().find(|x| x.full_id == full_id)?;
            let list = c
                .offline_store
                .as_ref()
                .and_then(|s| s.collection_tracks(&full_id).ok())
                .unwrap_or_default();
            Some(FfiPlaylistDetail {
                playlist: FfiPlaylist::from_playlist(c, &col.playlist),
                state: FfiLoad::Loaded,
                total: list.len() as i64,
                total_secs: list.iter().map(|t| t.duration.max(0)).sum(),
                tracks: tracks(c, &list),
                loading_more: false,
                more_failed: false,
                downloaded: true,
                owner_music_id: None,
            })
        })
        .flatten()
    }

    pub fn play_offline_collection(&self, full_id: String, start_index: u32, shuffle: bool) {
        self.run(move |c| {
            let Some(title) = c
                .offline_collections
                .iter()
                .find(|x| x.full_id == full_id)
                .map(|x| x.playlist.title.clone())
            else {
                return;
            };
            let list = c
                .offline_store
                .as_ref()
                .and_then(|s| s.collection_tracks(&full_id).ok())
                .unwrap_or_default();
            if list.is_empty() {
                return;
            }
            if shuffle {
                c.queue.set_shuffled(true);
            }
            c.play_tracks(list, start_index as usize, PlaybackOrigin::Other, title, None);
        });
    }
}

#[uniffi::export]
pub fn format_total_duration(secs: i64) -> String {
    fmt::fmt_total_duration(secs)
}

#[uniffi::export]
pub fn format_plays_count(count: i64) -> String {
    fmt::fmt_plays_count(count)
}

fn tokens(is_dark: bool, tint_hex: Option<&str>) -> PaletteTokens {
    let (tint, neutral) = match tint_hex.and_then(parse_hex_color) {
        Some(c) => (c, false),
        None => (DEFAULT_TINT, true),
    };
    PaletteTokens::resolve(is_dark, tint, neutral)
}

/// Hero banner background; `collection_hex` is the playlist accent.
#[uniffi::export]
pub fn hero_banner_color(is_dark: bool, tint_hex: Option<String>, collection_hex: Option<String>) -> u32 {
    tokens(is_dark, tint_hex.as_deref())
        .hero_banner_bg(collection_hex.as_deref().and_then(parse_hex_color))
        .argb()
}

#[uniffi::export]
pub fn wave_hero_color(is_dark: bool, tint_hex: Option<String>) -> u32 {
    tokens(is_dark, tint_hex.as_deref()).wave_hero_bg().argb()
}

/// Cover placeholder gradient `[top, bottom]`.
#[uniffi::export]
pub fn placeholder_colors(is_dark: bool, tint_hex: Option<String>) -> Vec<u32> {
    let t = tokens(is_dark, tint_hex.as_deref()).tint;
    let h = RgbColor::hex;
    let (top, bottom) = if is_dark {
        (mix_oklab(t, 10.0, h(0x2B2C30)), mix_oklab(t, 8.0, h(0x1B1C1F)))
    } else {
        (mix_oklab(t, 5.0, h(0xEAEBED)), mix_oklab(t, 8.0, h(0xD5D7DD)))
    };
    vec![top.argb(), bottom.argb()]
}

/// Cache limit choices in bytes; 0 = unlimited.
#[uniffi::export]
pub fn cache_limit_options() -> Vec<i64> {
    CACHE_LIMIT_OPTIONS.iter().map(|&b| b as i64).collect()
}
