use std::collections::HashSet;

use vively_audio::{EngineTickOutcome, MediaCommand, TrackSource, prepare_track};
use vively_core::{
    Error,
    api::Page,
    library::StreamMixSession,
    lyrics::LyricsContent,
    models::{Audio, Playlist},
    playback::{PlaybackOrigin, PlaybackPosition, PlaybackQueue, PlaybackSnapshot, RepeatMode},
    session::unix_now,
};

use crate::{
    app::{AppCore, CoreEvent, Dirty},
    models::{LoadState, LyricsLoadState, PlaylistDetailData, active_synced_lyric_index},
};

/// Ticks between position saves while playing (embedder ticks every ~200 ms).
const POSITION_SAVE_TICKS: u32 = 20;

impl AppCore {
    fn active_track_changed(&mut self) {
        self.emit(CoreEvent::ActiveTrackChanged);
        self.mark(Dirty::PLAYBACK | Dirty::TINT);
    }

    fn invalidate_standby(&mut self) {
        self.preloading_stable_id = None;
        self.audio_engine.clear_standby();
    }

    pub fn play_tracks(
        &mut self,
        tracks: Vec<Audio>,
        start_index: usize,
        origin: PlaybackOrigin,
        from_label: impl Into<String>,
        mix_session: Option<StreamMixSession>,
    ) {
        if tracks.is_empty() || start_index >= tracks.len() {
            return;
        }
        if mix_session.is_some() {
            self.queue.set_shuffled(false);
        }
        if !self.queue.replace(tracks, start_index, origin) {
            return;
        }
        self.mix_generation += 1;
        self.extending_mix = false;
        self.playing_from = from_label.into();
        self.active_mix_session = mix_session;
        self.pending_restore_pos = None;
        self.current_elapsed = 0.0;
        self.start_current_queue_track(true, 0.0, 0);
        self.save_queue_snapshot();
    }

    pub fn play_next_track(&mut self, track: Audio) {
        let had_current = self.queue.current().is_some();
        self.queue.play_next(track);
        self.invalidate_standby();
        self.save_queue_snapshot();
        if had_current {
            self.toast("Добавлено следующим в очередь");
        } else {
            self.start_current_queue_track(true, 0.0, 0);
        }
        self.mark(Dirty::PLAYBACK);
    }

    pub fn play_playlist_immediately(&mut self, playlist: Playlist, offline_only: bool) {
        if offline_only {
            if let Some(store) = self.offline_store.clone()
                && let Ok(tracks) = store.collection_tracks(&playlist.full_id())
                && !tracks.is_empty()
            {
                self.play_tracks(tracks, 0, PlaybackOrigin::Other, playlist.title, None);
            }
            return;
        }
        let key = playlist.stable_id();
        if let Some(detail) = self.playlist_details.get(&key)
            && !detail.tracks.is_empty()
        {
            let tracks = detail.tracks.clone();
            self.play_tracks(tracks, 0, PlaybackOrigin::Other, playlist.title, None);
            return;
        }
        let Some(token) = self.access_token() else {
            return;
        };
        let account = self.account_generation;
        let playback = self.load_generation;
        let service = self.service.clone();
        let request = playlist.clone();
        self.spawn(
            async move {
                service
                    .playlist_contents(
                        &request,
                        Page {
                            offset: 0,
                            count: 200,
                        },
                        &token,
                    )
                    .await
            },
            move |core, res| {
                let Ok(contents) = res else {
                    return;
                };
                if core.account_generation != account || core.load_generation != playback {
                    return;
                }
                let tracks = contents.audios.items.clone();
                let resolved = contents.playlist.unwrap_or(playlist);
                let label = resolved.title.clone();
                core.playlist_details.insert(
                    key,
                    PlaylistDetailData {
                        playlist: resolved,
                        tracks: contents.audios.items,
                        total_count: contents.audios.count,
                        owner: contents.owner,
                        owner_audio_open: contents.owner_audio_open,
                        loading_more: false,
                        more_failed: false,
                        state: LoadState::Loaded,
                    },
                );
                core.mark(Dirty::PAGES);
                if !tracks.is_empty() {
                    core.play_tracks(tracks, 0, PlaybackOrigin::Other, label, None);
                }
            },
        );
    }

