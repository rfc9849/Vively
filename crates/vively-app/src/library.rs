use std::sync::Arc;

use vively_core::{
    Error,
    catalog::{HomeBlockKind, home_blocks},
    library::LibrarySnapshot,
    models::Audio,
};

use crate::{
    app::{AppCore, Dirty},
    models::LoadState,
};

/// Case-insensitive title/artist filter used by every "Фильтр" box.
pub fn filter_tracks(tracks: &[Audio], query: &str) -> Vec<Audio> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return tracks.to_vec();
    }
    tracks
        .iter()
        .filter(|t| t.title.to_lowercase().contains(&q) || t.artist.to_lowercase().contains(&q))
        .cloned()
        .collect()
}

impl AppCore {
    pub fn access_token(&self) -> Option<String> {
        if let Ok(Some(live)) = self.session_store.session() {
            return Some(live.access_token);
        }
        self.session.as_ref().map(|s| s.access_token.clone())
    }

    pub fn signed_in_user_id(&self) -> Option<i64> {
        if let Ok(Some(live)) = self.session_store.session() {
            return Some(live.user_id);
        }
        self.session.as_ref().map(|s| s.user_id)
    }

    pub fn load_home(&mut self, force: bool) {
        if !force && matches!(self.home_state, LoadState::Loading | LoadState::Loaded) {
            return;
        }
        let Some(token) = self.access_token() else {
            return;
        };
        self.home_state = LoadState::Loading;
        self.mark(Dirty::HOME);

        let account = self.account_generation;
        let service = self.service.clone();
        self.spawn(
            async move {
                let resp = service.audio_home(&token).await?;
                let blocks = home_blocks(&resp);
                let primary_mix = blocks.iter().find_map(|b| match &b.kind {
                    HomeBlockKind::StreamMixes(mixes) => mixes.first().cloned(),
                    _ => None,
                });
                let settings = if let Some(mix) = &primary_mix {
                    service.stream_mix_settings(&mix.id, &token).await.ok()
                } else {
                    None
                };
                Ok::<_, Error>((blocks, primary_mix, settings))
            },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                match res {
                    Ok((blocks, mix, settings)) => {
                        core.home_blocks = blocks;
                        if let Some(mix) = mix {
                            core.primary_mix = Some(mix);
                        }
                        if let Some(settings) = settings
                            && !settings.mix_categories.is_empty()
                        {
                            core.mix_settings = settings;
                        }
                        core.home_state = LoadState::Loaded;
                    }
                    Err(err) => core.home_state = LoadState::Failed(err.to_string()),
                }
                core.mark(Dirty::HOME);
            },
        );
    }

    pub fn apply_library_snapshot(&mut self, snapshot: LibrarySnapshot) {
        if snapshot.revision < self.library_revision {
            return;
        }
        self.library_revision = snapshot.revision;
        self.library_tracks = snapshot.tracks;
        self.library_total = snapshot.total_count;
        self.library_can_more = snapshot.can_load_more;
        self.library_foreign_copies = snapshot.foreign_copies;
        self.mark(Dirty::LIBRARY);
    }

    pub fn load_library(&mut self, force: bool) {
        if !force && matches!(self.library_state, LoadState::Loading | LoadState::Loaded) {
            return;
        }
        let Some(token) = self.access_token() else {
            return;
        };
        self.library_state = LoadState::Loading;
        self.mark(Dirty::LIBRARY);

        let lib = Arc::clone(&self.library);
        let request_lib = Arc::clone(&lib);
        self.spawn(
            async move {
                let mut guard = lib.lock().await;
                guard.refresh(&token).await?;
                Ok::<_, Error>(guard.snapshot())
            },
            move |core, res| {
                if !Arc::ptr_eq(&core.library, &request_lib) {
                    return;
                }
                match res {
                    Ok(snapshot) => {
                        core.apply_library_snapshot(snapshot);
                        core.library_more_failed = false;
                        core.library_state = LoadState::Loaded;
                    }
                    Err(err) => core.library_state = LoadState::Failed(err.to_string()),
                }
                core.mark(Dirty::LIBRARY);
            },
        );
    }

    /// Next page of "Моя музыка". The caller decides whether the list is
    /// actually on screen (route / picker checks are UI concerns).
    pub fn load_more_library(&mut self) {
        if !matches!(self.library_state, LoadState::Loaded)
            || self.library_loading_more
            || !self.library_can_more
        {
            return;
        }
        let Some(token) = self.access_token() else {
            return;
        };
        self.library_more_failed = false;
        self.library_loading_more = true;
        self.mark(Dirty::LIBRARY);

        let lib = Arc::clone(&self.library);
        let request_lib = Arc::clone(&lib);
        self.spawn(
            async move {
                let mut guard = lib.lock().await;
                guard.load_more(&token).await?;
                Ok::<_, Error>(guard.snapshot())
            },
            move |core, res| {
                if !Arc::ptr_eq(&core.library, &request_lib) {
                    return;
                }
                core.library_loading_more = false;
                match res {
                    Ok(snapshot) => core.apply_library_snapshot(snapshot),
                    Err(_) => core.library_more_failed = true,
                }
                core.mark(Dirty::LIBRARY);
            },
        );
    }

    pub fn load_playlists(&mut self, force: bool) {
        if !force && matches!(self.playlists_state, LoadState::Loading | LoadState::Loaded) {
            return;
        }
        let (Some(token), Some(uid)) = (self.access_token(), self.signed_in_user_id()) else {
            return;
        };
        self.playlists_state = LoadState::Loading;
        self.mark(Dirty::PLAYLISTS);

        let account = self.account_generation;
        self.playlists_generation += 1;
        let generation = self.playlists_generation;
        let service = self.service.clone();
        self.spawn(
            async move { service.all_playlists(uid, &token).await },
            move |core, res| {
                if core.account_generation != account || core.playlists_generation != generation {
                    return;
                }
                match res {
                    Ok(list) => {
                        core.playlists = list.items;
                        core.playlists_state = LoadState::Loaded;
                    }
                    Err(err) => core.playlists_state = LoadState::Failed(err.to_string()),
                }
                core.mark(Dirty::PLAYLISTS);
            },
        );
    }

    pub fn is_liked(&self, track: &Audio) -> bool {
        self.library_foreign_copies
            .contains_key(&(track.owner_id, track.id))
            || self
                .library_tracks
                .iter()
                .any(|t| t.id == track.id && t.owner_id == track.owner_id)
    }

    pub fn toggle_like_track(&mut self, track: Audio) {
        let (Some(token), Some(uid)) = (self.access_token(), self.signed_in_user_id()) else {
            return;
        };
        let track_id = (track.owner_id, track.id);
        let saved_id = self
            .library_foreign_copies
            .get(&track_id)
            .map_or(track_id, |&copy_id| (uid, copy_id));
        if !self.library_pending_likes.insert(saved_id) {
            return;
        }

        let lib = Arc::clone(&self.library);
        let request_lib = Arc::clone(&lib);
        let tr = track.clone();
        self.spawn(
            async move {
                let mut guard = lib.lock().await;
                let liked = guard.toggle_like(&tr, &token).await?;
                Ok::<_, Error>((liked, guard.snapshot()))
            },
            move |core, res| {
                if !Arc::ptr_eq(&core.library, &request_lib) {
                    return;
                }
                core.library_pending_likes.remove(&saved_id);
                match res {
                    Ok((liked, snapshot)) => {
                        core.apply_library_snapshot(snapshot);
                        if liked {
                            if core
                                .restorable_own_track
                                .as_ref()
                                .is_some_and(|r| (r.owner_id, r.id) == track_id)
                            {
                                core.restorable_own_track = None;
                            }
                            core.toast("Добавлено в Мою музыку");
                        } else {
                            if track.owner_id == uid {
                                core.restorable_own_track = Some(track);
                            }
                            core.toast("Убрано из Моей музыки");
                        }
                    }
                    Err(err) => core.toast(format!("Ошибка: {err}")),
                }
                core.mark(Dirty::LIBRARY);
            },
        );
    }

    pub fn restore_own_removed_track(&mut self) {
        if let Some(track) = self.restorable_own_track.clone() {
            self.toggle_like_track(track);
        }
    }
}
