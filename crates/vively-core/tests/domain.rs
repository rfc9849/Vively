use std::collections::BTreeMap;

use serde_json::json;
use url::Url;
use vively_core::{
    Error,
    catalog::{
        HomeBlockKind, MusicContent, TrackStyle, catalog_sections, home_blocks, search_items,
    },
    client::{ResponseStyle, decode_response, form_encode, params},
    library::StreamMixSession,
    lyrics::{LyricsContent, LyricsSource, SyncedLine, TrackLyrics, VkLyricsResponse, clean_genius_lyrics, parse_musixmatch_macro, parse_musixmatch_subtitles, pick_best_lyrics},
    models::*,
    playback::{
        PlaybackOrigin, PlaybackPosition, PlaybackQueue, PlaybackReporter, PlaybackSnapshot,
        RepeatMode,
    },
    session::{DeviceProfile, Session},
    settings::AppSettings,
};

fn audio(id: i64) -> Audio {
    serde_json::from_value(json!({"id": id, "owner_id": 7, "title": "Track", "artist": "Artist", "duration": 125, "is_explicit": false, "url": "https://media.invalid/track.m3u8"})).unwrap()
}

#[test]
fn audio_identity_artwork_and_restrictions() {
    let mut track = audio(1);
    track.access_key = Some("key".into());
    track.album = Some(Album {
        thumb: Some(Thumb {
            photo_135: Some(Url::parse("https://media.invalid/small.jpg").unwrap()),
            photo_1200: Some(Url::parse("https://media.invalid/large.jpg").unwrap()),
            ..Default::default()
        }),
        ..Default::default()
    });
    assert_eq!(track.full_id(), "7_1");
    assert_eq!(track.stable_id(), "7_1_key");
    assert_eq!(track.formatted_duration(), "2:05");
    assert!(track.artwork_url().unwrap().path().ends_with("large.jpg"));
    assert!(track.thumbnail_url().unwrap().path().ends_with("small.jpg"));
    assert!(track.is_playable());
    track.content_restricted = Some(1);
    assert!(!track.is_playable());
    track.url = Some(String::new());
    assert!(track.stream_url().is_none());
    assert_eq!(
        serde_json::from_value::<Audio>(serde_json::to_value(&track).unwrap()).unwrap(),
        track
    );
}

#[test]
fn lists_skip_invalid_items_and_keep_server_count() {
    let list: VkList<Audio> = serde_json::from_value(
        json!({"count": 4, "items": [audio(1), json!({"id": 2}), audio(3), json!(false)]}),
    )
    .unwrap();
    assert_eq!(list.count, 4);
    assert_eq!(list.items.iter().map(|a| a.id).collect::<Vec<_>>(), [1, 3]);
    let list: VkList<Audio> = serde_json::from_value(json!({"items": false})).unwrap();
    assert_eq!(list.count, 0);
    assert!(list.items.is_empty());
    let list: VkList<Audio> =
        serde_json::from_value(json!({"items": null, "count": null})).unwrap();
    assert!(list.items.is_empty());
}

#[test]
fn playlists_accept_string_owner_and_malformed_optional_fields() {
    let playlist: Playlist = serde_json::from_value(json!({
        "id": 2, "owner_id": "-7", "type": 50, "title": 42, "photo": false, "permissions": [],
        "original_owner": {"first_name": "First", "last_name": "Last"}, "owner": {"name": "Group"}
    }))
    .unwrap();
    assert_eq!(playlist.owner_id, -7);
    assert_eq!(playlist.kind, PlaylistKind::Unknown);
    assert_eq!(playlist.title, "");
    assert!(playlist.photo.is_none());
    assert_eq!(playlist.author_name(), Some("Group"));
    assert_eq!(playlist.original_owner_name.as_deref(), Some("First Last"));
    assert!(!playlist.is_editable());
    let restored: Playlist =
        serde_json::from_value(serde_json::to_value(&playlist).unwrap()).unwrap();
    assert_eq!(restored, playlist);
    for (kind, expected) in [
        (json!(0), PlaylistKind::Playlist),
        (json!(1), PlaylistKind::Album),
        (json!("album"), PlaylistKind::Album),
        (json!("ugc"), PlaylistKind::Playlist),
        (json!("generated"), PlaylistKind::Playlist),
        (json!(""), PlaylistKind::Unknown),
    ] {
        let playlist: Playlist = serde_json::from_value(json!({"id": 1, "type": kind})).unwrap();
        assert_eq!(playlist.kind, expected);
    }
}