    pub fn start_vk_mix(
        &mut self,
        mix_id: String,
        entity_id: Option<String>,
        options_json: Option<String>,
        from_label: String,
    ) {
        let Some(token) = self.access_token() else {
            return;
        };
        let session = StreamMixSession {
            mix_id,
            entity_id,
            options_json,
        };
        self.mix_generation += 1;
        self.extending_mix = false;
        let generation = self.mix_generation;
        let service = self.service.clone();
        let request = session.clone();
        self.spawn(
            async move { request.initial(&service, &token).await },
            move |core, res| {
                if core.mix_generation != generation {
                    return;
                }
                match res {
                    Ok(tracks) if !tracks.is_empty() => {
                        core.play_tracks(
                            tracks,
                            0,
                            PlaybackOrigin::Other,
                            from_label.clone(),
                            Some(session),
                        );
                        core.toast(format!("Запущен «{from_label}»"));
                    }
                    Ok(_) => core.toast("В миксе пока нет доступных треков"),
                    Err(err) => core.toast(format!("Не удалось запустить микс: {err}")),
                }
            },
        );
    }

    pub fn toggle_or_start_home_mix(&mut self) {
        if self.active_mix_session.is_some() && self.queue.current().is_some() {
            self.toggle_play_pause();
            return;
        }
        let mix_id = self
            .primary_mix
            .as_ref()
            .map(|m| m.id.clone())
            .unwrap_or_else(|| "common".into());
        let label = self
            .primary_mix
            .as_ref()
            .map(|m| m.display_title().to_owned())
            .unwrap_or_else(|| "VK Микс".into());
        let session = StreamMixSession::tuned(mix_id.clone(), self.mix_picks.clone()).ok();
        self.start_vk_mix(mix_id, None, session.and_then(|s| s.options_json), label);
    }

    pub fn toggle_mix_option(&mut self, category_id: &str, option_id: &str) {
        let entry = self.mix_picks.entry(category_id.to_owned()).or_default();
        if let Some(pos) = entry.iter().position(|x| x == option_id) {
            entry.remove(pos);
        } else {
            entry.push(option_id.to_owned());
        }
        if entry.is_empty() {
            self.mix_picks.remove(category_id);
        }
        self.mark(Dirty::HOME);
    }

    pub fn clear_mix_picks(&mut self) {
        self.mix_picks.clear();
        self.mark(Dirty::HOME);
    }

    pub fn start_track_mix(&mut self, seed: Audio) {
        let (Some(token), Some(uid)) = (self.access_token(), self.signed_in_user_id()) else {
            return;
        };
        self.mix_generation += 1;
        self.extending_mix = false;
        let generation = self.mix_generation;
        let service = self.service.clone();
        let target = seed.full_id();
        self.spawn(
            async move {
                service
                    .recommendations(
                        &target,
                        uid,
                        Page {
                            offset: 0,
                            count: 40,
                        },
                        &token,
                    )
                    .await
            },
            move |core, res| {
                if core.mix_generation != generation {
                    return;
                }
                match res {
                    Ok(list) => {
                        let seed_id = seed.full_id();
                        let label = format!("Микс: {}", seed.title);
                        let mut tracks = vec![seed];
                        tracks.extend(list.items.into_iter().filter(|t| t.full_id() != seed_id));
                        core.play_tracks(tracks, 0, PlaybackOrigin::Other, label, None);
                        core.toast("Запущен микс по треку");
                    }
                    Err(err) => core.toast(format!("Ошибка микса: {err}")),
                }
            },
        );
    }

