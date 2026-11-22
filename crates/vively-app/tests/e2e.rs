//! End-to-end flows against a fake VK server: the real bootstrap, auth,
//! library, playback (simulated output) and like paths, driven the way an
//! embedder drives them (pump + tick on a plain, non-runtime thread).

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use vively_app::{
    AppCore, CoreConfig, CoreEvent, CorePaths, FileSessionPersistence, Host, TokioBridge,
    build_core,
    models::{AuthStep, LoadState, QrStage},
};
use vively_core::{
    downloads::{NetworkInterface, NetworkState, NetworkStatus},
    playback::PlaybackOrigin,
};
use wiremock::{
    Mock, MockServer, Request, ResponseTemplate,
    matchers::{path, path_regex},
};

const USER_ID: i64 = 42;

struct TestHost(Mutex<mpsc::Sender<()>>);

impl Host for TestHost {
    fn wake(&self) {
        let _ = self.0.lock().unwrap().send(());
    }
    fn network_state(&self) -> NetworkState {
        NetworkState {
            status: NetworkStatus::Online(NetworkInterface::Wifi),
            expensive: false,
            constrained: false,
        }
    }
}

fn ok(response: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({ "response": response }))
}

fn silent_mp3(frames: usize) -> Vec<u8> {
    let mut frame = vec![0xff, 0xfb, 0x90, 0x64];
    frame.resize(417, 0);
    frame.repeat(frames)
}

fn track_json(uri: &str, id: i64) -> Value {
    json!({
        "id": id, "owner_id": USER_ID, "title": format!("Песня {id}"), "artist": "Артист",
        "duration": 26, "is_explicit": false, "url": format!("{uri}/track{id}.mp3")
    })
}

struct FakeVk {
    server: MockServer,
    rt: tokio::runtime::Runtime,
}

impl FakeVk {
    /// `check` answers successive `auth.checkAuthCode` polls (last one repeats).
    fn new(check: Vec<Value>) -> Self {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let server = rt.block_on(MockServer::start());
        let uri = server.uri();
        let vk = Self { server, rt };
        vk.mount(
            Mock::given(path("/oauth/get_anonym_token"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({"token": "anon"}))),
        );
        vk.mount(Mock::given(path("/method/auth.getAuthCode")).respond_with(ok(json!({
            "auth_code": "code", "auth_hash": "hash",
            "auth_url": "https://vk.ru/qr?hash", "expires_in": 600
        }))));
        let polls = AtomicUsize::new(0);
        vk.mount(Mock::given(path("/method/auth.checkAuthCode")).respond_with(
            move |_: &Request| {
                let i = polls.fetch_add(1, Ordering::SeqCst).min(check.len() - 1);
                ok(check[i].clone())
            },
        ));
        vk.mount(Mock::given(path("/method/auth.revokeAuthCode")).respond_with(ok(json!({"status": 1}))));
        vk.mount(Mock::given(path("/method/account.getProfileInfo")).respond_with(ok(json!({
            "id": USER_ID, "first_name": "Тест", "last_name": "Тестов"
        }))));
        let tracks: Vec<Value> = (1..=3).map(|i| track_json(&uri, i)).collect();
        vk.mount(
            Mock::given(path("/method/audio.get"))
                .respond_with(ok(json!({"count": 3, "items": tracks}))),
        );
        let first = track_json(&uri, 1);
        vk.mount(Mock::given(path("/method/audio.getById")).respond_with(ok(json!([first]))));
        // audio.delete / add / restore via execute: VK returns 1 on success.
        vk.mount(Mock::given(path("/method/execute")).respond_with(ok(json!(1))));
        vk.mount(
            Mock::given(path_regex(r"^/track\d+\.mp3$")).respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "audio/mpeg")
                    .set_body_bytes(silent_mp3(1000)),
            ),
        );
        vk
    }

    fn mount(&self, mock: Mock) {
        self.rt.block_on(mock.mount(&self.server));
    }
}