#[test]
fn album_byline_and_playlist_permissions() {
    let playlist: Playlist = serde_json::from_value(
        json!({"id": 2, "type": 1, "year": 2024, "main_artists": [{"id": "a", "name": "Artist"}]}),
    )
    .unwrap();
    assert_eq!(playlist.byline(), "Artist · 2024");
    let playlist: Playlist = serde_json::from_value(
        json!({"id": 2, "type": 0, "permissions": {"edit": true}, "subtitle": "Subtitle"}),
    )
    .unwrap();
    assert!(playlist.is_editable());
    assert_eq!(playlist.byline(), "Subtitle");
}

#[test]
fn catalog_links_parse_playlist_album_and_artist_destinations() {
    let link: CatalogLink = serde_json::from_value(
        json!({"id": "l", "title": "Album", "url": "https://vk.com/music/album/-7_8_key"}),
    )
    .unwrap();
    let MusicDestination::Playlist(p) = link.destination().unwrap() else {
        panic!();
    };
    assert_eq!(p.full_id(), "-7_8");
    assert_eq!(p.access_key.as_deref(), Some("key"));
    assert!(p.is_album());
    let link: CatalogLink = serde_json::from_value(
        json!({"id": "a", "title": "Artist", "url": "https://vk.com/artist/artist-name"}),
    )
    .unwrap();
    assert_eq!(link.artist_domain(), Some("artist-name"));
    assert!(link.destination().is_none());
}

#[test]
fn catalog_blocks_keep_headers_order_and_first_duplicate() {
    let first = audio(1);
    let mut duplicate = first.clone();
    duplicate.title = "Second".into();
    let blocks: Vec<CatalogBlock> = serde_json::from_value(json!([
        {"layout": {"title": "Tracks"}, "actions": [{"section_id": "all"}]},
        {"data_type": "music_audios", "layout": {"name": "slider"}, "audios_ids": ["7_1", "missing"]},
        {"data_type": "music_playlists", "layout": {"title": "Empty"}, "playlists_ids": ["missing"]}
    ])).unwrap();
    let sections = catalog_sections(&[first, duplicate], &[], &blocks);
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].title, "Tracks");
    assert_eq!(sections[0].show_all_section_id.as_deref(), Some("all"));
    assert_eq!(sections[0].track_style, TrackStyle::Columns);
    let MusicContent::Tracks(tracks) = &sections[0].content else {
        panic!();
    };
    assert_eq!(tracks[0].title, "Track");
}

#[test]
fn home_sorts_mixes_and_recommendations_and_attaches_show_all() {
    let response: HomeResponse = serde_json::from_value(json!({
        "audios": [audio(1)],
        "audio_stream_mixes": [{"id": "my_music", "title": "Library"}, {"id": "common", "titles": {"common_state": "Mix"}}],
        "recommended_playlists": [{"id": 2, "owner_id": 7, "percentage": 0.754}],
        "section": {"blocks": [
            {"data_type": "music_audios", "layout": {"title": "Tracks"}, "audios_ids": ["7_1"]},
            {"data_type": "music_recommended_playlists", "layout": {"title": "Recommended"}, "playlists_ids": ["7_2"]},
            {"data_type": "action", "actions": [{"section_id": "more"}]},
            {"data_type": "audio_stream_mixes", "audio_stream_mixes_ids": ["my_music", "common"]}
        ]}
    })).unwrap();
    let blocks = home_blocks(&response);
    assert_eq!(blocks.len(), 3);
    let HomeBlockKind::StreamMixes(mixes) = &blocks[0].kind else {
        panic!();
    };
    assert_eq!(mixes.len(), 1);
    assert_eq!(mixes[0].display_title(), "Mix");
    let HomeBlockKind::Recommended {
        show_all_section_id,
        playlists,
        ..
    } = &blocks[1].kind
    else {
        panic!();
    };
    assert_eq!(show_all_section_id.as_deref(), Some("more"));
    assert_eq!(playlists[0].match_text().as_deref(), Some("75%"));
    assert!(matches!(blocks[2].kind, HomeBlockKind::Music(_)));
}

#[test]
fn search_resolves_artist_links() {
    let response: SearchResponse = serde_json::from_value(json!({
        "links": [{"id": "a", "title": "Artist", "url": "https://vk.com/artist/artist-name"}],
        "catalog": {"sections": [{"blocks": [
            {"layout": {"title": "Artists"}},
            {"data_type": "links", "links_ids": ["a", "missing"]}
        ]}]}
    }))
    .unwrap();
    let items = search_items(&response);
    assert_eq!(items.len(), 1);
}