    pub fn start_current_queue_track(&mut self, autoplay: bool, start_secs: f64, skip_attempts: usize) {
        let Some(current_idx) = self.queue.current_index() else {
            return;
        };
        let Some(track) = self.queue.current().cloned() else {
            return;
        };

        self.load_generation += 1;
        let seq = self.load_generation;
        self.track_loading = true;
        self.audio_engine.stop_and_clear();
        self.current_elapsed = start_secs;
        self.preloading_stable_id = None;
        self.mark(Dirty::PLAYBACK | Dirty::POSITION);
        self.active_track_changed();
        self.ensure_lyrics_for_current();

        if track.content_restricted.unwrap_or(0) != 0 {
            self.skip_unplayable(&track, "трек ограничен правообладателем".into(), skip_attempts);
            return;
        }

        let local_path = self
            .offline_store
            .as_ref()
            .and_then(|s| s.local_file(&track).ok().flatten());
        let rt = self.tokio.handle();
        let http = self.audio_engine.http_client().clone();
        let service = self.service.clone();
        let token = self.access_token();
        let needs_refresh =
            local_path.is_none() && (self.queue.is_stale(&track) || track.stream_url().is_none());
        let failed_track = track.clone();

        self.spawn(
            async move {
                let mut resolved = track;
                if let Some(path) = local_path {
                    let prep = prepare_track(
                        &rt,
                        &http,
                        resolved.stable_id(),
                        resolved.duration as f64,
                        TrackSource::LocalFile(path),
                    )
                    .await?;
                    return Ok::<_, Error>((resolved, prep));
                }
                let mut refreshed = false;
                if needs_refresh
                    && let Some(tok) = &token
                    && let Ok(mut fresh) = service.tracks_by_id(&[resolved.stable_id()], tok).await
                    && let Some(first) = fresh.pop()
                {
                    resolved = first;
                    refreshed = true;
                }
                loop {
                    let attempt = match resolved.stream_url() {
                        Some(url) => {
                            prepare_track(
                                &rt,
                                &http,
                                resolved.stable_id(),
                                resolved.duration as f64,
                                TrackSource::RemoteUrl(url),
                            )
                            .await
                        }
                        None => Err(Error::Media("нет ссылки на поток".into())),
                    };
                    match attempt {
                        Ok(prep) => return Ok((resolved, prep)),
                        Err(err) if refreshed => return Err(err),
                        Err(err) => {
                            refreshed = true;
                            let fresh = match &token {
                                Some(tok) => service
                                    .tracks_by_id(&[resolved.stable_id()], tok)
                                    .await
                                    .ok()
                                    .and_then(|mut list| list.pop()),
                                None => None,
                            };
                            match fresh {
                                Some(fresh) => resolved = fresh,
                                None => return Err(err),
                            }
                        }
                    }
                }
            },
            move |core, res| {
                if core.load_generation != seq {
                    return;
                }
                core.track_loading = false;
                match res {
                    Ok((fresh, prepared)) => {
                        let target = core.pending_restore_pos.take().unwrap_or(start_secs);
                        core.queue.replace_track(fresh.clone());
                        match core
                            .audio_engine
                            .load_active(prepared, Some(current_idx), autoplay, target)
                        {
                            Ok(_) => {
                                core.current_elapsed = target;
                                core.on_track_became_active(&fresh);
                            }
                            Err(err) => core.skip_unplayable(&fresh, err.to_string(), skip_attempts),
                        }
                    }
                    Err(err) => core.skip_unplayable(&failed_track, err.to_string(), skip_attempts),
                }
                core.mark(Dirty::PLAYBACK | Dirty::POSITION);
            },
        );
    }

    fn skip_unplayable(&mut self, track: &Audio, reason: String, attempts: usize) {
        let total = self.queue.tracks().len();
        if total <= 1 || attempts >= 2 || attempts + 1 >= total {
            self.track_loading = false;
            self.audio_engine.stop_and_clear();
            self.toast(format!("«{}» недоступен: {reason}", track.title));
            self.mark(Dirty::PLAYBACK);
            return;
        }
        self.toast(format!("«{}» пропущен: {reason}", track.title));
        if self.queue.advance() {
            self.start_current_queue_track(true, 0.0, attempts + 1);
        }
    }

    fn on_track_became_active(&mut self, track: &Audio) {
        let origin = self.queue.origin(track);
        if let Some(store) = &self.offline_store {
            let _ = self
                .auto_cache
                .track_started(store, track, origin, &self.settings, unix_now());
        }
        self.sync_telemetry_and_broadcast();
        self.maintain_standby_preload();
        self.maybe_extend_mix();
    }

    pub fn toggle_play_pause(&mut self) {
        if self.queue.current().is_none() {
            return;
        }
        if self.track_loading {
            self.load_generation += 1;
            self.track_loading = false;
            self.pending_restore_pos = Some(self.current_elapsed);
            self.mark(Dirty::PLAYBACK);
            return;
        }
        if !self.audio_engine.has_active() {
            let resume_at = self
                .pending_restore_pos
                .take()
                .unwrap_or(self.current_elapsed);
            self.start_current_queue_track(true, resume_at, 0);
            return;
        }
        if self.audio_engine.is_playing() {
            self.audio_engine.pause();
            self.save_position_now();
        } else {
            self.audio_engine.play();
        }
        self.sync_telemetry_and_broadcast();
        self.mark(Dirty::PLAYBACK);
    }

