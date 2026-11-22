use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use aes::cipher::{BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use rusqlite::Connection;
use serde_json::json;
use url::Url;
use vively_core::{
    AudioService, VkClient,
    config::VkConfig,
    downloads::*,
    media::*,
    models::{Audio, Playlist},
    playback::PlaybackOrigin,
    session::unix_now,
    settings::AppSettings,
};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};

fn audio(id: i64) -> Audio {
    serde_json::from_value(json!({"id":id,"owner_id":7,"title":"Track","artist":"Artist","duration":125,"is_explicit":false,"url":"https://media.invalid/track.m3u8"})).unwrap()
}

fn packet(pid: u16, payload: &[u8]) -> Vec<u8> {
    assert!(payload.len() <= 183);
    let mut packet = vec![
        0x47,
        0x40 | ((pid >> 8) as u8 & 31),
        pid as u8,
        0x30,
        (183 - payload.len()) as u8,
    ];
    packet.resize(188 - payload.len(), 0);
    packet.extend_from_slice(payload);
    packet
}

fn ts_headers() -> Vec<u8> {
    let pat = [0, 0, 0xb0, 13, 0, 1, 0xc1, 0, 0, 0, 1, 0xe1, 0, 0, 0, 0, 0];
    let pmt = [
        0, 2, 0xb0, 18, 0, 1, 0xc1, 0, 0, 0xe1, 1, 0xf0, 0, 3, 0xe1, 1, 0xf0, 0, 0, 0, 0, 0,
    ];
    [packet(0, &pat), packet(256, &pmt)].concat()
}

fn ts_audio(bytes: &[u8]) -> Vec<u8> {
    let mut pes = vec![0, 0, 1, 0xc0, 0, 0, 0x80, 0, 0];
    pes.extend_from_slice(bytes);
    packet(257, &pes)
}

#[test]
fn playlists_resolve_relative_keys_sequences_and_explicit_ivs() {
    let base = Url::parse("https://media.invalid/path/track.m3u8").unwrap();
    let segments = parse_playlist("#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:42\n#EXT-X-KEY:METHOD=AES-128,URI=\"../key,a\"\none.ts\n#EXT-X-KEY:METHOD=AES-128,URI=\"key\",IV=0x123\ntwo.ts\n#EXT-X-KEY:METHOD=NONE\nthree.ts", &base).unwrap();
    assert_eq!(segments.len(), 3);
    assert_eq!(
        segments[0].key_url.as_ref().unwrap().as_str(),
        "https://media.invalid/key,a"
    );
    assert_eq!(segments[0].iv, sequence_iv(42));
    assert_eq!(segments[1].iv[14..], [1, 0x23]);
    assert!(segments[2].key_url.is_none());
    assert_eq!(segments[2].iv, sequence_iv(44));
    assert!(parse_playlist("expired", &base).is_err());
    assert!(parse_playlist("#EXTM3U", &base).is_err());
    assert!(
        parse_playlist(
            "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"key\"\na.ts",
            &base
        )
        .is_err()
    );
    assert!(
        parse_playlist(
            "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=100\nmedia.m3u8",
            &base
        )
        .is_err()
    );
}

#[test]
fn aes_segments_decrypt_and_reject_invalid_keys_and_padding() {
    let key = [7u8; 16];
    let iv = sequence_iv(42);
    let plaintext = b"MP3 stream bytes";
    let encrypted = cbc::Encryptor::<aes::Aes128>::new_from_slices(&key, &iv)
        .unwrap()
        .encrypt_padded_vec_mut::<Pkcs7>(plaintext);
    assert_eq!(
        decrypt_aes128_cbc(&encrypted, &key, &iv).unwrap(),
        plaintext
    );
    assert!(decrypt_aes128_cbc(&encrypted, &[7; 15], &iv).is_err());
    assert!(decrypt_aes128_cbc(&encrypted[..3], &key, &iv).is_err());
}

#[test]
fn ts_demux_finds_mp3_and_checks_packet_bounds() {
    let payload = [0xff, 0xfb, 0x90, 0, 1, 2, 3, 4];
    let ts = [ts_headers(), ts_audio(&payload)].concat();
    assert_eq!(extract_mp3(&ts).unwrap(), payload);
    for length in 0..600 {
        let bytes: Vec<_> = (0..length)
            .map(|i| ((i * 31 + length) % 256) as u8)
            .collect();
        let _ = extract_mp3(&bytes);
    }
    for value in [0, 1, 127, 255] {
        let mut bytes = vec![value; 188 * 3];
        bytes[0] = 0x47;
        bytes[188] = 0x47;
        let _ = extract_mp3(&bytes);
    }
}

