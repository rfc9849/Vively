use std::sync::Arc;

use url::Url;
use vively_core::{
    Error,
    library::MusicLibrary,
    models::Playlist,
    session::Session,
    settings::{Appearance, DEFAULT_CROSSFADE},
};

use crate::{
    app::{AppCore, Dirty},
    models::{RecentQuickItem, RecentTargetKind},
};

const RECENT_LIMIT: usize = 4;

impl AppCore {
    /// Bring up everything tied to a signed-in account: library, offline
    /// store, saved queue, profile, first page loads and the download drain.
    pub fn activate_session(&mut self, session: Session) {
        let user_id = session.user_id;
        let token = session.access_token.clone();
        let needs_profile = session.user.is_none();
        self.session = Some(session);

        self.library = Arc::new(tokio::sync::Mutex::new(MusicLibrary::with_copy_history(
            self.service.clone(),
            user_id,
            self.data_dir
                .join(format!("library-copies-{user_id}.json")),
        )));
        let offline_root = self.data_dir.join("offline");
        self.attach_offline_store(&offline_root, user_id);
        self.restore_saved_queue(user_id);

        if needs_profile {
            let service = self.service.clone();
            let store = Arc::clone(&self.session_store);
            self.spawn(
                async move {
                    let user = service.profile_info(&token).await?;
                    let _ = store.update_user(user.clone());
                    Ok::<_, Error>(user)
                },
                |core, res| {
                    if let Ok(user) = res {
                        if let Some(sess) = core.session.as_mut() {
                            sess.user = Some(user);
                        }
                        core.mark(Dirty::SESSION);
                    }
                },
            );
        }

        self.load_home(false);
        self.load_playlists(false);
        self.load_library(false);
        self.trigger_downloads_drain();
        self.mark(Dirty::SESSION);
    }

    fn push_recent(&mut self, item: RecentQuickItem) {
        self.recent_items.retain(|r| r.id != item.id);
        self.recent_items.insert(0, item);
        self.recent_items.truncate(RECENT_LIMIT);
        if let Ok(bytes) = serde_json::to_vec(&self.recent_items) {
            let _ = std::fs::write(&self.recent_links_path, bytes);
        }
        self.mark(Dirty::HOME);
    }

    pub fn record_recent_playlist(&mut self, pl: &Playlist) {
        self.push_recent(RecentQuickItem {
            id: format!("pl:{}", pl.full_id()),
            title: pl.title.clone(),
            subtitle: Some(pl.byline()),
            artwork_url: pl.artwork_url().cloned(),
            kind: RecentTargetKind::Playlist(pl.clone()),
        });
    }

    pub fn record_recent_artist(&mut self, id: &str, name: &str, artwork_url: Option<Url>) {
        self.push_recent(RecentQuickItem {
            id: format!("ar:{id}"),
            title: name.to_owned(),
            subtitle: Some("Исполнитель".into()),
            artwork_url,
            kind: RecentTargetKind::Artist {
                id: id.to_owned(),
                name: name.to_owned(),
            },
        });
    }

    pub fn set_appearance(&mut self, appearance: Appearance) {
        self.settings.appearance = appearance;
        self.save_settings();
    }

    pub fn set_crossfade_secs(&mut self, secs: f64) {
        self.settings.crossfade_duration = secs.clamp(0.0, 12.0).round();
        self.save_settings();
    }

    pub fn toggle_crossfade_default(&mut self) {
        let next = if self.settings.crossfade_duration > 0.0 {
            0.0
        } else {
            DEFAULT_CROSSFADE
        };
        self.set_crossfade_secs(next);
    }

    pub fn toggle_genius_fallback(&mut self) {
        self.settings.genius_fallback_enabled = !self.settings.genius_fallback_enabled;
        self.save_settings();
        self.lyrics_states.clear();
        let provider = Arc::clone(&self.lyrics_provider);
        self.fire(async move {
            provider.lock().await.clear();
        });
        self.ensure_lyrics_for_current();
        self.mark(Dirty::LYRICS);
    }

    pub fn toggle_auto_cache(&mut self) {
        self.settings.auto_cache_enabled = !self.settings.auto_cache_enabled;
        self.save_settings();
    }

    pub fn toggle_wifi_only(&mut self) {
        self.settings.downloads_wifi_only = !self.settings.downloads_wifi_only;
        self.save_settings();
    }

    pub fn set_cache_limit(&mut self, limit_bytes: u64) {
        self.settings.cache_limit_bytes = limit_bytes;
        self.save_settings();
        if let Some(store) = self.offline_store.clone() {
            let _ = store.enforce_limit(limit_bytes, &self.protected_download_ids());
            self.sync_downloads_snapshot();
        }
    }