    pub fn next_track(&mut self) {
        if self.queue.tracks().is_empty() {
            return;
        }
        let prev_mode = self.queue.repeat_mode;
        if prev_mode == RepeatMode::One {
            self.queue.repeat_mode = RepeatMode::All;
        }
        let moved = self.queue.advance();
        self.queue.repeat_mode = prev_mode;
        if moved {
            self.start_current_queue_track(true, 0.0, 0);
            self.save_queue_snapshot();
        }
        self.mark(Dirty::PLAYBACK);
    }

    pub fn prev_track(&mut self) {
        if self.queue.tracks().is_empty() {
            return;
        }
        let cur_idx = self.queue.current_index().unwrap_or(0);
        let next_idx = self.queue.previous(self.current_elapsed);
        if next_idx == cur_idx && self.audio_engine.has_active() {
            let _ = self.audio_engine.seek(0.0);
            self.current_elapsed = 0.0;
            self.emit(CoreEvent::PlaybackSeeked);
        } else {
            self.start_current_queue_track(true, 0.0, 0);
            self.save_queue_snapshot();
        }
        self.mark(Dirty::PLAYBACK | Dirty::POSITION);
    }

    pub fn seek_to(&mut self, secs: f64) {
        let dur = self
            .queue
            .current()
            .map(|t| t.duration as f64)
            .unwrap_or(0.0);
        let clamped = if dur > 0.0 {
            secs.clamp(0.0, dur)
        } else {
            secs.max(0.0)
        };
        if self.audio_engine.has_active() {
            let _ = self.audio_engine.seek(clamped);
        } else {
            self.pending_restore_pos = Some(clamped);
        }
        self.current_elapsed = clamped;
        self.save_position_now();
        self.emit(CoreEvent::PlaybackSeeked);
        self.mark(Dirty::POSITION | Dirty::LYRICS);
    }

    pub fn seek_relative(&mut self, delta_secs: f64) {
        if self.queue.current().is_none() {
            return;
        }
        let target = self.playback_position() + delta_secs;
        self.seek_to(target);
    }

    pub fn playback_position(&self) -> f64 {
        if self.audio_engine.has_active() {
            self.audio_engine.progress_position()
        } else {
            self.current_elapsed
        }
    }

    pub(crate) fn save_settings(&mut self) {
        let _ = self.settings.save(&self.settings_path);
        self.mark(Dirty::SETTINGS);
    }

    pub fn set_volume(&mut self, volume: f32) {
        let clamped = volume.clamp(0.0, 1.0);
        self.audio_engine.set_volume(clamped);
        self.settings.volume = clamped;
        self.settings.muted = self.audio_engine.is_muted();
        self.save_settings();
        self.mark(Dirty::PLAYBACK);
    }

    pub fn toggle_mute(&mut self) {
        self.settings.muted = self.audio_engine.toggle_mute();
        self.save_settings();
        self.mark(Dirty::PLAYBACK);
    }

    fn queue_order_changed(&mut self) {
        self.invalidate_standby();
        self.maintain_standby_preload();
        self.save_queue_snapshot();
        self.mark(Dirty::PLAYBACK);
    }

    pub fn toggle_shuffle(&mut self) {
        let next = !self.queue.is_shuffled();
        self.queue.set_shuffled(next);
        self.queue_order_changed();
    }

    pub fn cycle_repeat(&mut self) {
        self.queue.repeat_mode = self.queue.repeat_mode.cycle();
        self.queue_order_changed();
    }

    pub fn select_queue_track(&mut self, track: Audio) {
        if self.queue.select_track(&track) {
            self.start_current_queue_track(true, 0.0, 0);
            self.save_queue_snapshot();
        }
    }

    pub fn remove_upcoming_at(&mut self, offset: usize) {
        self.queue.remove_upcoming(&[offset]);
        self.queue_order_changed();
    }

