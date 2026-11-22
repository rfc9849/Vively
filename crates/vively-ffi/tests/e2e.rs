//! QR login -> library -> playback through the FFI actor, the same path the
//! Android app uses, against a fake VK server.

use std::{
    sync::{Mutex, mpsc},
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use vively_ffi::{
    CoreListener, FfiAuthStep, FfiConfig, FfiDirty, FfiEvent, FfiLoad, FfiNetwork,
    FfiNetworkStatus, VivelyCore,
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{path, path_regex},
};

struct Listener(Mutex<mpsc::Sender<(FfiDirty, Vec<FfiEvent>)>>);

impl CoreListener for Listener {
    fn on_update(&self, dirty: FfiDirty, events: Vec<FfiEvent>) {
        let _ = self.0.lock().unwrap().send((dirty, events));
    }
    fn network_state(&self) -> FfiNetwork {
        FfiNetwork { status: FfiNetworkStatus::Wifi, expensive: false, constrained: false }
    }
}

fn ok(v: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({ "response": v }))
}

fn wait_until(
    updates: &mpsc::Receiver<(FfiDirty, Vec<FfiEvent>)>,
    events: &mut Vec<FfiEvent>,
    timeout: Duration,
    mut done: impl FnMut(&[FfiEvent]) -> bool,
) {
    let deadline = Instant::now() + timeout;
    while !done(events) {
        let left = deadline.checked_duration_since(Instant::now()).expect("timed out");
        if let Ok((_, ev)) = updates.recv_timeout(left.min(Duration::from_millis(250))) {
            events.extend(ev);
        }
    }
}

#[test]
fn qr_login_then_library_and_playback_via_actor() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(MockServer::start());
    let uri = server.uri();
    let track = |id: i64| json!({"id": id, "owner_id": 42, "title": format!("t{id}"), "artist": "a",
        "duration": 26, "is_explicit": false, "url": format!("{uri}/track{id}.mp3")});
    let mut frame = vec![0xff, 0xfb, 0x90, 0x64];
    frame.resize(417, 0);
    rt.block_on(async {
        Mock::given(path("/oauth/get_anonym_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"token": "anon"})))
            .mount(&server).await;
        Mock::given(path("/method/auth.getAuthCode")).respond_with(ok(json!({
            "auth_code": "c", "auth_hash": "h", "auth_url": "https://vk.ru/qr", "expires_in": 600})))
            .mount(&server).await;
        Mock::given(path("/method/auth.checkAuthCode")).respond_with(ok(json!({
            "status": 2, "access_token": "tok", "user_id": 42, "expires_in": 0})))
            .mount(&server).await;
        Mock::given(path("/method/account.getProfileInfo")).respond_with(ok(json!({
            "id": 42, "first_name": "Тест", "last_name": "Тестов"})))
            .mount(&server).await;
        Mock::given(path("/method/audio.get"))
            .respond_with(ok(json!({"count": 2, "items": [track(1), track(2)]})))
            .mount(&server).await;
        Mock::given(path("/method/audio.getById")).respond_with(ok(json!([track(1)])))
            .mount(&server).await;
        Mock::given(path("/method/execute")).respond_with(ok(json!(1))).mount(&server).await;
        Mock::given(path_regex(r"^/track\d+\.mp3$"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "audio/mpeg")
                .set_body_bytes(frame.repeat(1000)))
            .mount(&server).await;
    });

    let root = tempfile::tempdir().unwrap();
    let (tx, updates) = mpsc::channel();
    let core = VivelyCore::new(
        FfiConfig {
            config_dir: root.path().join("config").display().to_string(),
            data_dir: root.path().join("data").display().to_string(),
            device_name: "Vively (test)".into(),
            simulated_audio: true,
            api_host: Some(uri.clone()),
        },
        Box::new(Listener(Mutex::new(tx))),
    )
    .unwrap();
    assert!(!core.auth().signed_in);

    let mut events = Vec::new();
    core.start_qr_auth();
    wait_until(&updates, &mut events, Duration::from_secs(10), |ev| {
        ev.iter().any(|e| matches!(e, FfiEvent::SignedIn))
    });
    let auth = core.auth();
    assert!(auth.signed_in);
    assert_eq!(auth.step, FfiAuthStep::Phone);

    let deadline = Instant::now() + Duration::from_secs(10);
    while core.library().state != FfiLoad::Loaded {
        assert!(Instant::now() < deadline, "library never loaded");
        let _ = updates.recv_timeout(Duration::from_millis(250));
    }
    let lib = core.library();
    assert_eq!(lib.tracks.len(), 2);
    assert!(lib.tracks.iter().all(|t| t.liked && t.playable));

    core.play_library(0);
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let pb = core.playback();
        if pb.playing && !pb.loading {
            assert_eq!(pb.current.unwrap().id, 1);
            assert!(pb.has_next);
            break;
        }
        assert!(Instant::now() < deadline, "playback never started: {pb:?}");
        let _ = updates.recv_timeout(Duration::from_millis(250));
    }
    core.pause();
    let deadline = Instant::now() + Duration::from_secs(3);
    while core.playback().playing {
        assert!(Instant::now() < deadline, "pause ignored");
        std::thread::sleep(Duration::from_millis(50));
    }

    core.sign_out();
    let deadline = Instant::now() + Duration::from_secs(3);
    while core.auth().signed_in {
        assert!(Instant::now() < deadline, "sign-out ignored");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(core.library().tracks.is_empty());
}