    pub fn toggle_broadcast_setting(&mut self) {
        self.settings.broadcast_enabled = !self.settings.broadcast_enabled;
        self.save_settings();
        self.sync_telemetry_and_broadcast();
    }

    pub fn toggle_reporting_setting(&mut self) {
        self.settings.playback_reporting_enabled = !self.settings.playback_reporting_enabled;
        self.save_settings();
        self.sync_telemetry_and_broadcast();
    }
}

impl AppCore {
    /// Drop everything tied to the current account (sign-out, user switch).
    pub fn reset_account(&mut self) {
        use crate::models::{LoadState, SearchResultsData};
        use vively_core::{downloads::CacheUsage, playback::PlaybackQueue};

        self.account_generation += 1;
        self.load_generation += 1;
        self.mix_generation += 1;
        self.search_generation += 1;
        self.editor_search_gen += 1;
        self.editor_picker_generation += 1;
        self.editor_saving = false;
        self.add_pl_added.clear();
        self.add_pl_busy.clear();
        self.upload_in_flight = false;
        self.extending_mix = false;
        self.drain_in_flight = false;
        self.drain_requested = false;
        self.playlist_detail_generations.clear();

        self.audio_engine.stop_and_clear();
        self.queue = PlaybackQueue::default();
        self.active_mix_session = None;
        self.current_elapsed = 0.0;
        self.pending_restore_pos = None;
        self.track_loading = false;
        self.preloading_stable_id = None;
        self.reporter.reset();
        self.auto_cache.cancel();
        let _ = std::fs::remove_file(&self.queue_snapshot_path);
        let _ = std::fs::remove_file(&self.queue_position_path);

        self.home_state = LoadState::Idle;
        self.home_blocks.clear();
        self.primary_mix = None;
        self.mix_picks.clear();
        self.recent_items.clear();
        let _ = std::fs::remove_file(&self.recent_links_path);

        self.library_state = LoadState::Idle;
        self.library = Arc::new(tokio::sync::Mutex::new(MusicLibrary::new(
            self.service.clone(),
            0,
        )));
        self.library_tracks.clear();
        self.library_revision = 0;
        self.library_foreign_copies.clear();
        self.library_pending_likes.clear();
        self.library_total = 0;
        self.library_can_more = false;
        self.library_loading_more = false;
        self.library_more_failed = false;
        self.restorable_own_track = None;

        self.playlists.clear();
        self.playlists_state = LoadState::Idle;
        self.disliked_tracks.clear();
        self.followed_owners.clear();
        self.profile_search = None;
        self.playlist_details.clear();
        self.artist_pages.clear();
        self.catalog_pages.clear();
        self.friends.clear();
        self.friends_total = 0;
        self.friends_state = LoadState::Idle;
        self.friend_pages.clear();

        self.search_query.clear();
        self.search_state = LoadState::Idle;
        self.search_results = SearchResultsData::default();
        self.search_history.clear();
        let _ = self.search_history.save(&self.search_history_path);

        self.offline_store = None;
        self.download_engine = None;
        self.ready_downloads.clear();
        self.pending_downloads.clear();
        self.download_progress.clear();
        self.offline_collections.clear();
        self.cache_usage = CacheUsage {
            total: 0,
            pinned: 0,
            track_count: 0,
        };

        self.lyrics_states.clear();
        let provider = Arc::clone(&self.lyrics_provider);
        self.fire(async move {
            provider.lock().await.clear();
        });
        self.mark(Dirty::ALL);
    }

    /// Stop broadcasting, forget the stored session and reset auth. Follow
    /// with [`AppCore::reset_account`].
    pub fn end_session(&mut self) {
        if let Some(token) = self.access_token() {
            let service = self.service.clone();
            let broadcaster = Arc::clone(&self.broadcaster);
            self.fire(async move {
                let mut b = broadcaster.lock().await;
                let _ = b.sync(&service, None, false, false, &token).await;
                b.reset();
            });
        }
        let _ = self.session_store.sign_out();
        self.reset_auth();
        self.session = None;
        self.mark(Dirty::SESSION);
    }

    /// Adopt a freshly logged-in session. Returns `true` if it replaced a
    /// different account (the embedder then resets its own per-account UI).
    pub fn adopt_login(&mut self, session: Session) -> bool {
        let switched = self
            .session
            .as_ref()
            .is_some_and(|prev| prev.user_id != session.user_id);
        if switched {
            self.reset_account();
        }
        let _ = self.session_store.adopt(session.clone());
        self.activate_session(session);
        self.toast("Вход выполнен");
        switched
    }
}
