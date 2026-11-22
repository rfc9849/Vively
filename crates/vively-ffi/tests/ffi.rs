use std::{
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use vively_ffi::{
    CoreListener, FfiAppearance, FfiLoad, FfiAuthStep, FfiCaptchaNav, FfiConfig, FfiDirty, FfiEvent, FfiNetwork,
    FfiNetworkStatus, VivelyCore, captcha_interceptor_script, parse_captcha_nav, resolve_palette,
};

struct TestListener {
    updates: Mutex<mpsc::Sender<(FfiDirty, Vec<FfiEvent>)>>,
}

impl CoreListener for TestListener {
    fn on_update(&self, dirty: FfiDirty, events: Vec<FfiEvent>) {
        let _ = self.updates.lock().unwrap().send((dirty, events));
    }

    fn network_state(&self) -> FfiNetwork {
        FfiNetwork {
            status: FfiNetworkStatus::Wifi,
            expensive: false,
            constrained: false,
        }
    }
}

fn core() -> (Arc<VivelyCore>, mpsc::Receiver<(FfiDirty, Vec<FfiEvent>)>) {
    let root = std::env::temp_dir().join(format!(
        "vively-ffi-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let (tx, rx) = mpsc::channel();
    let core = VivelyCore::new(
        FfiConfig {
            config_dir: root.join("config").display().to_string(),
            data_dir: root.join("data").display().to_string(),
            device_name: "Vively (test)".into(),
            simulated_audio: true,
            api_host: None,
        },
        Box::new(TestListener {
            updates: Mutex::new(tx),
        }),
    )
    .unwrap();
    (core, rx)
}

#[test]
fn starts_signed_out_and_rejects_bad_phone_via_listener() {
    let (core, updates) = core();
    let auth = core.auth();
    assert!(!auth.signed_in);
    assert_eq!(auth.step, FfiAuthStep::Phone);

    core.submit_auth("123".into());
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        let (dirty, _) = updates.recv_timeout(left).expect("auth update");
        if dirty.auth {
            break;
        }
    }
    assert!(core.auth().error.is_some());
    assert!(core.library().tracks.is_empty());
    assert!(core.playback().current.is_none());
}

#[test]
fn captcha_helpers_are_exposed() {
    assert!(captcha_interceptor_script().contains("postMessage"));
    assert_eq!(
        parse_captcha_nav("https://id.vk.ru/x#cancel=1".into()),
        Some(FfiCaptchaNav::Cancel)
    );
    assert_eq!(parse_captcha_nav("https://vk.ru/".into()), None);
}

#[test]
fn palette_is_exposed_and_tint_changes_highlight() {
    let neutral = resolve_palette(true, None);
    assert_eq!(neutral.hi, neutral.ink);
    let red = resolve_palette(true, Some("#e04040".into()));
    assert_ne!(red.hi, red.ink);
    assert_eq!(resolve_palette(false, None).cta_fg, 0xFFFFFFFF);
    assert_eq!(resolve_palette(true, Some("garbage".into())), neutral);
}

#[test]
fn screen_snapshots_and_settings_commands_round_trip() {
    let (core, _updates) = core();
    assert!(core.settings().wifi_only);
    core.toggle_wifi_only();
    assert!(!core.settings().wifi_only, "commands and queries are ordered");
    core.set_crossfade(40.0);
    assert_eq!(core.settings().crossfade_secs, 12.0);
    core.set_appearance(FfiAppearance::Dark);
    assert_eq!(core.settings().appearance, FfiAppearance::Dark);
    assert!(core.search().tracks.is_empty());
    assert_eq!(core.downloads().track_count, 0);
    assert!(core.playlist_detail("missing".into()).is_none());
    assert!(core.friend_music(1).is_none());
    assert_eq!(core.lyrics().state, FfiLoad::Idle);
    assert!(core.home().blocks.is_empty());
    assert_eq!(core.artist("x".into()).id, "x");
}
