use std::{
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use vively_audio::{DualAudioEngine, MediaCommand};
use vively_app::{
    AppCore, AuthField, CoreDeps, CoreEvent, Dirty, Host, TokioBridge,
    library::filter_tracks,
    models::{AuthStep, LoadState, SectionTarget},
};
use vively_core::{
    AudioService, VkClient,
    config::VkConfig,
    lyrics::LyricsProvider,
    models::Audio,
    playback::{PlaybackOrigin, RepeatMode},
    downloads::{NetworkInterface, NetworkState, NetworkStatus},
    session::{Session, SessionPersistence, SessionStore},
};

struct TestHost {
    wakes: Mutex<mpsc::Sender<()>>,
}

impl Host for TestHost {
    fn wake(&self) {
        let _ = self.wakes.lock().unwrap().send(());
    }

    fn network_state(&self) -> NetworkState {
        NetworkState {
            status: NetworkStatus::Online(NetworkInterface::Wifi),
            expensive: false,
            constrained: false,
        }
    }
}

#[derive(Default)]
struct MemPersistence(Mutex<Option<Session>>);

impl SessionPersistence for MemPersistence {
    fn load(&self) -> vively_core::Result<Option<Session>> {
        Ok(self.0.lock().unwrap().clone())
    }

    fn save(&self, session: &Session) -> vively_core::Result<()> {
        *self.0.lock().unwrap() = Some(session.clone());
        Ok(())
    }

    fn delete(&self) -> vively_core::Result<()> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

fn core() -> (AppCore, mpsc::Receiver<()>) {
    let (tx, rx) = mpsc::channel();
    let host = Arc::new(TestHost {
        wakes: Mutex::new(tx),
    });
    let vk_client = VkClient::new(VkConfig::android("test-secret").unwrap(), "testdevice").unwrap();
    let session_store =
        Arc::new(SessionStore::new(Arc::new(MemPersistence::default())).unwrap());
    let tmp = std::env::temp_dir().join(format!("vively-app-test-{}", std::process::id()));
    let deps = CoreDeps {
        lyrics_provider: Arc::new(tokio::sync::Mutex::new(
            LyricsProvider::new(AudioService::new(vk_client.clone())).unwrap(),
        )),
        audio_engine: DualAudioEngine::new_simulated().unwrap(),
        settings_path: tmp.join("settings.json"),
        queue_snapshot_path: tmp.join("queue.json"),
        queue_position_path: tmp.join("position.json"),
        service: AudioService::new(vk_client.clone()),
        vk_client,
        qr_device_name: "Vively (test)".into(),
        session_store,
        initial_session: None,
        search_history: Default::default(),
        settings: Default::default(),
        data_dir: tmp.clone(),
        recent_links_path: tmp.join("recent_links.json"),
        exports_dir: tmp.join("exports"),
        downloader: vively_core::media::Mp3StreamDownloader::new().unwrap(),
        search_history_path: std::env::temp_dir()
            .join(format!("vively-app-test-history-{}.json", std::process::id())),
    };
    (AppCore::new(host, TokioBridge::new().unwrap(), deps), rx)
}

#[test]
fn spawned_results_apply_on_pump_and_report_dirty() {
    let (mut core, wakes) = core();
    core.spawn(
        async {
            tokio::time::sleep(Duration::from_millis(5)).await;
            41 + 1
        },
        |core, value| {
            assert_eq!(value, 42);
            core.mark(Dirty::LIBRARY);
            core.toast("done");
        },
    );

    assert!(core.pump().is_empty(), "nothing should apply before completion");
    wakes
        .recv_timeout(Duration::from_secs(2))
        .expect("host should be woken");

    let dirty = core.pump();
    assert!(dirty.contains(Dirty::LIBRARY));
    assert!(!dirty.intersects(Dirty::PLAYBACK));
    assert!(core.pump().is_empty(), "dirty bits are consumed by pump");

    let events = core.take_events();
    assert!(matches!(events.as_slice(), [CoreEvent::Toast(m)] if m == "done"));
}

#[test]
fn jobs_can_chain_spawns() {
    let (mut core, wakes) = core();
    core.spawn(async { 1 }, |core, a| {
        core.spawn(async move { a + 1 }, |core, b| {
            assert_eq!(b, 2);
            core.mark(Dirty::SEARCH);
        });
    });
    for _ in 0..4 {
        wakes.recv_timeout(Duration::from_secs(2)).unwrap();
        if core.pump().contains(Dirty::SEARCH) {
            return;
        }
    }
    panic!("chained job never applied");
}

#[test]
fn direct_changes_outside_pump_wake_the_host_once() {
    let (mut core, wakes) = core();
    core.mark(Dirty::AUTH);
    core.toast("a");
    core.mark(Dirty::HOME);
    wakes.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(
        wakes.recv_timeout(Duration::from_millis(50)).is_err(),
        "wakes coalesce until the next pump"
    );
    assert!(core.pump().contains(Dirty::AUTH | Dirty::HOME));
    core.mark(Dirty::AUTH);
    wakes.recv_timeout(Duration::from_secs(1)).unwrap();
}

#[test]
fn invalid_phone_is_rejected_without_network() {
    let (mut core, _wakes) = core();
    core.submit_auth("123");
    assert!(core.auth.error.is_some());
    assert!(!core.auth.loading);
    assert_eq!(core.auth.step, AuthStep::Phone);
    assert!(core.pump().contains(Dirty::AUTH));
}

#[test]
fn auth_go_back_from_password_returns_to_phone_and_focuses_it() {
    let (mut core, _wakes) = core();
    core.auth.step = AuthStep::Password { had_otp: false };
    core.auth.error = Some("x".into());
    core.auth_go_back();
    assert_eq!(core.auth.step, AuthStep::Phone);
    assert!(core.auth.error.is_none());
    let events = core.take_events();
    assert!(matches!(
        events.as_slice(),
        [CoreEvent::FocusAuthField {
            field: AuthField::Phone,
            clear: false
        }]
    ));
}

#[test]
fn short_otp_code_is_rejected_locally() {
    let (mut core, _wakes) = core();
    core.auth.step = AuthStep::Otp {
        method_label: String::new(),
        subtitle: String::new(),
        placeholder: String::new(),
        resend_label: String::new(),
        code_len: 6,
    };
    core.submit_auth(" 12 ");
    assert_eq!(
        core.auth.error.as_deref(),
        Some("Введите код подтверждения полностью")
    );
    assert!(!core.auth.loading);
}

#[test]
fn loaders_are_noops_while_signed_out() {
    let (mut core, _wakes) = core();
    core.load_home(true);
    core.load_library(true);
    core.load_playlists(true);
    core.load_more_library();
    core.load_friends(true);
    core.load_artist("artist", true);
    core.load_section(
        SectionTarget::Id {
            section_id: "s".into(),
            title: "t".into(),
        },
        true,
    );
    assert_eq!(core.friends_state, LoadState::Idle);
    assert!(core.artist_pages.is_empty());
    assert!(core.catalog_pages.is_empty());
    assert_eq!(core.home_state, LoadState::Idle);
    assert_eq!(core.library_state, LoadState::Idle);
    assert_eq!(core.playlists_state, LoadState::Idle);
    assert!(!core.library_loading_more);
    assert!(core.pump().is_empty());
}

#[test]
fn filter_tracks_matches_title_or_artist_case_insensitively() {
    let mk = |id: i64, title: &str, artist: &str| {
        serde_json::from_value::<vively_core::models::Audio>(serde_json::json!({
            "id": id, "owner_id": 1, "title": title, "artist": artist, "duration": 10,
            "is_explicit": false, "url": ""
        }))
        .unwrap()
    };
    let tracks = vec![mk(1, "Кино", "Цой"), mk(2, "Other", "KINO band"), mk(3, "x", "y")];
    assert_eq!(filter_tracks(&tracks, "  ").len(), 3);
    assert_eq!(filter_tracks(&tracks, "кИНо").len(), 1);
    assert_eq!(filter_tracks(&tracks, "kino").len(), 1);
    assert!(filter_tracks(&tracks, "zzz").is_empty());
}

#[test]
fn blank_search_edit_resets_without_network() {
    let (mut core, _wakes) = core();
    core.search_state = LoadState::Loading;
    core.edit_search_query("   ".into());
    assert_eq!(core.search_state, LoadState::Idle);
    assert!(core.search_results.is_empty());
    assert_eq!(core.search_generation, 1);
    assert!(core.pump().contains(Dirty::SEARCH));
}

#[test]
fn download_actions_without_store_are_noops() {
    let (mut core, _wakes) = core();
    core.download_single_track(
        serde_json::from_value(serde_json::json!({
            "id": 1, "owner_id": 1, "title": "t", "artist": "a", "duration": 1,
            "is_explicit": false, "url": ""
        }))
        .unwrap(),
    );
    core.cancel_all_pending_downloads();
    core.trigger_downloads_drain();
    assert!(!core.drain_in_flight);
    assert!(!core.drain_requested);
    assert!(core.take_events().is_empty());
}

fn audio(id: i64, restricted: bool) -> Audio {
    serde_json::from_value(serde_json::json!({
        "id": id, "owner_id": 1, "title": format!("t{id}"), "artist": "a", "duration": 30,
        "is_explicit": false, "url": "", "content_restricted": if restricted { 1 } else { 0 }
    }))
    .unwrap()
}

#[test]
fn unplayable_single_track_is_reported_and_loading_clears() {
    let (mut core, wakes) = core();
    core.play_tracks(vec![audio(1, true)], 0, PlaybackOrigin::Other, "test", None);
    let mut events = core.take_events();
    for _ in 0..20 {
        if !core.track_loading {
            break;
        }
        let _ = wakes.recv_timeout(Duration::from_secs(2));
        core.pump();
        events.extend(core.take_events());
    }
    assert!(!core.track_loading);
    assert!(events.iter().any(|e| matches!(e, CoreEvent::ActiveTrackChanged)));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CoreEvent::Toast(m) if m.contains("недоступен")))
    );
}