struct Harness {
    core: AppCore,
    wakes: mpsc::Receiver<()>,
    paths: CorePaths,
    _root: tempfile::TempDir,
    _vk: FakeVk,
}

fn harness(vk: FakeVk) -> Harness {
    let root = tempfile::tempdir().unwrap();
    let paths = CorePaths::from_root(root.path());
    let (tx, wakes) = mpsc::channel();
    let core = build_core(
        Arc::new(TestHost(Mutex::new(tx))),
        TokioBridge::new().unwrap(),
        CoreConfig {
            persistence: Arc::new(FileSessionPersistence {
                path: paths.session_file(),
            }),
            paths: paths.clone(),
            simulated_audio: true,
            qr_device_name: "Vively (test)".into(),
            client_secret: "test-secret".into(),
            dev_session: None,
            api_host: Some(vk.server.uri().parse().unwrap()),
        },
    )
    .unwrap();
    Harness {
        core,
        wakes,
        paths,
        _root: root,
        _vk: vk,
    }
}

fn toasts(events: &[CoreEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            CoreEvent::Toast(m) => Some(m.clone()),
            _ => None,
        })
        .collect()
}

/// Pump + tick like an embedder until `done`, collecting events.
fn drive(
    h: &mut Harness,
    timeout: Duration,
    mut done: impl FnMut(&AppCore, &[CoreEvent]) -> bool,
) -> Vec<CoreEvent> {
    let deadline = Instant::now() + timeout;
    let mut events = Vec::new();
    loop {
        h.core.tick();
        h.core.pump();
        events.extend(h.core.take_events());
        if done(&h.core, &events) {
            return events;
        }
        assert!(
            Instant::now() < deadline,
            "timed out: auth={:?} error={:?} library={:?} loading={} toasts={:?}",
            h.core.auth.step,
            h.core.auth.error,
            h.core.library_state,
            h.core.track_loading,
            toasts(&events),
        );
        let _ = h.wakes.recv_timeout(Duration::from_millis(50));
    }
}

fn authorized() -> Value {
    json!({"status": 2, "access_token": "user-token", "user_id": USER_ID, "expires_in": 0})
}

#[test]
fn qr_login_library_playback_like_and_sign_out() {
    let mut h = harness(FakeVk::new(vec![
        json!({"status": 1, "user_info": {"first_name": "Тест", "last_name": "Тестов"}}),
        authorized(),
    ]));
    assert!(h.core.session.is_none());

    h.core.start_qr_auth();
    drive(&mut h, Duration::from_secs(5), |c, _| {
        matches!(c.auth.step, AuthStep::Qr(QrStage::Ready { .. }))
    });
    drive(&mut h, Duration::from_secs(8), |c, _| {
        matches!(&c.auth.step, AuthStep::Qr(QrStage::Scanned { user_name: Some(n), .. }) if n == "Тест Тестов")
    });
    let events = drive(&mut h, Duration::from_secs(8), |_, ev| {
        ev.iter().any(|e| matches!(e, CoreEvent::LoggedIn(_)))
    });
    let session = events
        .into_iter()
        .find_map(|e| match e {
            CoreEvent::LoggedIn(s) => Some(s),
            _ => None,
        })
        .unwrap();
    assert_eq!(session.user_id, USER_ID);
    assert!(!h.core.adopt_login(session), "first login is not an account switch");
    assert!(h.paths.session_file().exists(), "session persisted");

    drive(&mut h, Duration::from_secs(5), |c, _| {
        c.library_state == LoadState::Loaded && c.library_tracks.len() == 3
    });
    assert_eq!(h.core.library_total, 3);
    assert!(h.core.is_liked(&h.core.library_tracks[0]));

    let tracks = h.core.library_tracks.clone();
    h.core
        .play_tracks(tracks, 0, PlaybackOrigin::Library, "Моя музыка", None);
    assert!(h.core.track_loading);
    let events = drive(&mut h, Duration::from_secs(20), |c, _| {
        !c.track_loading && c.audio_engine.has_active()
    });
    assert!(
        !toasts(&events).iter().any(|t| t.contains("недоступен") || t.contains("пропущен")),
        "track failed to start: {:?}",
        toasts(&events)
    );
    assert!(h.core.audio_engine.is_playing());
    assert_eq!(h.core.queue.current().unwrap().id, 1);

    h.core.toggle_play_pause();
    assert!(!h.core.audio_engine.is_playing());
    h.core.next_track();
    drive(&mut h, Duration::from_secs(20), |c, _| {
        !c.track_loading && c.queue.current().is_some_and(|t| t.id == 2)
    });

    // Output device lost mid-track: same track resumes on a fresh stream.
    std::thread::sleep(Duration::from_millis(300));
    h.core.audio_engine.inject_output_failure();
    drive(&mut h, Duration::from_secs(20), |c, _| {
        !c.track_loading
            && c.audio_engine.has_active()
            && c.audio_engine.is_playing()
            && c.queue.current().is_some_and(|t| t.id == 2)
    });

    let first = h.core.library_tracks[0].clone();
    h.core.toggle_like_track(first);
    let events = drive(&mut h, Duration::from_secs(5), |c, _| c.library_tracks.len() == 2);
    assert!(toasts(&events).iter().any(|t| t == "Убрано из Моей музыки"));
    assert!(h.core.restorable_own_track.is_some(), "own track can be restored");

    h.core.end_session();
    h.core.reset_account();
    assert!(h.core.session.is_none());
    assert!(h.core.library_tracks.is_empty());
    assert!(h.core.queue.tracks().is_empty());
    assert!(!h.core.audio_engine.has_active());
    assert!(!h.paths.session_file().exists(), "session file removed on sign-out");
}