    pub fn move_upcoming_item(&mut self, from: usize, to: usize) {
        if self.queue.move_upcoming(from, to) {
            self.queue_order_changed();
        }
    }

    pub fn clear_upcoming_queue(&mut self) {
        let count = self.queue.upcoming().len();
        if count == 0 {
            return;
        }
        let offsets: Vec<usize> = (0..count).collect();
        self.queue.remove_upcoming(&offsets);
        self.invalidate_standby();
        self.save_queue_snapshot();
        self.toast("Очередь очищена");
        self.mark(Dirty::PLAYBACK);
    }

    /// Current + upcoming tracks, deduped, for "save queue as playlist".
    pub fn queue_tracks_for_export(&self) -> Vec<Audio> {
        let mut tracks = Vec::new();
        let mut seen = HashSet::new();
        for tr in self.queue.current().into_iter().chain(self.queue.upcoming()) {
            if seen.insert((tr.owner_id, tr.id)) {
                tracks.push(tr.clone());
            }
        }
        tracks
    }

    pub fn maintain_standby_preload(&mut self) {
        if self.queue.repeat_mode == RepeatMode::One || self.audio_engine.is_crossfading() {
            return;
        }
        let Some(next_idx) = self.queue.next_index() else {
            return;
        };
        if Some(next_idx) == self.queue.current_index() {
            return;
        }
        let Some(next_track) = self.queue.tracks().get(next_idx).cloned() else {
            return;
        };
        let sid = next_track.stable_id();
        if self.audio_engine.has_valid_standby(next_idx, &sid)
            || self.preloading_stable_id.as_deref() == Some(sid.as_str())
        {
            return;
        }
        if next_track.content_restricted.unwrap_or(0) != 0 {
            return;
        }

        self.preloading_stable_id = Some(sid.clone());
        let generation = self.load_generation;
        let local_path = self
            .offline_store
            .as_ref()
            .and_then(|s| s.local_file(&next_track).ok().flatten());
        let rt = self.tokio.handle();
        let http = self.audio_engine.http_client().clone();
        let service = self.service.clone();
        let token = self.access_token();
        let needs_refresh = local_path.is_none()
            && (self.queue.is_stale(&next_track) || next_track.stream_url().is_none());

        self.spawn(
            async move {
                let mut resolved = next_track;
                let source = if let Some(path) = local_path {
                    TrackSource::LocalFile(path)
                } else {
                    if needs_refresh
                        && let Some(tok) = &token
                        && let Ok(mut fresh) =
                            service.tracks_by_id(&[resolved.stable_id()], tok).await
                        && let Some(first) = fresh.pop()
                    {
                        resolved = first;
                    }
                    let url = resolved
                        .stream_url()
                        .ok_or_else(|| Error::Media("Нет потока".into()))?;
                    TrackSource::RemoteUrl(url)
                };
                let prep = prepare_track(
                    &rt,
                    &http,
                    resolved.stable_id(),
                    resolved.duration as f64,
                    source,
                )
                .await?;
                Ok::<_, Error>((resolved, prep))
            },
            move |core, res| {
                if core.load_generation != generation
                    || core.preloading_stable_id.as_deref() != Some(sid.as_str())
                {
                    return;
                }
                core.preloading_stable_id = None;
                if let Ok((fresh, prep)) = res
                    && core.queue.next_index() == Some(next_idx)
                    && core
                        .queue
                        .tracks()
                        .get(next_idx)
                        .map(Audio::stable_id)
                        .as_deref()
                        == Some(sid.as_str())
                {
                    core.queue.replace_track(fresh);
                    let _ = core.audio_engine.preload_standby(prep, next_idx);
                    core.mark(Dirty::PLAYBACK);
                }
            },
        );
    }

    fn maybe_extend_mix(&mut self) {
        if self.extending_mix || !self.queue.needs_more() {
            return;
        }
        let (Some(mix_sess), Some(token)) = (self.active_mix_session.clone(), self.access_token())
        else {
            return;
        };
        self.extending_mix = true;
        let generation = self.mix_generation;
        let service = self.service.clone();
        self.spawn(
            async move { mix_sess.next_batch(&service, &token).await },
            move |core, res| {
                if core.mix_generation != generation {
                    return;
                }
                core.extending_mix = false;
                if let Ok(more) = res
                    && !more.is_empty()
                {
                    core.queue.append(more);
                    core.maintain_standby_preload();
                    core.mark(Dirty::PLAYBACK);
                }
            },
        );
    }