#[test]
fn media_commands_update_repeat_and_stop_playback() {
    let (mut core, _wakes) = core();
    core.handle_media_command(MediaCommand::SetRepeat(RepeatMode::One));
    assert!(core.queue.repeat_mode == RepeatMode::One);
    core.current_elapsed = 12.0;
    core.track_loading = true;
    core.handle_media_command(MediaCommand::Stop);
    assert!(!core.track_loading);
    assert_eq!(core.current_elapsed, 0.0);
    assert!(core.pump().contains(Dirty::PLAYBACK));
}

#[test]
fn recent_artists_dedupe_and_cap_at_four() {
    let (mut core, _wakes) = core();
    for i in 0..6 {
        core.record_recent_artist(&format!("a{i}"), "n", None);
    }
    core.record_recent_artist("a3", "n", None);
    let ids: Vec<_> = core.recent_items.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["ar:a3", "ar:a5", "ar:a4", "ar:a2"]);
    assert!(core.pump().contains(Dirty::HOME));
}

#[test]
fn signed_out_actions_are_noops() {
    let (mut core, _wakes) = core();
    core.toggle_owner_follow(5);
    core.toggle_artist_follow("x".into());
    core.search_in_profile(5, "q".into());
    assert!(core.followed_owners.is_empty());
    assert!(core.profile_search.is_none());
    core.search_in_profile(5, String::new());
    assert!(core.profile_search.is_none());
}