#[test]
fn qr_declined_ends_polling() {
    let mut h = harness(FakeVk::new(vec![json!({"status": 3})]));
    h.core.start_qr_auth();
    drive(&mut h, Duration::from_secs(8), |c, _| {
        matches!(c.auth.step, AuthStep::Qr(QrStage::Declined))
    });
    assert!(!h.core.auth.loading);
    assert!(h.core.session.is_none());
}

#[test]
fn captcha_prompt_reaches_embedder_and_answer_retries_request() {
    let vk = FakeVk::new(vec![json!({"status": 0})]);
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&calls);
    // Equal-priority mocks resolve first-mounted-wins, so outrank the default.
    vk.mount(Mock::given(path("/method/auth.getAuthCode")).respond_with(move |_: &Request| {
        if seen.fetch_add(1, Ordering::SeqCst) == 0 {
            ResponseTemplate::new(200).set_body_json(json!({"error": {
                "error_code": 14, "error_msg": "Captcha needed", "captcha_sid": "sid",
                "redirect_uri": "https://captcha.invalid/", "captcha_ts": 1.0, "captcha_attempt": 1
            }}))
        } else {
            ok(json!({"auth_code": "c", "auth_hash": "h", "auth_url": "https://vk.ru/qr", "expires_in": 600}))
        }
    }).with_priority(1));
    let mut h = harness(vk);
    h.core.start_qr_auth();
    let events = drive(&mut h, Duration::from_secs(5), |_, ev| {
        ev.iter().any(|e| matches!(e, CoreEvent::CaptchaRequested(_)))
    });
    for ev in events {
        if let CoreEvent::CaptchaRequested(prompt) = ev {
            assert_eq!(
                prompt.challenge.redirect_url.as_ref().map(|u| u.as_str()),
                Some("https://captcha.invalid/")
            );
            let _ = prompt.reply.send(Some("solved-token".into()));
        }
    }
    drive(&mut h, Duration::from_secs(5), |c, _| {
        matches!(c.auth.step, AuthStep::Qr(QrStage::Ready { .. }))
    });
    assert_eq!(calls.load(Ordering::SeqCst), 2, "request retried once after captcha");
}