    /// Periodic housekeeping; the embedder calls this every ~200 ms.
    pub fn tick(&mut self) {
        if self.audio_engine.has_active() {
            let outcome = self.audio_engine.tick(self.settings.crossfade_duration);
            let pos = self.audio_engine.position();
            if self.current_elapsed.floor() as i64 != pos.floor() as i64 {
                self.mark(Dirty::POSITION);
            }
            if self.did_cross_synced_lyric(self.current_elapsed, pos) {
                self.mark(Dirty::LYRICS);
            }
            self.current_elapsed = pos;
            if let Some(cur) = self.queue.current() {
                self.reporter.update_position(cur, self.current_elapsed);
            }

            match outcome {
                EngineTickOutcome::PromotedStandby { queue_index } => {
                    self.queue.select(queue_index);
                    self.current_elapsed = 0.0;
                    self.active_track_changed();
                    self.ensure_lyrics_for_current();
                    if let Some(cur) = self.queue.current().cloned() {
                        self.on_track_became_active(&cur);
                    }
                    self.save_queue_snapshot();
                    self.mark(Dirty::PLAYBACK | Dirty::POSITION);
                }
                EngineTickOutcome::TrackEnded => {
                    if self.queue.repeat_mode == RepeatMode::One {
                        self.start_current_queue_track(true, 0.0, 0);
                    } else if self.queue.advance() {
                        self.start_current_queue_track(true, 0.0, 0);
                        self.save_queue_snapshot();
                    } else {
                        self.audio_engine.stop_and_clear();
                        self.current_elapsed = 0.0;
                        self.sync_telemetry_and_broadcast();
                    }
                    self.mark(Dirty::PLAYBACK | Dirty::POSITION);
                }
                EngineTickOutcome::OutputLost {
                    position,
                    was_playing,
                } => {
                    self.current_elapsed = position;
                    self.pending_restore_pos = Some(position);
                    if was_playing {
                        self.start_current_queue_track(true, position, 0);
                    }
                    self.mark(Dirty::PLAYBACK | Dirty::POSITION);
                }
                EngineTickOutcome::Idle | EngineTickOutcome::Playing => {}
            }

            if self.audio_engine.is_playing() {
                self.position_save_ticks += 1;
                if self.position_save_ticks >= POSITION_SAVE_TICKS {
                    self.position_save_ticks = 0;
                    self.save_position_now();
                }
            }
        }

        if let Some(store) = self.offline_store.clone() {
            let network = self.network_state();
            if let Ok(true) = self
                .auto_cache
                .tick(&store, network, &self.settings, unix_now())
            {
                self.sync_downloads_snapshot();
                self.trigger_downloads_drain();
            }
        }
    }

    fn did_cross_synced_lyric(&self, old_pos: f64, new_pos: f64) -> bool {
        let Some(cur) = self.queue.current() else {
            return false;
        };
        let Some(LyricsLoadState::Loaded(Some(lyr))) = self.lyrics_states.get(&cur.stable_id())
        else {
            return false;
        };
        let LyricsContent::Synced(lines) = &lyr.content else {
            return false;
        };
        let old_ms = (old_pos * 1000.0).round() as i64;
        let new_ms = (new_pos * 1000.0).round() as i64;
        active_synced_lyric_index(lines, old_ms) != active_synced_lyric_index(lines, new_ms)
    }