#[test]
fn crossfade_setting_clamps_and_toggles() {
    let (mut core, _wakes) = core();
    core.set_crossfade_secs(40.0);
    assert_eq!(core.settings.crossfade_duration, 12.0);
    core.toggle_crossfade_default();
    assert_eq!(core.settings.crossfade_duration, 0.0);
    core.toggle_crossfade_default();
    assert!(core.settings.crossfade_duration > 0.0);
    assert!(core.pump().contains(Dirty::SETTINGS));
}

#[test]
fn editor_draft_ops_dedupe_reorder_and_respect_saving() {
    let (mut core, _wakes) = core();
    core.begin_playlist_editor(vec![audio(1, false)], false);
    core.add_editor_tracks_bulk(vec![audio(1, false), audio(2, false), audio(3, false)]);
    assert_eq!(core.editor_draft_tracks.len(), 3);
    core.move_editor_draft_track(2, 0);
    assert_eq!(core.editor_draft_tracks[0].id, 3);
    core.toggle_editor_draft_track(audio(2, false));
    assert_eq!(core.editor_draft_tracks.len(), 2);
    core.editor_saving = true;
    core.clear_editor_draft_tracks();
    assert_eq!(core.editor_draft_tracks.len(), 2, "no edits while saving");
}

#[test]
fn editor_save_requires_title_and_session() {
    let (mut core, _wakes) = core();
    core.save_playlist_editor(None, "   ", "");
    assert_eq!(core.editor_error.as_deref(), Some("Укажите название плейлиста"));
    core.save_playlist_editor(None, "Mix", "");
    assert!(core.editor_error.as_deref().unwrap().contains("Войдите"));
    assert!(!core.editor_saving);
}

#[test]
fn upload_rows_fall_back_to_file_name() {
    let item = vively_app::media::upload_item_for("/nonexistent/Artist - Song.mp3".into());
    assert_eq!(item.file_name, "Artist - Song.mp3");
    assert!(!item.artist.is_empty() || !item.title.is_empty());
}

#[test]
fn reset_account_clears_state_and_bumps_generations() {
    let (mut core, _wakes) = core();
    core.library_tracks.push(audio(1, false));
    core.disliked_tracks.insert((1, 1));
    core.record_recent_artist("a", "n", None);
    core.editor_saving = true;
    let generation = core.account_generation;
    core.reset_account();
    assert!(core.library_tracks.is_empty());
    assert!(core.disliked_tracks.is_empty());
    assert!(core.recent_items.is_empty());
    assert!(!core.editor_saving);
    assert_eq!(core.account_generation, generation + 1);
}

/// Debounce/poll timers must be creatable from a thread with no tokio
/// context (the gpui main thread, the Android core thread).
#[test]
fn debounced_search_from_non_runtime_thread_does_not_panic() {
    let (mut core, wakes) = core();
    std::thread::spawn(move || {
        core.edit_search_query("кино".into());
        assert_eq!(core.search_state, LoadState::Loading);
        let _ = wakes.recv_timeout(Duration::from_secs(2));
        core.pump();
    })
    .join()
    .expect("debounce timer must not panic outside a runtime");
}
