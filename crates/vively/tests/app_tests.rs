use std::{io::Cursor, sync::Arc};

use image::{ImageFormat, Rgba, RgbaImage};
use serde_json::json;
use vively::{
    design::{
        artwork::{encode_cover_jpeg, sample_cover_bytes},
        colors::{DEFAULT_TINT, Palette, RgbColor, mix_oklab, parse_hex_color},
    },
    state::{
        app_state::{FileSessionPersistence, VivelyApp, encode_qr_matrix},
        models::{
            FriendMusicData, LYRICS_MANUAL_PAUSE, LoadState, LyricsLoadState, LyricsScrollState,
            PlaylistDetailData, active_synced_lyric_index,
        },
        router::{ActiveModal, PanelTab, Route, RouterState, SectionTarget, SettingsTab},
        runtime::{
            AppPaths, CaptchaNavOutcome, extract_captcha_success_token, fmt_duration,
            fmt_friends_count, fmt_gb, fmt_playlists_count, fmt_tracks_count,
            parse_captcha_nav_outcome,
        },
    },
};
use std::time::{Duration, Instant};

use vively_core::{
    lyrics::{LyricsContent, LyricsSource, SyncedLine, TrackLyrics},
    models::{Audio, Friend, Playlist, VkUser},
    playback::PlaybackOrigin,
    session::Session,
};

fn make_png(px_color: [u8; 4], w: u32, h: u32) -> Vec<u8> {
    let img = RgbaImage::from_pixel(w, h, Rgba(px_color));
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .unwrap();
    out
}

fn sample_audio(id: i64, title: &str, artist: &str) -> Audio {
    serde_json::from_value(json!({
        "id": id,
        "owner_id": 42,
        "title": title,
        "artist": artist,
        "duration": 141,
        "is_explicit": id % 2 == 0,
        "url": ""
    }))
    .unwrap()
}

#[test]
fn oklab_mixing_hex_parsing_and_palette_modes() {
    let red = parse_hex_color("#D91E18").unwrap();
    let short_white = parse_hex_color("#fff").unwrap();
    assert!((short_white.r - 1.0).abs() < 1e-4);
    assert!(parse_hex_color("invalid").is_none());

    let mixed = mix_oklab(red, 50.0, RgbColor::hex(0x1C1D1F));
    assert!(mixed.r > 0.15 && mixed.r < 0.85);

    let neutral_dark = Palette::resolve(true, DEFAULT_TINT, true);
    assert_eq!(neutral_dark.hi, neutral_dark.ink);

    let chromatic_dark = Palette::resolve(true, red, false);
    assert_ne!(chromatic_dark.hi, chromatic_dark.ink);

    let chromatic_light = Palette::resolve(false, red, false);
    assert!(!chromatic_light.is_dark);
    assert_ne!(chromatic_light.ground, chromatic_dark.ground);
}

#[test]
fn cover_sampling_distinguishes_grey_from_saturated_and_encodes_jpeg() {
    let grey_png = make_png([120, 121, 120, 255], 32, 32);
    let grey_tint = sample_cover_bytes(&grey_png).unwrap();
    assert!(grey_tint.is_neutral);

    let vivid_png = make_png([220, 40, 55, 255], 32, 32);
    let vivid_tint = sample_cover_bytes(&vivid_png).unwrap();
    assert!(!vivid_tint.is_neutral);
    assert!(vivid_tint.color.r > vivid_tint.color.g);

    let rect_png = make_png([30, 140, 220, 255], 300, 180);
    let jpeg = encode_cover_jpeg(&rect_png).unwrap();
    assert!(jpeg.starts_with(&[0xFF, 0xD8]));
    let decoded = image::load_from_memory(&jpeg).unwrap();
    assert_eq!(decoded.width(), 180);
    assert_eq!(decoded.height(), 180);
}