#[test]
fn id3_round_trips_unicode_and_strips_tags_without_audio() {
    let bytes = build_id3("Трек", "Björk", Some("Album"), Some(&[1, 2, 3])).unwrap();
    assert_eq!(
        read_id3(&bytes),
        Id3Tags {
            artist: Some("Björk".into()),
            title: Some("Трек".into())
        }
    );
    assert!(strip_leading_id3(&bytes).is_empty());
    let combined = [bytes, vec![0xff, 0xfb, 1, 2]].concat();
    assert_eq!(strip_leading_id3(&combined), [0xff, 0xfb, 1, 2]);
    for length in 0..300 {
        let bytes = vec![0xff; length];
        assert_eq!(read_id3(&bytes), Id3Tags::default());
        assert_eq!(strip_leading_id3(&bytes), bytes);
    }
}

#[tokio::test]
async fn encrypted_hls_downloads_keep_segment_order_and_write_tagged_mp3() {
    let server = MockServer::start().await;
    let key = [3; 16];
    let first = [ts_headers(), ts_audio(&[0xff, 0xfb, 1, 2])].concat();
    let second = ts_audio(&[3, 4, 5, 6]);
    for (index, plain) in [(0, first), (1, second)] {
        let iv = sequence_iv(index + 5);
        let bytes = cbc::Encryptor::<aes::Aes128>::new_from_slices(&key, &iv)
            .unwrap()
            .encrypt_padded_vec_mut::<Pkcs7>(&plain);
        Mock::given(path(format!("/{index}.ts")))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(bytes)
                    .set_delay(Duration::from_millis(if index == 0 { 10 } else { 0 })),
            )
            .expect(1)
            .mount(&server)
            .await;
    }
    Mock::given(path("/key"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(key.to_vec()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/track.m3u8")).respond_with(ResponseTemplate::new(200).set_body_string("#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:5\n#EXT-X-KEY:METHOD=AES-128,URI=\"key\"\n0.ts\n1.ts\n#EXT-X-ENDLIST")).expect(1).mount(&server).await;
    let downloader = Mp3StreamDownloader::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("track.mp3");
    let progress = Mutex::new(Vec::new());
    let tag = Mp3Tag {
        title: "Track".into(),
        artist: "Artist".into(),
        album: None,
        artwork: None,
    };
    let bytes = downloader
        .download(
            format!("{}/track.m3u8", server.uri()).parse().unwrap(),
            &destination,
            &tag,
            &|v| progress.lock().unwrap().push(v),
        )
        .await
        .unwrap();
    let file = std::fs::read(destination).unwrap();
    assert_eq!(file.len() as u64, bytes);
    assert_eq!(read_id3(&file).title.as_deref(), Some("Track"));
    assert_eq!(strip_leading_id3(&file), [0xff, 0xfb, 1, 2, 3, 4, 5, 6]);
    assert_eq!(*progress.lock().unwrap(), vec![0.5, 1.0]);
}

fn mark_ready(store: &OfflineStore, id: i64, size: usize) {
    let full_id = format!("7_{id}");
    std::fs::write(store.paths.track_file(&full_id).unwrap(), vec![0; size]).unwrap();
    Connection::open(store.paths.database())
        .unwrap()
        .execute(
            "UPDATE cached_track SET state='ready', byte_size=?1 WHERE full_id=?2",
            rusqlite::params![size, full_id],
        )
        .unwrap();
}

#[tokio::test]
async fn removing_an_active_download_cancels_its_http_work() {
    let server = MockServer::start().await;
    let started = Arc::new(tokio::sync::Notify::new());
    let notify = started.clone();
    Mock::given(path("/track.m3u8"))
        .respond_with(move |_: &wiremock::Request| {
            notify.notify_one();
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(60))
                .set_body_string("#EXTM3U\\nsegment.ts")
        })
        .expect(1)
        .mount(&server)
        .await;
    let mut config = VkConfig::android("test-secret").unwrap();
    config.api_host = server.uri().parse().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(OfflineStore::open(dir.path(), 7).unwrap());
    let mut track = audio(1);
    track.url = Some(format!("{}/track.m3u8", server.uri()));
    store
        .enqueue(&track, &DownloadRequest::default(), unix_now())
        .unwrap();
    let engine = DownloadEngine::new(
        store.clone(),
        AudioService::new(VkClient::new(config, "test-device").unwrap()),
    )
    .unwrap();
    let settings = AppSettings::default();
    let protected = HashSet::new();
    let network = NetworkState {
        status: NetworkStatus::Online(NetworkInterface::Wifi),
        ..Default::default()
    };
    let events = Mutex::new(Vec::new());
    let progress = |e| events.lock().unwrap().push(e);
    let (result, ()) = tokio::join!(
        engine.drain("test-token", network, &settings, &protected, &progress),
        async {
            started.notified().await;
            engine.remove("7_1").unwrap();
        }
    );
    let summary = result.unwrap();
    assert_eq!(summary.ready, 0);
    assert_eq!(summary.failed, 0);
    assert!(store.track("7_1").unwrap().is_none());
    assert!(!store.paths.track_file("7_1").unwrap().exists());
    assert!(
        events
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, DownloadEvent::Removed { .. }))
    );
}