    /// OS media keys / notification / lock-screen controls.
    pub fn handle_media_command(&mut self, cmd: MediaCommand) {
        match cmd {
            MediaCommand::Play => {
                if !self.audio_engine.is_playing() && !self.track_loading {
                    self.toggle_play_pause();
                }
            }
            MediaCommand::Pause => {
                if self.audio_engine.is_playing() || self.track_loading {
                    self.toggle_play_pause();
                }
            }
            MediaCommand::TogglePlayPause => self.toggle_play_pause(),
            MediaCommand::Stop => {
                self.load_generation += 1;
                self.track_loading = false;
                self.audio_engine.stop_and_clear();
                self.current_elapsed = 0.0;
                self.sync_telemetry_and_broadcast();
                self.mark(Dirty::PLAYBACK | Dirty::POSITION);
            }
            MediaCommand::Next => self.next_track(),
            MediaCommand::Previous => self.prev_track(),
            MediaCommand::SeekTo(pos) => self.seek_to(pos),
            MediaCommand::SeekBy(delta) => self.seek_to(self.current_elapsed + delta),
            MediaCommand::SetVolume(vol) => self.set_volume(vol),
            MediaCommand::SetShuffle(shuf) => {
                if self.queue.is_shuffled() != shuf {
                    self.toggle_shuffle();
                }
            }
            MediaCommand::SetRepeat(rep) => {
                self.queue.repeat_mode = rep;
                self.save_queue_snapshot();
                self.mark(Dirty::PLAYBACK);
            }
        }
    }

    pub fn sync_telemetry_and_broadcast(&mut self) {
        let Some(token) = self.access_token() else {
            return;
        };
        let playing = self.audio_engine.is_playing();
        let events = self.reporter.sync(
            &self.queue,
            playing,
            self.current_elapsed,
            self.settings.playback_reporting_enabled,
            unix_now(),
        );
        if !events.is_empty() {
            let service = self.service.clone();
            let tok = token.clone();
            self.fire(async move {
                let _ = service.report_playback_events(&events, &tok).await;
            });
        }

        let cur = self.queue.current().cloned();
        let enabled = self.settings.broadcast_enabled;
        let service = self.service.clone();
        let broadcaster = std::sync::Arc::clone(&self.broadcaster);
        self.fire(async move {
            let mut guard = broadcaster.lock().await;
            let _ = guard
                .sync(&service, cur.as_ref(), playing, enabled, &token)
                .await;
        });
    }

    pub fn save_queue_snapshot(&self) {
        if self.active_mix_session.is_some() {
            return;
        }
        if let Some(uid) = self.signed_in_user_id()
            && let Some(snap) = self.queue.snapshot(uid, unix_now())
        {
            let _ = snap.save(&self.queue_snapshot_path);
        }
    }

    pub fn save_position_now(&self) {
        if let Some(cur) = self.queue.current() {
            let pos = PlaybackPosition {
                track_id: cur.stable_id(),
                time: self.current_elapsed,
            };
            let _ = pos.save(&self.queue_position_path);
        }
    }

    pub fn restore_saved_queue(&mut self, user_id: i64) {
        let Ok(Some(snapshot)) = PlaybackSnapshot::load(&self.queue_snapshot_path) else {
            return;
        };
        let Ok(restored) = PlaybackQueue::restore(snapshot, user_id, unix_now()) else {
            return;
        };
        let saved_pos = PlaybackPosition::load(&self.queue_position_path)
            .ok()
            .flatten();
        let pos = restored
            .current()
            .and_then(|tr| saved_pos.as_ref().map(|p| p.time_for(tr)))
            .unwrap_or(0.0);

        self.queue = restored;
        self.current_elapsed = pos;
        self.pending_restore_pos = Some(pos);
        self.playing_from = match self.queue.current().map(|t| self.queue.origin(t)) {
            Some(PlaybackOrigin::Library) => "Моя музыка".into(),
            _ => "Очередь".into(),
        };
        self.active_track_changed();
        self.ensure_lyrics_for_current();
    }

    pub fn ensure_lyrics_for_current(&mut self) {
        let Some(track) = self.queue.current().cloned() else {
            return;
        };
        let sid = track.stable_id();
        if self.lyrics_states.contains_key(&sid) {
            return;
        }
        self.lyrics_states.insert(sid.clone(), LyricsLoadState::Loading);
        self.mark(Dirty::LYRICS);

        let account = self.account_generation;
        let provider = std::sync::Arc::clone(&self.lyrics_provider);
        let token = self.access_token();
        let genius = self.settings.genius_fallback_enabled;
        self.spawn(
            async move {
                provider
                    .lock()
                    .await
                    .load(&track, token.as_deref(), genius)
                    .await
            },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                let state = match res {
                    Ok(lyr) => LyricsLoadState::Loaded(lyr),
                    Err(err) => LyricsLoadState::Failed(err.to_string()),
                };
                core.lyrics_states.insert(sid, state);
                core.mark(Dirty::LYRICS);
            },
        );
    }
}