#[test]
fn response_envelopes_promote_api_oauth_captcha_and_validation_errors() {
    assert_eq!(
        decode_response::<i64>(br#"{"response":1}"#, ResponseStyle::Method).unwrap(),
        1
    );
    assert!(
        matches!(decode_response::<i64>(br#"{"error":{"error_code":5,"error_msg":"Expired"}}"#, ResponseStyle::Method), Err(Error::Api(e)) if e.is_refreshable())
    );
    assert!(matches!(
        decode_response::<i64>(
            br#"{"error":"need_validation","validation_type":"2fa_app","validation_sid":"sid"}"#,
            ResponseStyle::OAuth
        ),
        Err(Error::Validation(_))
    ));
    assert!(matches!(decode_response::<i64>(br#"{"error":"need_captcha","captcha_sid":"sid","redirect_uri":"https://captcha.invalid/"}"#, ResponseStyle::OAuth), Err(Error::Captcha(_))));
    assert!(matches!(
        decode_response::<i64>(br#"{}"#, ResponseStyle::Method),
        Err(Error::Decode { .. })
    ));
    assert!(matches!(
        decode_response::<i64>(br#"{"response":false}"#, ResponseStyle::Method),
        Err(Error::Decode { .. })
    ));
    assert!(
        decode_response::<serde_json::Value>(br#"{"response":null}"#, ResponseStyle::Method)
            .unwrap()
            .is_null()
    );
    let failed_execute = br#"{"response":false,"execute_errors":[{"method":"audio.get","error_code":201,"error_msg":"Access denied"}]}"#;
    assert!(matches!(
        decode_response::<serde_json::Value>(failed_execute, ResponseStyle::Method),
        Err(Error::Api(e)) if e.code == 201
    ));
    assert!(matches!(
        decode_response::<Vec<i64>>(
            br#"{"response":{"items":false},"execute_errors":[{"method":"audio.get","error_code":15,"error_msg":"Access denied"}]}"#,
            ResponseStyle::Method
        ),
        Err(Error::Api(e)) if e.code == 15
    ));
    assert_eq!(
        decode_response::<i64>(
            br#"{"response":3,"execute_errors":[{"method":"users.get","error_code":18,"error_msg":"Deleted"}]}"#,
            ResponseStyle::Method
        )
        .unwrap(),
        3
    );
}

#[test]
fn form_encoding_escapes_utf8_and_reserved_characters() {
    assert_eq!(
        form_encode(&params([("q", "a +&=/@,Ж".into())])),
        "q=a%20%2B%26%3D%2F%40%2C%D0%96"
    );
}

#[test]
fn lyrics_use_synced_text_and_preserve_interludes() {
    let response: VkLyricsResponse =
        serde_json::from_value(json!({"lyrics": {"text": ["fallback"], "timestamps": [
            {"begin": 0, "end": 100, "interlude": true},
            {"begin": 100, "end": 200, "line": " line "},
            {"begin": 200, "line": "invalid"}
        ]}}))
        .unwrap();
    let lyrics = TrackLyrics::from_vk(response).unwrap();
    let LyricsContent::Synced(lines) = lyrics.content else {
        panic!();
    };
    assert_eq!(lines.len(), 2);
    assert!(lines[0].is_interlude());
    assert_eq!(lines[1].text.as_deref(), Some("line"));
    let response: VkLyricsResponse = serde_json::from_value(json!({"lyrics": {"text": ["", " first ", "", " last ", ""], "timestamps": [{"begin": 0, "end": 20, "interlude": true}]}})).unwrap();
    assert_eq!(
        TrackLyrics::from_vk(response).unwrap().content,
        LyricsContent::Plain(vec!["first".into(), "".into(), "last".into()])
    );
    assert!(TrackLyrics::from_plain("\n   \n", LyricsSource::Genius).is_none());
}

#[test]
fn genius_lyrics_strip_section_tags_and_normalize_stanzas() {
    let raw = "[Текст песни «Пример»]\n\n [Verse 1: redize]\n Line one\n Line two\n \n \n [Chorus: redize]\n Hook line\n  [Bridge: pooterdabooter]\n Bridge line [Guitar Solo]\n [Outro:\n Artist A &\n Artist B]\n Final line\n\n  ";
    let lines = clean_genius_lyrics(raw);
    assert_eq!(
        lines,
        vec![
            "Line one",
            "Line two",
            "",
            "Hook line",
            "",
            "Bridge line",
            "",
            "Final line",
        ]
    );
    assert!(TrackLyrics::from_plain("[Instrumental]\n[Outro]", LyricsSource::Genius).is_none());
}

#[test]
fn musixmatch_subtitles_parse_mxm_json_and_lrc_and_macro() {
    let mxm_body = json!([
        {"text": "First line", "time": {"total": 1.25}},
        {"text": "", "time": {"total": 4.5}},
        {"text": "", "time": {"total": 6.0}},
        {"text": "Second line", "time": {"total": 10.0}},
        {"text": "", "time": {"total": 15.0}}
    ])
    .to_string();
    let lines = parse_musixmatch_subtitles(&mxm_body, 20_000).unwrap();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].begin, 1250);
    assert_eq!(lines[0].end, 4500);
    assert_eq!(lines[0].text.as_deref(), Some("First line"));
    assert!(lines[1].is_interlude());
    assert_eq!(lines[1].begin, 4500);
    assert_eq!(lines[1].end, 10_000);
    assert_eq!(lines[2].begin, 10_000);
    assert_eq!(lines[2].end, 15_000);
    assert_eq!(lines[2].text.as_deref(), Some("Second line"));

    let lrc = "[00:02.5] One\n[00:05.25] Two\n[01:02.125] Three";
    let lrc_lines = parse_musixmatch_subtitles(lrc, 70_000).unwrap();
    assert_eq!(lrc_lines.len(), 3);
    assert_eq!(lrc_lines[0].begin, 2500);
    assert_eq!(lrc_lines[1].begin, 5250);
    assert_eq!(lrc_lines[2].begin, 62_125);
    assert_eq!(lrc_lines[2].end, 70_000);

    let instrumental_macro = json!({
        "message": {
            "header": {"status_code": 200},
            "body": {
                "macro_calls": {
                    "matcher.track.get": {"message": {"header": {"status_code": 200}, "body": {"track": {"instrumental": 1}}}},
                    "track.lyrics.get": {"message": {"header": {"status_code": 200}, "body": {"lyrics": {"lyrics_body": "ignored"}}}}
                }
            }
        }
    });
    assert!(parse_musixmatch_macro(&instrumental_macro, 120_000).is_none());

    let plain_macro = json!({
        "message": {
            "header": {"status_code": 200},
            "body": {
                "macro_calls": {
                    "track.subtitles.get": {"message": {"header": {"status_code": 404}, "body": []}},
                    "track.lyrics.get": {"message": {"header": {"status_code": 200}, "body": {"lyrics": {"instrumental": 0, "lyrics_body": "Alpha\n\nBeta\n******* This Lyrics is NOT for Commercial use *******\n(1234)"}}}}
                }
            }
        }
    });
    assert_eq!(
        parse_musixmatch_macro(&plain_macro, 120_000),
        Some(TrackLyrics {
            content: LyricsContent::Plain(vec!["Alpha".into(), "".into(), "Beta".into()]),
            source: LyricsSource::Musixmatch,
        })
    );
}

#[test]
fn lyrics_priority_orders_synced_above_vk_above_genius_above_musixmatch_plain() {
    let synced_line = vec![SyncedLine {
        id: 0,
        begin: 0,
        end: 1000,
        text: Some("hi".into()),
    }];
    let vk_synced = TrackLyrics {
        content: LyricsContent::Synced(synced_line.clone()),
        source: LyricsSource::Vk,
    };
    let mxm_synced = TrackLyrics {
        content: LyricsContent::Synced(synced_line),
        source: LyricsSource::Musixmatch,
    };
    let vk_plain = TrackLyrics {
        content: LyricsContent::Plain(vec!["vk".into()]),
        source: LyricsSource::Vk,
    };
    let genius_plain = TrackLyrics {
        content: LyricsContent::Plain(vec!["genius".into()]),
        source: LyricsSource::Genius,
    };
    let mxm_plain = TrackLyrics {
        content: LyricsContent::Plain(vec!["mxm".into()]),
        source: LyricsSource::Musixmatch,
    };

    assert_eq!(
        pick_best_lyrics([Some(mxm_synced.clone()), Some(vk_synced.clone())]),
        Some(vk_synced)
    );
    assert_eq!(
        pick_best_lyrics([Some(vk_plain.clone()), Some(mxm_synced.clone()), Some(genius_plain.clone())]),
        Some(mxm_synced)
    );
    assert_eq!(
        pick_best_lyrics([Some(genius_plain.clone()), Some(vk_plain.clone()), Some(mxm_plain.clone())]),
        Some(vk_plain)
    );
    assert_eq!(
        pick_best_lyrics([Some(mxm_plain.clone()), Some(genius_plain.clone())]),
        Some(genius_plain)
    );
    assert_eq!(pick_best_lyrics([None, Some(mxm_plain.clone())]), Some(mxm_plain));
}

#[test]
fn queue_shuffle_restores_order_and_keeps_manual_insert_origin() {
    let mut queue = PlaybackQueue::default();
    assert!(queue.replace(
        vec![audio(1), audio(2), audio(3)],
        1,
        PlaybackOrigin::Library
    ));
    queue.set_shuffled(true);
    assert_eq!(queue.current().unwrap().id, 2);
    queue.play_next(audio(4));
    assert_eq!(queue.upcoming()[0].id, 4);
    assert_eq!(queue.origin(&audio(4)), PlaybackOrigin::Other);
    assert_eq!(queue.origin(&audio(2)), PlaybackOrigin::Library);
    queue.set_shuffled(false);
    assert_eq!(
        queue.tracks().iter().map(|a| a.id).collect::<Vec<_>>(),
        [1, 2, 4, 3]
    );
    queue.repeat_mode = RepeatMode::One;
    assert_eq!(queue.next_index(), Some(1));
    queue.repeat_mode = RepeatMode::All;
    queue.select(3);
    assert_eq!(queue.next_index(), Some(0));
    assert_eq!(queue.previous(4.0), 3);
    assert_eq!(queue.previous(2.0), 2);
    queue.remove_upcoming(&[0, usize::MAX]);
    assert_eq!(queue.tracks().len(), 3);
}

#[test]
fn queue_snapshot_limits_tracks_and_rejects_other_accounts() {
    let mut queue = PlaybackQueue::default();
    queue.replace((0..1000).map(audio).collect(), 800, PlaybackOrigin::Library);
    let snapshot = queue.snapshot(7, 1000).unwrap();
    assert_eq!(snapshot.tracks.len(), 300);
    assert_eq!(snapshot.current_index, 100);
    assert!(PlaybackQueue::restore(snapshot.clone(), 8, 1001).is_err());
    let restored = PlaybackQueue::restore(snapshot, 7, 3000).unwrap();
    assert_eq!(restored.current().unwrap().id, 800);
    assert!(restored.is_stale(restored.current().unwrap()));
}

#[test]
fn reporting_tracks_start_pause_resume_and_switch() {
    let mut queue = PlaybackQueue::default();
    queue.replace(vec![audio(1), audio(2)], 0, PlaybackOrigin::Other);
    let mut reporter = PlaybackReporter::default();
    let started = reporter.sync(&queue, true, 0.0, true, 100);
    assert_eq!(started[0].reason, "new");
    reporter.update_position(queue.current().unwrap(), 12.4);
    let paused = reporter.sync(&queue, false, 12.4, true, 120);
    assert_eq!(paused[0].reason, "pause");
    assert_eq!(paused[0].duration, 12);
    let resumed = reporter.sync(&queue, true, 12.4, true, 130);
    assert_eq!(resumed[0].reason, "continue");
    assert_eq!(resumed[0].start_time, 100);
    queue.advance();
    let switched = reporter.sync(&queue, true, 0.0, true, 140);
    assert_eq!(switched.len(), 2);
    assert_eq!(switched[1].previous_audio_id.as_deref(), Some("7_1"));
    let serialized = serde_json::to_value(&switched[1]).unwrap();
    assert_eq!(serialized["e"], "music_start_playback");
    assert_eq!(serialized["repeat"], "none");
    assert!(reporter.sync(&queue, true, 1.0, false, 150).is_empty());
}

#[test]
fn settings_device_and_queue_files_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let mut settings = AppSettings::load(&path).unwrap();
    assert_eq!(settings.crossfade_duration, 0.0);
    assert!(settings.downloads_wifi_only);
    settings.crossfade_duration = 50.0;
    settings.save(&path).unwrap();
    assert_eq!(AppSettings::load(&path).unwrap().crossfade_duration, 12.0);
    let path = dir.path().join("device.json");
    let profile = DeviceProfile::load_or_create(&path).unwrap();
    assert_eq!(profile.device_id.len(), 16);
    assert!(profile.device_id.bytes().all(|b| b.is_ascii_alphanumeric()));
    assert_eq!(DeviceProfile::load_or_create(&path).unwrap(), profile);
    let mut queue = PlaybackQueue::default();
    queue.replace(vec![audio(1)], 0, PlaybackOrigin::Other);
    let path = dir.path().join("queue.json");
    let snapshot = queue.snapshot(7, 100).unwrap();
    snapshot.save(&path).unwrap();
    assert_eq!(PlaybackSnapshot::load(&path).unwrap().unwrap(), snapshot);
    let position = PlaybackPosition {
        track_id: audio(1).stable_id(),
        time: 15.5,
    };
    assert_eq!(position.time_for(&audio(1)), 15.5);
    assert_eq!(position.time_for(&audio(2)), 0.0);
}

#[test]
fn mix_options_drop_empty_categories_and_session_expiry_uses_unix_time() {
    let session = StreamMixSession::tuned(
        "common",
        BTreeMap::from([
            ("genre".into(), vec!["rock".into()]),
            ("mood".into(), vec![]),
        ]),
    )
    .unwrap();
    assert_eq!(
        session.options_json.as_deref(),
        Some(r#"{"genre":["rock"]}"#)
    );
    let session = Session {
        user_id: 7,
        access_token: "test-token".into(),
        common_token: None,
        trusted_hash: None,
        expires_at: Some(100),
        user: None,
    };
    assert!(!session.is_expired_at(99));
    assert!(session.is_expired_at(100));

    let swift_json = r#"{"userID":4242,"accessToken":"vk1.a.tok","commonToken":"vk1.c.tok","trustedHash":"th","expiresAt":816740000.25,"user":{"id":4242,"first_name":"Кирилл","last_name":"Морозов","photo_200":"https://sun9.userapi.com/u.jpg","screen_name":"kmorozov"}}"#;
    let imported: Session = serde_json::from_str(swift_json).unwrap();
    assert_eq!(imported.user_id, 4242);
    assert_eq!(imported.access_token, "vk1.a.tok");
    assert_eq!(imported.common_token.as_deref(), Some("vk1.c.tok"));
    assert_eq!(imported.trusted_hash.as_deref(), Some("th"));
    assert_eq!(imported.expires_at, Some(816_740_000 + 978_307_200));
    assert_eq!(imported.user.as_ref().unwrap().full_name(), "Кирилл Морозов");

    let section_fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../vively/docs/section.json");
    if let Ok(bytes) = std::fs::read(&section_fixture) {
        let parsed: CatalogSectionResponse = decode_response(&bytes, ResponseStyle::Method).unwrap();
        let sections = catalog_sections(
            parsed.audios.as_deref().unwrap_or_default(),
            parsed.playlists.as_deref().unwrap_or_default(),
            &parsed.section.blocks,
        );
        assert!(!sections.is_empty());
    }

    let mix_fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../vively/docs/mix.json");
    if let Ok(bytes) = std::fs::read(&mix_fixture) {
        let mut val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let section = val["response"]["catalog"]["sections"][0].clone();
        val["response"]["section"] = section;
        let home_bytes = serde_json::to_vec(&val).unwrap();
        let home: HomeResponse = decode_response(&home_bytes, ResponseStyle::Method).unwrap();
        let blocks = home_blocks(&home);
        assert!(!blocks.is_empty());
    }
}

#[test]
fn phone_normalization_filename_split_and_search_history() {
    use vively_core::{
        auth::{filename_artist_title, normalize_phone},
        settings::SearchHistory,
    };

    assert_eq!(normalize_phone("+7 (999) 155-34-83"), "79991553483");
    assert_eq!(normalize_phone("8 (999) 155-34-83"), "79991553483");
    assert_eq!(normalize_phone("+86 138 0013 8000"), "8613800138000");

    assert_eq!(
        filename_artist_title("Кино - Спокойная ночь"),
        ("Кино".into(), "Спокойная ночь".into())
    );
    assert_eq!(
        filename_artist_title("standalone_track"),
        (String::new(), "standalone_track".into())
    );

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.json");
    let mut history = SearchHistory::default();
    assert!(!history.record("   "));
    assert!(history.record("Molchat Doma"));
    assert!(history.record("Кино"));
    assert!(history.record("molchat doma"));
    assert_eq!(history.entries, vec!["molchat doma", "Кино"]);
    for idx in 0..12 {
        history.record(&format!("q{idx}"));
    }
    assert_eq!(history.entries.len(), 10);
    assert_eq!(history.entries[0], "q11");
    history.save(&path).unwrap();
    assert_eq!(SearchHistory::load(&path).unwrap(), history);
}