#[test]
fn offline_store_persists_priorities_pins_and_collection_order() {
    let dir = tempfile::tempdir().unwrap();
    let store = OfflineStore::open(dir.path(), 7).unwrap();
    let automatic = DownloadRequest {
        priority: DownloadPriority::Auto,
        manual_pin: false,
        source: None,
        source_id: None,
        source_offset: None,
    };
    store.enqueue(&audio(1), &automatic, 100).unwrap();
    store
        .enqueue(&audio(1), &DownloadRequest::default(), 200)
        .unwrap();
    let record = store.track("7_1").unwrap().unwrap();
    assert!(record.manual_pin);
    assert_eq!(record.priority, DownloadPriority::Manual);
    assert_eq!(record.added_at, 100);
    assert_eq!(record.audio().unwrap(), audio(1));
    let playlist = Playlist::new(2, 7, "List");
    store
        .save_collection(&playlist, &[audio(3), audio(2)], 300)
        .unwrap();
    store
        .save_collection(&playlist, &[audio(2), audio(3)], 400)
        .unwrap();
    assert_eq!(
        store
            .collection_tracks("7_2")
            .unwrap()
            .iter()
            .map(|a| a.id)
            .collect::<Vec<_>>(),
        [2, 3]
    );
    assert_eq!(store.collections().unwrap()[0].playlist.title, "List");
    drop(store);
    let reopened = OfflineStore::open(dir.path(), 7).unwrap();
    assert_eq!(reopened.tracks().unwrap().len(), 3);
    assert!(
        OfflineStore::open(dir.path(), 8)
            .unwrap()
            .tracks()
            .unwrap()
            .is_empty()
    );
    reopened.remove("7_2").unwrap();
    assert_eq!(reopened.collection_tracks("7_2").unwrap().len(), 1);
    assert!(reopened.paths.track_file("../../escape").is_err());
}

#[test]
fn collection_writes_roll_back_on_database_failure() {
    let dir = tempfile::tempdir().unwrap();
    let store = OfflineStore::open(dir.path(), 7).unwrap();
    Connection::open(store.paths.database()).unwrap().execute_batch("CREATE TRIGGER reject_track BEFORE INSERT ON cached_track WHEN NEW.audio_id = 2 BEGIN SELECT RAISE(ABORT, 'rejected'); END;").unwrap();
    assert!(
        store
            .save_collection(&Playlist::new(1, 7, "List"), &[audio(1), audio(2)], 100)
            .is_err()
    );
    assert!(store.tracks().unwrap().is_empty());
    assert!(store.collections().unwrap().is_empty());
}

#[test]
fn cache_eviction_keeps_pins_collections_and_protected_tracks() {
    let dir = tempfile::tempdir().unwrap();
    let store = OfflineStore::open(dir.path(), 7).unwrap();
    let automatic = DownloadRequest {
        priority: DownloadPriority::Auto,
        manual_pin: false,
        source: None,
        source_id: None,
        source_offset: None,
    };
    for id in 1..=2 {
        store.enqueue(&audio(id), &automatic, id * 100).unwrap();
    }
    store
        .enqueue(&audio(3), &DownloadRequest::default(), 300)
        .unwrap();
    store
        .save_collection(&Playlist::new(9, 7, "List"), &[audio(4)], 400)
        .unwrap();
    for id in 1..=4 {
        mark_ready(&store, id, 100);
    }
    assert_eq!(
        store.usage().unwrap(),
        CacheUsage {
            total: 400,
            pinned: 200,
            track_count: 4
        }
    );
    let protected = HashSet::from(["7_2".into()]);
    assert_eq!(store.enforce_limit(150, &protected).unwrap(), ["7_1"]);
    assert_eq!(store.usage().unwrap().total, 300);
    assert_eq!(store.clear_auto_cache(&HashSet::new()).unwrap(), ["7_2"]);
    assert_eq!(store.usage().unwrap().total, 200);
    assert_eq!(
        store
            .ready_tracks()
            .unwrap()
            .iter()
            .map(|a| a.id)
            .collect::<Vec<_>>(),
        [3]
    );
    store.remove_collection("7_9").unwrap();
    assert_eq!(store.clear_auto_cache(&HashSet::new()).unwrap(), ["7_4"]);
    assert!(store.local_file(&audio(3)).unwrap().is_some());
    store.clear_all().unwrap();
    assert!(store.tracks().unwrap().is_empty());
}