#[test]
fn russian_pluralization_and_smartcaptcha_token_parsing() {
    assert_eq!(fmt_duration(54), "0:54");
    assert_eq!(fmt_duration(141), "2:21");
    assert_eq!(fmt_tracks_count(1), "1 трек");
    assert_eq!(fmt_tracks_count(2), "2 трека");
    assert_eq!(fmt_tracks_count(11), "11 треков");
    assert_eq!(fmt_tracks_count(1482), "1 482 трека");
    assert_eq!(fmt_playlists_count(1), "1 плейлист");
    assert_eq!(fmt_playlists_count(4), "4 плейлиста");
    assert_eq!(fmt_playlists_count(14), "14 плейлистов");
    assert_eq!(fmt_friends_count(1), "1 друг");
    assert_eq!(fmt_friends_count(3), "3 друга");
    assert_eq!(fmt_friends_count(5), "5 друзей");
    assert_eq!(fmt_gb(2 * 1024 * 1024 * 1024), "2,0 ГБ");

    assert_eq!(
        extract_captcha_success_token(r#"{"response":{"success_token":"tok-123"}}"#).as_deref(),
        Some("tok-123")
    );
    assert_eq!(
        extract_captcha_success_token(r#"{"success_token":"flat-456"}"#).as_deref(),
        Some("flat-456")
    );
    assert!(extract_captcha_success_token(r#"{"response":{"other":1}}"#).is_none());

    assert_eq!(
        parse_captcha_nav_outcome("https://id.vk.ru/not_robot_captcha#success=1&success_token=abc"),
        Some(CaptchaNavOutcome::Success(Some("abc".into())))
    );
    assert_eq!(
        parse_captcha_nav_outcome("https://id.vk.ru/not_robot_captcha?cancel=1"),
        Some(CaptchaNavOutcome::Cancel)
    );
}

#[test]
fn router_history_and_panel_toggles_behave_consistently() {
    let mut router = RouterState::default();
    assert_eq!(router.current(), &Route::Home);
    assert!(!router.can_go_back());

    assert!(router.navigate(Route::Library));
    assert!(router.navigate(Route::Friends));
    assert!(router.navigate(Route::FriendMusic(99)));
    assert_eq!(router.current().nav_highlight(), Some("friends"));

    assert_eq!(router.go_back(), Some(Route::Friends));
    assert_eq!(router.go_back(), Some(Route::Library));
    assert!(router.can_go_forward());
    assert_eq!(router.go_forward(), Some(Route::Friends));

    router.toggle_panel(PanelTab::Lyrics);
    assert!(!router.panel_open);
    router.toggle_panel(PanelTab::Queue);
    assert!(router.panel_open);
    assert_eq!(router.panel_tab, PanelTab::Queue);

    router.toggle_stage_tab(PanelTab::Lyrics);
    assert!(router.stage_view.is_none());
    router.toggle_stage_tab(PanelTab::Queue);
    assert_eq!(router.stage_view, Some(PanelTab::Queue));
}

fn sample_synced_lyrics(count: usize) -> TrackLyrics {
    let lines = (0..count)
        .map(|i| SyncedLine {
            id: i,
            begin: (i as i64 + 1) * 3_000,
            end: (i as i64 + 2) * 3_000,
            text: Some(format!("Synced lyric line number {i} with enough words to read")),
        })
        .collect();
    TrackLyrics {
        content: LyricsContent::Synced(lines),
        source: LyricsSource::Vk,
    }
}

#[test]
fn active_synced_lyric_index_tracks_timestamps_and_intro() {
    let LyricsContent::Synced(lines) = sample_synced_lyrics(4).content else {
        panic!();
    };
    assert_eq!(active_synced_lyric_index(&lines, 0), None);
    assert_eq!(active_synced_lyric_index(&lines, 2_999), None);
    assert_eq!(active_synced_lyric_index(&lines, 3_000), Some(0));
    assert_eq!(active_synced_lyric_index(&lines, 5_999), Some(0));
    assert_eq!(active_synced_lyric_index(&lines, 6_000), Some(1));
    assert_eq!(active_synced_lyric_index(&lines, 99_000), Some(3));
}

#[test]
fn lyrics_scroll_state_pauses_on_manual_scroll_and_resumes_after_cooldown_when_playing() {
    use gpui_kit::{ScrollHandle, point, px};

    let handle = ScrollHandle::new();
    let mut state = LyricsScrollState::default();
    let t0 = Instant::now();

    assert!(!state.prepare_render(&handle, "t1", Some(10), true, false, t0));
    assert!(state.snap_next);

    state.target_y = Some(-300.0);
    state.snap_next = false;
    handle.set_offset(point(px(0.0), px(-300.0)));
    state.last_offset_y = Some(-300.0);

    state.mark_manual_scroll(t0, -50.0);
    handle.set_offset(point(px(0.0), px(-50.0)));
    assert!(state.manual_paused);

    let after_cooldown = t0 + LYRICS_MANUAL_PAUSE + Duration::from_secs(5);
    assert!(!state.prepare_render(&handle, "t1", Some(10), false, false, after_cooldown));
    assert!(state.manual_paused);
    assert_eq!(f32::from(handle.offset().y), -50.0);

    let before_cooldown = t0 + Duration::from_secs(1);
    assert!(!state.prepare_render(&handle, "t1", Some(10), true, false, before_cooldown));
    assert!(state.manual_paused);

    state.target_y = Some(-300.0);
    let needs_frame = state.prepare_render(&handle, "t1", Some(10), true, false, after_cooldown);
    assert!(!state.manual_paused);
    assert!(needs_frame);
    let moved_y = f32::from(handle.offset().y);
    assert!(moved_y < -50.0 && moved_y > -300.0, "moved_y={moved_y}");
}

mod ui_harness {
    use super::*;
    use gpui_kit::{
        AppContext as _, Bounds, Point, TestAppContext, WindowBounds, WindowOptions, px, size,
        test::TestWindowExt,
    };

    #[gpui_kit::test]
    fn renders_all_routes_player_surfaces_modals_and_clean_sign_out(cx: &mut TestAppContext) {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(temp.path());
        let persistence = Arc::new(FileSessionPersistence {
            path: temp.path().join("session.json"),
        });

        cx.update(|app| {
            let _ = vively::design::icons::VivelyAssets;
            gpui_kit::init(app);
            vively::ui::install_shortcuts_and_menus(app);
        });

        let (win_handle, view) = cx.update(|app| {
            let bounds = Bounds {
                origin: Point::default(),
                size: size(px(1380.0), px(880.0)),
            };
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                app,
                |window, cx| {
                    cx.new(|cx| {
                        VivelyApp::new_with_config(
                            paths.clone(),
                            persistence.clone(),
                            true,
                            window,
                            cx,
                        )
                        .unwrap()
                    })
                },
            )
            .unwrap()
        });

        cx.update_window(win_handle, |_, window, cx| {
            window.render_frame(cx);

            view.update(cx, |app, cx| {
                assert!(app.core.session.is_none());
                let session = Session {
                    user_id: 42,
                    access_token: "test-token".into(),
                    common_token: Some("common".into()),
                    trusted_hash: None,
                    expires_at: None,
                    user: Some(VkUser {
                        id: 42,
                        first_name: "Кирилл".into(),
                        last_name: "Морозов".into(),
                        photo_200: None,
                        screen_name: Some("kmorozov".into()),
                        phone: None,
                    }),
                };
                app.core.session = Some(session);
                app.core.home_state = LoadState::Loaded;
                app.core.library_state = LoadState::Loaded;
                app.core.playlists_state = LoadState::Loaded;
                app.core.friends_state = LoadState::Loaded;

                let t1 = sample_audio(1, "Судно (Борис Рыжий)", "Molchat Doma");
                let t2 = sample_audio(2, "Кончится лето", "Кино");
                app.core.library_tracks = vec![t1.clone(), t2.clone()];
                app.core.library_total = 2;

                let pl = Playlist::new(10, 42, "Ночной фокус");
                app.core.playlists = vec![pl.clone()];
                app.core.playlist_details.insert(
                    pl.stable_id(),
                    PlaylistDetailData {
                        playlist: pl.clone(),
                        tracks: vec![t1.clone(), t2.clone()],
                        total_count: 2,
                        owner: None,
                        owner_audio_open: false,
                        loading_more: false,
                        more_failed: false,
                        state: LoadState::Loaded,
                    },
                );

                let friend = Friend {
                    id: 77,
                    first_name: "Илья".into(),
                    last_name: "Сафонов".into(),
                    photo_base: None,
                    is_closed: Some(false),
                    can_access_closed: Some(true),
                    deactivated: None,
                };
                app.core.friends = vec![friend.clone()];
                app.core.friends_total = 1;
                app.core.friend_pages.insert(
                    77,
                    FriendMusicData {
                        friend,
                        tracks: vec![t1.clone()],
                        total_tracks: 1,
                        playlists: vec![pl.clone()],
                        total_playlists: 1,
                        playlists_error: None,
                        playlists_loading_more: false,
                        loading_more: false,
                        more_failed: false,
                        state: LoadState::Loaded,
                    },
                );

                app.core.queue.replace(vec![t1, t2], 0, PlaybackOrigin::Library);
                app.core.playing_from = "Моя музыка".into();
                cx.notify();
            });
            window.render_frame(cx);

            let routes = vec![
                Route::Home,
                Route::Search,
                Route::Library,
                Route::Collection("42_10".into()),
                Route::Artist("molchat_doma".into()),
                Route::Friends,
                Route::FriendMusic(77),
                Route::Downloads,
                Route::Section(SectionTarget::Id {
                    section_id: "rec".into(),
                    title: "Совпадение вкусов".into(),
                }),
            ];

            for route in routes {
                view.update(cx, |app, cx| {
                    app.router.navigate(route);
                    cx.notify();
                });
                window.render_frame(cx);
            }

            let modals = vec![
                ActiveModal::MixTune {
                    mix_id: "common".into(),
                },
                ActiveModal::BulkDownloadLibrary,
                ActiveModal::PlaylistEditor { editing: None },
                ActiveModal::Settings {
                    tab: SettingsTab::General,
                },
                ActiveModal::Settings {
                    tab: SettingsTab::Downloads,
                },
                ActiveModal::Settings {
                    tab: SettingsTab::Account,
                },
                ActiveModal::Auth,
            ];

            for modal in modals {
                view.update(cx, |app, cx| {
                    app.router.modal = Some(modal);
                    cx.notify();
                });
                window.render_frame(cx);
            }

            view.update(cx, |app, cx| {
                app.router.modal = None;
                app.router.stage_open = true;
                app.router.panel_tab = PanelTab::Queue;
                app.toggle_theme_quick(window, cx);
            });
            window.render_frame(cx);

            view.update(cx, |app, cx| {
                app.sign_out(window, cx);
                assert!(app.core.session.is_none());
                assert!(app.core.library_tracks.is_empty());
                assert!(app.core.playlists.is_empty());
                assert!(app.core.queue.current().is_none());
                assert_eq!(app.router.current(), &Route::Home);
            });
            window.render_frame(cx);
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn synced_lyrics_scroll_in_stage_and_panel_and_respect_manual_scroll(cx: &mut TestAppContext) {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(temp.path());
        let persistence = Arc::new(FileSessionPersistence {
            path: temp.path().join("session.json"),
        });

        cx.update(|app| {
            let _ = vively::design::icons::VivelyAssets;
            gpui_kit::init(app);
            vively::ui::install_shortcuts_and_menus(app);
        });

        let (win_handle, view) = cx.update(|app| {
            let bounds = Bounds {
                origin: Point::default(),
                size: size(px(1380.0), px(880.0)),
            };
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                app,
                |window, cx| {
                    cx.new(|cx| {
                        VivelyApp::new_with_config(
                            paths.clone(),
                            persistence.clone(),
                            true,
                            window,
                            cx,
                        )
                        .unwrap()
                    })
                },
            )
            .unwrap()
        });

        let stage_offset_19 = cx.update_window(win_handle, |_, window, cx| {
            let t1 = sample_audio(101, "Synced Song", "Artist");
            let t2 = sample_audio(102, "Next Song", "Artist");
            let lyr1 = sample_synced_lyrics(40);
            let lyr2 = sample_synced_lyrics(40);

            view.update(cx, |app, cx| {
                app.core.session = Some(Session {
                    user_id: 42,
                    access_token: "tok".into(),
                    common_token: None,
                    trusted_hash: None,
                    expires_at: None,
                    user: None,
                });
                app.core.queue
                    .replace(vec![t1.clone(), t2.clone()], 0, PlaybackOrigin::Library);
                app.core.lyrics_states
                    .insert(t1.stable_id(), LyricsLoadState::Loaded(Some(lyr1)));
                app.core.lyrics_states
                    .insert(t2.stable_id(), LyricsLoadState::Loaded(Some(lyr2)));
                app.core.current_elapsed = 61.0;
                app.router.panel_open = true;
                app.router.panel_tab = PanelTab::Lyrics;
                app.router.stage_open = false;
                cx.notify();
            });

            window.render_frame(cx);
            window.simulate_next_frame(cx);
            window.render_frame(cx);

            let (panel_offset, panel_max, panel_vp_h) = view.read_with(cx, |app, _| {
                (
                    f32::from(app.right_scroll.offset().y),
                    f32::from(app.right_scroll.max_offset().y),
                    f32::from(app.right_scroll.bounds().size.height),
                )
            });
            assert!(panel_vp_h > 300.0 && panel_vp_h < 880.0, "panel_vp_h={panel_vp_h}");
            assert!(panel_max > 200.0, "panel_max={panel_max}");
            assert!(panel_offset < -100.0, "panel should have scrolled to line 19, got {panel_offset}");

            view.update(cx, |app, cx| {
                app.router.stage_open = true;
                app.router.stage_view = Some(PanelTab::Lyrics);
                cx.notify();
            });
            window.render_frame(cx);
            window.simulate_next_frame(cx);
            window.render_frame(cx);

            let (stage_offset_19, stage_max, stage_vp_h, stage_target_19) =
                view.read_with(cx, |app, _| {
                    (
                        f32::from(app.stage_scroll.offset().y),
                        f32::from(app.stage_scroll.max_offset().y),
                        f32::from(app.stage_scroll.bounds().size.height),
                        app.stage_lyrics_scroll.borrow().target_y,
                    )
                });
            assert!(stage_vp_h > 300.0 && stage_vp_h <= 700.0, "stage_vp_h={stage_vp_h}");
            assert!(stage_max > 400.0, "stage_max={stage_max}");
            assert!(
                stage_offset_19 < -200.0,
                "stage should have snapped to line 19, got {stage_offset_19}"
            );
            assert!(
                (stage_offset_19 - stage_target_19.unwrap()).abs() < 1.0,
                "stage offset {stage_offset_19} should match target {:?}",
                stage_target_19
            );

            stage_offset_19
        })
        .unwrap();

        let settle = |cx: &mut TestAppContext| {
            cx.update_window(win_handle, |_, window, cx| {
                window.render_frame(cx);
            })
            .unwrap();
            for _ in 0..120 {
                let ran = cx
                    .update_window(win_handle, |_, window, cx| window.simulate_next_frame(cx))
                    .unwrap();
                if ran == 0 {
                    break;
                }
                cx.background_executor
                    .advance_clock(Duration::from_millis(16));
                cx.update_window(win_handle, |_, window, cx| {
                    window.render_frame(cx);
                })
                .unwrap();
            }
        };

        cx.update_window(win_handle, |_, _, cx| {
            view.update(cx, |app, cx| {
                app.core.current_elapsed = 82.0;
                cx.notify();
            });
        })
        .unwrap();
        settle(cx);

        let stage_offset_26 = cx
            .update_window(win_handle, |_, _, cx| {
                view.read_with(cx, |app, _| f32::from(app.stage_scroll.offset().y))
            })
            .unwrap();
        assert!(
            stage_offset_26 < stage_offset_19 - 100.0,
            "stage should have scrolled further down: line19={stage_offset_19}, line26={stage_offset_26}"
        );

        cx.update_window(win_handle, |_, window, cx| {
            view.update(cx, |app, cx| {
                app.stage_scroll.set_offset(gpui_kit::point(px(0.0), px(0.0)));
                app.core.current_elapsed = 88.0;
                cx.notify();
            });
            window.render_frame(cx);
            window.simulate_next_frame(cx);
            window.render_frame(cx);
        })
        .unwrap();

        let (manual_paused, after_manual_y) = cx
            .update_window(win_handle, |_, _, cx| {
                view.read_with(cx, |app, _| {
                    (
                        app.stage_lyrics_scroll.borrow().manual_paused,
                        f32::from(app.stage_scroll.offset().y),
                    )
                })
            })
            .unwrap();
        assert!(manual_paused, "manual scroll should pause auto-follow");
        assert!(
            after_manual_y.abs() < 1.0,
            "manual scroll position should be preserved, got {after_manual_y}"
        );

        cx.update_window(win_handle, |_, _, cx| {
            view.update(cx, |app, cx| {
                app.seek_to(88.0, cx);
            });
        })
        .unwrap();
        settle(cx);

        let (paused_after_seek, y_after_seek) = cx
            .update_window(win_handle, |_, _, cx| {
                view.read_with(cx, |app, _| {
                    (
                        app.stage_lyrics_scroll.borrow().manual_paused,
                        f32::from(app.stage_scroll.offset().y),
                    )
                })
            })
            .unwrap();
        assert!(!paused_after_seek, "seek should resume auto-follow");
        assert!(
            y_after_seek < stage_offset_26,
            "seek to line 28 should scroll past line 26: line26={stage_offset_26}, line28={y_after_seek}"
        );

        cx.update_window(win_handle, |_, _, cx| {
            view.update(cx, |app, cx| {
                app.seek_to(0.5, cx);
            });
        })
        .unwrap();
        settle(cx);

        let y_intro = cx
            .update_window(win_handle, |_, _, cx| {
                view.read_with(cx, |app, _| f32::from(app.stage_scroll.offset().y))
            })
            .unwrap();
        assert!(
            y_intro.abs() < 1.0,
            "seeking to intro should scroll back to 0.0, got {y_intro}"
        );

        cx.update_window(win_handle, |_, _, cx| {
            view.update(cx, |app, cx| {
                app.seek_to(88.0, cx);
            });
        })
        .unwrap();
        settle(cx);

        cx.update_window(win_handle, |_, _, cx| {
            view.update(cx, |app, cx| {
                app.core.queue.select(1);
                app.core.current_elapsed = 1.0;
                app.reset_lyrics_scroll();
                cx.notify();
            });
        })
        .unwrap();
        settle(cx);

        let y_next_track = cx
            .update_window(win_handle, |_, _, cx| {
                view.read_with(cx, |app, _| f32::from(app.stage_scroll.offset().y))
            })
            .unwrap();
        assert!(
            y_next_track.abs() < 1.0,
            "track change should reset scroll to top, got {y_next_track}"
        );
    }

    #[gpui_kit::test]
    fn track_loading_indicator_shows_while_buffering_and_cancels_cleanly(cx: &mut TestAppContext) {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(temp.path());
        let persistence = Arc::new(FileSessionPersistence {
            path: temp.path().join("session.json"),
        });

        cx.update(|app| {
            let _ = vively::design::icons::VivelyAssets;
            gpui_kit::init(app);
            vively::ui::install_shortcuts_and_menus(app);
        });

        let (win_handle, view) = cx.update(|app| {
            let bounds = Bounds {
                origin: Point::default(),
                size: size(px(1280.0), px(820.0)),
            };
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                app,
                |window, cx| {
                    cx.new(|cx| {
                        VivelyApp::new_with_config(
                            paths.clone(),
                            persistence.clone(),
                            true,
                            window,
                            cx,
                        )
                        .unwrap()
                    })
                },
            )
            .unwrap()
        });

        cx.update_window(win_handle, |_, window, cx| {
            let track = sample_audio(201, "liar", "@russianmafiaextra");

            view.update(cx, |app, cx| {
                app.core.session = Some(Session {
                    user_id: 42,
                    access_token: "tok".into(),
                    common_token: None,
                    trusted_hash: None,
                    expires_at: None,
                    user: None,
                });
                app.core.library_tracks = vec![track.clone()];
                app.core.queue.replace(vec![track], 0, PlaybackOrigin::Library);
                app.core.current_elapsed = 29.0;
                app.core.pending_restore_pos = Some(29.0);
                app.core.track_loading = true;
                app.router.navigate(Route::Library);
                cx.notify();
            });

            window.render_frame(cx);

            view.update(cx, |app, cx| {
                app.router.stage_open = true;
                cx.notify();
            });
            window.render_frame(cx);

            view.update(cx, |app, cx| {
                app.toggle_play_pause(cx);
                assert!(!app.core.track_loading);
                assert_eq!(app.core.pending_restore_pos, Some(29.0));
            });
            window.render_frame(cx);
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn playlist_creator_scrolls_picker_and_draft_list_without_leaking_to_background(
        cx: &mut TestAppContext,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(temp.path());
        let persistence = Arc::new(FileSessionPersistence {
            path: temp.path().join("session.json"),
        });

        cx.update(|app| {
            let _ = vively::design::icons::VivelyAssets;
            gpui_kit::init(app);
            vively::ui::install_shortcuts_and_menus(app);
        });

        let (win_handle, view) = cx.update(|app| {
            let bounds = Bounds {
                origin: Point::default(),
                size: size(px(1280.0), px(820.0)),
            };
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                app,
                |window, cx| {
                    cx.new(|cx| {
                        VivelyApp::new_with_config(
                            paths.clone(),
                            persistence.clone(),
                            true,
                            window,
                            cx,
                        )
                        .unwrap()
                    })
                },
            )
            .unwrap()
        });

        cx.update_window(win_handle, |_, window, cx| {
            let tracks: Vec<_> = (1..=80)
                .map(|i| sample_audio(i, &format!("Liked Track {i}"), &format!("Artist {i}")))
                .collect();

            view.update(cx, |app, cx| {
                app.core.session = Some(Session {
                    user_id: 42,
                    access_token: "tok".into(),
                    common_token: None,
                    trusted_hash: None,
                    expires_at: None,
                    user: None,
                });
                app.core.library_tracks = tracks.clone();
                app.core.library_total = 240;
                app.core.library_can_more = true;
                app.core.library_state = vively::state::models::LoadState::Loaded;
                app.router.navigate(Route::Library);
                cx.notify();
            });
            window.render_frame(cx);

            let bg_max = view.read_with(cx, |app, _| f32::from(app.canvas_scroll.max_offset().y));
            assert!(
                bg_max > 1000.0,
                "background Library page should overflow so ScrollBounce is active, got {bg_max}"
            );

            view.update(cx, |app, cx| {
                app.open_playlist_editor(None, None, window, cx);
                app.add_editor_tracks_bulk(tracks[..25].to_vec(), cx);
            });
            window.render_frame(cx);

            let (picker_bounds, picker_max, draft_bounds, draft_max) =
                view.read_with(cx, |app, _| {
                    let s = app.editor_picker_scroll.0.borrow();
                    (
                        s.base_handle.bounds(),
                        f32::from(s.base_handle.max_offset().y),
                        app.editor_draft_scroll.bounds(),
                        f32::from(app.editor_draft_scroll.max_offset().y),
                    )
                });
            assert!(
                f32::from(picker_bounds.size.height) > 300.0 && picker_max > 1000.0,
                "picker should have scrollable bounds: bounds={picker_bounds:?} max={picker_max}"
            );
            assert!(
                f32::from(draft_bounds.size.height) > 150.0 && draft_max > 300.0,
                "draft list should have scrollable bounds: bounds={draft_bounds:?} max={draft_max}"
            );

            let picker_center = gpui_kit::point(
                picker_bounds.origin.x + picker_bounds.size.width * 0.5,
                picker_bounds.origin.y + picker_bounds.size.height * 0.5,
            );
            window.dispatch_event(
                gpui_kit::PlatformInput::Scroll(gpui_kit::ScrollEvent {
                    position: picker_center,
                    delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.0), px(-240.0))),
                    touch_phase: gpui_kit::TouchPhase::Moved,
                    ..Default::default()
                }),
                cx,
            );
            window.render_frame(cx);

            let (picker_y, bg_y) = view.read_with(cx, |app, _| {
                (
                    f32::from(app.editor_picker_scroll.0.borrow().base_handle.offset().y),
                    f32::from(app.canvas_scroll.offset().y),
                )
            });
            assert!(
                picker_y < -150.0,
                "picker should scroll smoothly under trackpad/wheel input, got {picker_y}"
            );
            assert!(
                bg_y.abs() < 0.5,
                "modal backdrop must occlude background canvas scroll, got bg_y={bg_y}"
            );

            let draft_center = gpui_kit::point(
                draft_bounds.origin.x + draft_bounds.size.width * 0.5,
                draft_bounds.origin.y + draft_bounds.size.height * 0.5,
            );
            window.dispatch_event(
                gpui_kit::PlatformInput::Scroll(gpui_kit::ScrollEvent {
                    position: draft_center,
                    delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.0), px(-180.0))),
                    touch_phase: gpui_kit::TouchPhase::Moved,
                    ..Default::default()
                }),
                cx,
            );
            window.render_frame(cx);

            let (draft_y, bg_y_after) = view.read_with(cx, |app, _| {
                (
                    f32::from(app.editor_draft_scroll.offset().y),
                    f32::from(app.canvas_scroll.offset().y),
                )
            });
            assert!(
                draft_y < -100.0,
                "selected tracks pane should scroll independently, got {draft_y}"
            );
            assert!(
                bg_y_after.abs() < 0.5,
                "background canvas must remain stationary, got {bg_y_after}"
            );
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn playlist_creator_filters_reorders_bulk_toggles_and_renders_compact_and_edit_modes(
        cx: &mut TestAppContext,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(temp.path());
        let persistence = Arc::new(FileSessionPersistence {
            path: temp.path().join("session.json"),
        });

        cx.update(|app| {
            let _ = vively::design::icons::VivelyAssets;
            gpui_kit::init(app);
            vively::ui::install_shortcuts_and_menus(app);
        });

        let (win_handle, view) = cx.update(|app| {
            let bounds = Bounds {
                origin: Point::default(),
                size: size(px(740.0), px(660.0)),
            };
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                app,
                |window, cx| {
                    cx.new(|cx| {
                        VivelyApp::new_with_config(
                            paths.clone(),
                            persistence.clone(),
                            true,
                            window,
                            cx,
                        )
                        .unwrap()
                    })
                },
            )
            .unwrap()
        });

        cx.update_window(win_handle, |_, window, cx| {
            let t1 = sample_audio(1, "Alpha Song", "Band A");
            let t2 = sample_audio(2, "Beta Song", "Band B");
            let t3 = sample_audio(3, "Gamma Song", "Band C");

            view.update(cx, |app, cx| {
                app.core.session = Some(Session {
                    user_id: 42,
                    access_token: "tok".into(),
                    common_token: None,
                    trusted_hash: None,
                    expires_at: None,
                    user: None,
                });
                app.core.library_tracks = vec![t1.clone(), t2.clone(), t3.clone()];
                app.core.library_total = 3;
                app.core.library_state = vively::state::models::LoadState::Loaded;
                app.open_playlist_editor(None, Some(t1.clone()), window, cx);
                app.add_editor_tracks_bulk(vec![t1.clone(), t2.clone(), t3.clone()], cx);
                assert_eq!(app.core.editor_draft_tracks.len(), 3);

                app.move_editor_draft_track(2, 0, cx);
                assert_eq!(app.core.editor_draft_tracks[0].id, 3);
                assert_eq!(app.core.editor_draft_tracks[1].id, 1);

                app.remove_editor_tracks_bulk(&[t1.clone(), t3.clone()], cx);
                assert_eq!(app.core.editor_draft_tracks.len(), 1);
                assert_eq!(app.core.editor_draft_tracks[0].id, 2);

                app.on_editor_picker_query_change("beta".into(), false, cx);
                assert_eq!(app.editor_picker_query, "beta");
            });
            window.render_frame(cx);

            view.update(cx, |app, cx| {
                app.editor_picker_open = true;
                cx.notify();
            });
            window.render_frame(cx);

            view.update(cx, |app, cx| {
                app.search_editor_query_in_vk(window, cx);
                assert_eq!(
                    app.editor_source,
                    vively::state::router::EditorTrackSource::Search
                );
                assert_eq!(app.editor_picker_query, "beta");
                app.clear_editor_draft_tracks(cx);
                assert!(app.core.editor_draft_tracks.is_empty());
                app.clear_editor_picker_query(window, cx);
                assert!(app.editor_picker_query.is_empty());
            });
            window.render_frame(cx);

            view.update(cx, |app, cx| {
                let existing = Playlist::new(77, 42, "Existing Playlist");
                app.open_playlist_editor(Some(existing), None, window, cx);
                app.toggle_theme_quick(window, cx);
            });
            window.render_frame(cx);
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn full_player_occludes_background_scroll_and_new_modals_and_actions_work(
        cx: &mut TestAppContext,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(temp.path());
        let persistence = Arc::new(FileSessionPersistence {
            path: temp.path().join("session.json"),
        });

        cx.update(|app| {
            let _ = vively::design::icons::VivelyAssets;
            gpui_kit::init(app);
            vively::ui::install_shortcuts_and_menus(app);
        });

        let (win_handle, view) = cx.update(|app| {
            let bounds = Bounds {
                origin: Point::default(),
                size: size(px(1280.0), px(820.0)),
            };
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                app,
                |window, cx| {
                    cx.new(|cx| {
                        VivelyApp::new_with_config(
                            paths.clone(),
                            persistence.clone(),
                            true,
                            window,
                            cx,
                        )
                        .unwrap()
                    })
                },
            )
            .unwrap()
        });

        cx.update_window(win_handle, |_, window, cx| {
            let tracks: Vec<_> = (1..=60)
                .map(|i| sample_audio(i, &format!("Track {i}"), &format!("Artist {i}")))
                .collect();
            let playlists: Vec<_> = (1..=25)
                .map(|i| Playlist::new(i, 42, format!("Playlist {i}")))
                .collect();

            view.update(cx, |app, cx| {
                app.core.session = Some(Session {
                    user_id: 42,
                    access_token: "tok".into(),
                    common_token: None,
                    trusted_hash: None,
                    expires_at: None,
                    user: None,
                });
                app.core.library_tracks = tracks.clone();
                app.core.library_total = 120;
                app.core.library_can_more = true;
                app.core.library_state = vively::state::models::LoadState::Loaded;
                app.core.playlists = playlists.clone();
                app.core.playlists_state = vively::state::models::LoadState::Loaded;
                app.core.queue
                    .replace(tracks[..30].to_vec(), 0, PlaybackOrigin::Library);
                app.router.navigate(Route::Library);
                app.scrub_hover_ratio = Some(0.45);
                cx.notify();
            });
            window.render_frame(cx);

            view.update(cx, |app, cx| {
                app.router.stage_open = true;
                app.router.stage_view = Some(PanelTab::Queue);
                app.stage_scrub_hover_ratio = Some(0.5);
                cx.notify();
            });
            window.render_frame(cx);

            let (stage_q_bounds, stage_q_max) = view.read_with(cx, |app, _| {
                (
                    app.stage_queue_scroll.bounds(),
                    f32::from(app.stage_queue_scroll.max_offset().y),
                )
            });
            assert!(
                stage_q_max > 200.0,
                "stage queue should overflow for scrolling, got {stage_q_max}"
            );

            let center = gpui_kit::point(
                stage_q_bounds.origin.x + stage_q_bounds.size.width * 0.5,
                stage_q_bounds.origin.y + stage_q_bounds.size.height * 0.5,
            );
            window.dispatch_event(
                gpui_kit::PlatformInput::Scroll(gpui_kit::ScrollEvent {
                    position: center,
                    delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.0), px(-220.0))),
                    touch_phase: gpui_kit::TouchPhase::Moved,
                    ..Default::default()
                }),
                cx,
            );
            window.render_frame(cx);

            let (stage_q_y, bg_y) = view.read_with(cx, |app, _| {
                (
                    f32::from(app.stage_queue_scroll.offset().y),
                    f32::from(app.canvas_scroll.offset().y),
                )
            });
            assert!(
                stage_q_y < -100.0,
                "stage queue should scroll cleanly, got {stage_q_y}"
            );
            assert!(
                bg_y.abs() < 0.5,
                "immersive stage must occlude background canvas scroll, got {bg_y}"
            );

            view.update(cx, |app, cx| {
                app.core.current_elapsed = 20.0;
                app.seek_relative(15.0, cx);
                assert!((app.core.current_elapsed - 35.0).abs() < 0.01);
                app.seek_relative(-50.0, cx);
                assert!(app.core.current_elapsed.abs() < 0.01);

                app.save_queue_as_playlist(window, cx);
                assert!(!app.router.stage_open);
                assert_eq!(app.core.editor_draft_tracks.len(), 30);

                app.clear_upcoming_queue(cx);
                assert!(app.core.queue.upcoming().is_empty());
            });
            window.render_frame(cx);

            view.update(cx, |app, cx| {
                app.request_delete_playlist(playlists[0].clone(), cx);
                assert!(matches!(
                    app.router.modal,
                    Some(ActiveModal::ConfirmDeletePlaylist { .. })
                ));
            });
            window.render_frame(cx);

            view.update(cx, |app, cx| {
                app.open_add_tracks_to_playlist(playlists[0].clone(), window, cx);
                assert!(matches!(
                    app.router.modal,
                    Some(ActiveModal::AddTracksToPlaylist { .. })
                ));
            });
            window.render_frame(cx);

            view.update(cx, |app, cx| {
                app.open_add_to_playlist_modal(tracks[0].clone(), window, cx);
                assert!(matches!(
                    app.router.modal,
                    Some(ActiveModal::AddToPlaylist { .. })
                ));
            });
            window.render_frame(cx);

            let add_pl_max =
                view.read_with(cx, |app, _| f32::from(app.add_pl_scroll.max_offset().y));
            assert!(
                add_pl_max > 200.0,
                "AddToPlaylist modal list should be scrollable, got {add_pl_max}"
            );
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn renders_loading_error_and_empty_states_across_all_tabs_and_sidebar(
        cx: &mut TestAppContext,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(temp.path());
        let persistence = Arc::new(FileSessionPersistence {
            path: temp.path().join("session.json"),
        });

        cx.update(|app| {
            let _ = vively::design::icons::VivelyAssets;
            gpui_kit::init(app);
            vively::ui::install_shortcuts_and_menus(app);
        });

        let (win_handle, view) = cx.update(|app| {
            let bounds = Bounds {
                origin: Point::default(),
                size: size(px(1280.0), px(820.0)),
            };
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                app,
                |window, cx| {
                    cx.new(|cx| {
                        VivelyApp::new_with_config(
                            paths.clone(),
                            persistence.clone(),
                            true,
                            window,
                            cx,
                        )
                        .unwrap()
                    })
                },
            )
            .unwrap()
        });

        cx.update_window(win_handle, |_, window, cx| {
            let friend = vively_core::models::Friend {
                id: 77,
                first_name: "Мяу".into(),
                last_name: "Мяу".into(),
                photo_base: None,
                is_closed: Some(false),
                can_access_closed: Some(true),
                deactivated: None,
            };
            let empty_pl = Playlist::new(501, 42, "Пустой плейлист");
            let pl_key = empty_pl.stable_id();

            for state in [
                vively::state::models::LoadState::Loading,
                vively::state::models::LoadState::Failed("network error".into()),
                vively::state::models::LoadState::Loaded,
            ] {
                view.update(cx, |app, cx| {
                    app.core.session = Some(Session {
                        user_id: 42,
                        access_token: "tok".into(),
                        common_token: None,
                        trusted_hash: None,
                        expires_at: None,
                        user: None,
                    });
                    app.core.home_state = state.clone();
                    app.core.library_state = state.clone();
                    app.core.friends_state = state.clone();
                    app.core.playlists_state = state.clone();
                    app.core.search_state = state.clone();
                    app.core.friend_pages.insert(
                        77,
                        vively::state::models::FriendMusicData {
                            friend: friend.clone(),
                            tracks: Vec::new(),
                            total_tracks: 0,
                            playlists: Vec::new(),
                            total_playlists: 0,
                            playlists_error: None,
                            playlists_loading_more: false,
                            loading_more: false,
                            more_failed: false,
                            state: state.clone(),
                        },
                    );
                    app.core.playlist_details.insert(
                        pl_key.clone(),
                        vively::state::models::PlaylistDetailData {
                            playlist: empty_pl.clone(),
                            tracks: Vec::new(),
                            total_count: 0,
                            owner: None,
                            owner_audio_open: false,
                            loading_more: false,
                            more_failed: false,
                            state: state.clone(),
                        },
                    );
                    cx.notify();
                });

                for route in [
                    Route::Home,
                    Route::Search,
                    Route::Library,
                    Route::Friends,
                    Route::FriendMusic(77),
                    Route::Downloads,
                    Route::Collection(pl_key.clone()),
                    Route::Artist("artist-1".into()),
                    Route::Section(SectionTarget::Id {
                        section_id: "sec-1".into(),
                        title: "Раздел".into(),
                    }),
                ] {
                    view.update(cx, |app, cx| {
                        app.router.navigate(route);
                        cx.notify();
                    });
                    window.render_frame(cx);
                }
            }

            view.update(cx, |app, cx| {
                app.core.search_query = "nothing".into();
                app.core.search_state = vively::state::models::LoadState::Loaded;
                app.router.navigate(Route::Search);
                app.toggle_theme_quick(window, cx);
            });
            window.render_frame(cx);

            let (qr_size, qr_modules) =
                encode_qr_matrix("https://qr.vk.ru/ca?q=AbC123").unwrap();
            assert!(qr_size >= 21);
            assert_eq!(qr_modules.len(), qr_size * qr_size);
            assert!(qr_modules.iter().any(|&dark| dark));
            assert!(qr_modules.iter().any(|&dark| !dark));

            for stage in [
                vively::state::models::QrStage::Loading,
                vively::state::models::QrStage::Ready {
                    url: "https://qr.vk.ru/ca?q=AbC123".into(),
                    size: qr_size,
                    modules: qr_modules.clone(),
                },
                vively::state::models::QrStage::Scanned {
                    url: "https://qr.vk.ru/ca?q=AbC123".into(),
                    size: qr_size,
                    modules: qr_modules,
                    user_name: Some("Тест Тестов".into()),
                },
                vively::state::models::QrStage::ConfirmCode {
                    url: "https://qr.vk.ru/ca?q=AbC123".into(),
                },
                vively::state::models::QrStage::Expired,
                vively::state::models::QrStage::Declined,
            ] {
                view.update(cx, |app, cx| {
                    app.core.session = None;
                    app.core.auth.step = vively::state::models::AuthStep::Qr(stage);
                    cx.notify();
                });
                window.render_frame(cx);
            }
        })
        .unwrap();
    }
}