#[test]
fn interrupted_jobs_and_missing_files_are_requeued() {
    let dir = tempfile::tempdir().unwrap();
    let store = OfflineStore::open(dir.path(), 7).unwrap();
    store
        .enqueue(&audio(1), &DownloadRequest::default(), 100)
        .unwrap();
    store
        .enqueue(&audio(2), &DownloadRequest::default(), 100)
        .unwrap();
    Connection::open(store.paths.database()).unwrap().execute_batch("UPDATE cached_track SET state='downloading' WHERE audio_id=1; UPDATE cached_track SET state='ready',byte_size=100 WHERE audio_id=2;").unwrap();
    store.recover_interrupted().unwrap();
    assert!(
        store
            .tracks()
            .unwrap()
            .iter()
            .all(|r| r.state == DownloadState::Queued)
    );
    assert_eq!(store.track("7_2").unwrap().unwrap().byte_size, 0);
}

#[test]
fn auto_cache_waits_for_listening_and_checks_network_at_the_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let store = OfflineStore::open(dir.path(), 7).unwrap();
    let settings = AppSettings {
        auto_cache_enabled: true,
        ..Default::default()
    };
    let wifi = NetworkState {
        status: NetworkStatus::Online(NetworkInterface::Wifi),
        ..Default::default()
    };
    let cellular = NetworkState {
        status: NetworkStatus::Online(NetworkInterface::Cellular),
        ..Default::default()
    };
    let mut cache = AutoCache::default();
    cache
        .track_started(&store, &audio(1), PlaybackOrigin::Library, &settings, 100)
        .unwrap();
    assert!(!cache.tick(&store, wifi, &settings, 114).unwrap());
    assert!(!cache.tick(&store, cellular, &settings, 115).unwrap());
    cache
        .track_started(&store, &audio(2), PlaybackOrigin::Other, &settings, 200)
        .unwrap();
    assert!(!cache.tick(&store, wifi, &settings, 220).unwrap());
    cache
        .track_started(&store, &audio(3), PlaybackOrigin::Library, &settings, 300)
        .unwrap();
    assert!(cache.tick(&store, wifi, &settings, 315).unwrap());
    let record = store.track("7_3").unwrap().unwrap();
    assert_eq!(record.priority, DownloadPriority::Auto);
    assert!(!record.manual_pin);
    assert!(record.url_is_fresh(315));
    assert!(!record.url_is_fresh(315 + 1800));
}

#[tokio::test]
async fn download_engine_writes_ready_files_and_emits_progress() {
    let server = MockServer::start().await;
    Mock::given(path("/track.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string("#EXTM3U\nsegment.ts"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/segment.ts"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes([ts_headers(), ts_audio(&[0xff, 0xfb, 1, 2])].concat()),
        )
        .expect(1)
        .mount(&server)
        .await;
    let mut config = VkConfig::android("test-secret").unwrap();
    config.api_host = server.uri().parse().unwrap();
    let client = VkClient::new(config, "test-device").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(OfflineStore::open(dir.path(), 7).unwrap());
    let mut track = audio(1);
    track.url = Some(format!("{}/track.m3u8", server.uri()));
    store
        .enqueue(&track, &DownloadRequest::default(), unix_now())
        .unwrap();
    let engine = DownloadEngine::new(store.clone(), AudioService::new(client)).unwrap();
    let events = Mutex::new(Vec::new());
    let summary = engine
        .drain(
            "test-token",
            NetworkState {
                status: NetworkStatus::Online(NetworkInterface::Wifi),
                ..Default::default()
            },
            &AppSettings::default(),
            &HashSet::new(),
            &|e| events.lock().unwrap().push(e),
        )
        .await
        .unwrap();
    assert_eq!(summary.ready, 1);
    assert_eq!(summary.failed, 0);
    let record = store.track("7_1").unwrap().unwrap();
    assert_eq!(record.state, DownloadState::Ready);
    let file = std::fs::read(store.local_file(&track).unwrap().unwrap()).unwrap();
    assert_eq!(record.byte_size, file.len() as u64);
    assert_eq!(strip_leading_id3(&file), [0xff, 0xfb, 1, 2]);
    assert!(
        events
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, DownloadEvent::Progress { fraction, .. } if *fraction == 1.0))
    );
}
