//! UniFFI surface for the Android app.
//!
//! `AppCore` lives on one dedicated thread (`vively-core`). Kotlin talks to
//! it through [`VivelyCore`]: commands are queued closures, snapshots are
//! synchronous round-trips, and changes come back through
//! [`CoreListener::on_update`].

uniffi::setup_scaffolding!();

#[cfg(target_os = "android")]
mod android;
mod dto;
mod extra;
mod screens;

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        mpsc::{self, RecvTimeoutError},
    },
    thread,
    time::{Duration, Instant},
};

pub use dto::*;
pub use extra::*;
pub use screens::*;
use futures_channel::oneshot;
use vively_app::{
    AppCore, CoreConfig, CoreEvent, CorePaths, Dirty, FileSessionPersistence, Host, TokioBridge,
    build_core,
    captcha::{self, CaptchaNavOutcome},
    fmt,
};
use vively_audio::MediaCommand;
use vively_core::{
    config::ANDROID_CLIENT_SECRET, downloads::NetworkState, models::Audio,
    playback::PlaybackOrigin,
};

const TICK: Duration = Duration::from_millis(200);

/// Rust panics print to stderr, which Android discards; route them to logcat.
#[cfg(target_os = "android")]
fn install_panic_logger() {
    use std::{ffi::CString, os::raw::c_char, sync::Once};
    unsafe extern "C" {
        fn __android_log_write(prio: i32, tag: *const c_char, text: *const c_char) -> i32;
    }
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let msg = format!("{info}\n{}", std::backtrace::Backtrace::force_capture());
            let text = CString::new(msg.replace('\0', " ")).unwrap_or_default();
            unsafe { __android_log_write(6, c"vively".as_ptr(), text.as_ptr()) };
            prev(info);
        }));
    });
}

#[cfg(not(target_os = "android"))]
fn install_panic_logger() {}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".into())
}
/// Bound on pump/event rounds per flush (events can trigger more work).
const MAX_FLUSH_ROUNDS: usize = 8;

/// Implemented in Kotlin.
#[uniffi::export(callback_interface)]
pub trait CoreListener: Send + Sync {
    /// Called on the core thread. Do NOT call back into `VivelyCore`
    /// synchronously from here (snapshots would deadlock); post to your UI
    /// thread / coroutine first.
    fn on_update(&self, dirty: FfiDirty, events: Vec<FfiEvent>);
    /// May be called from any thread.
    fn network_state(&self) -> FfiNetwork;
}

type Job = Box<dyn FnOnce(&mut AppCore) + Send>;

enum Msg {
    Run(Job),
    Pump,
    Captcha { id: u64, answer: Option<String> },
    Shutdown,
}

struct FfiHost {
    tx: mpsc::Sender<Msg>,
    listener: Arc<dyn CoreListener>,
}

impl Host for FfiHost {
    fn wake(&self) {
        let _ = self.tx.send(Msg::Pump);
    }

    fn network_state(&self) -> NetworkState {
        self.listener.network_state().into()
    }
}

type CaptchaReplies = HashMap<u64, oneshot::Sender<Option<String>>>;

fn convert_event(core: &mut AppCore, captchas: &mut CaptchaReplies, ev: CoreEvent) -> FfiEvent {
    match ev {
        CoreEvent::Toast(message) => FfiEvent::Toast { message },
        CoreEvent::CaptchaRequested(prompt) => {
            let ev = FfiEvent::Captcha {
                id: prompt.id,
                redirect_url: prompt.challenge.redirect_url.as_ref().map(ToString::to_string),
                image_url: prompt.challenge.image_url.as_ref().map(ToString::to_string),
            };
            captchas.insert(prompt.id, prompt.reply);
            ev
        }
        CoreEvent::LoggedIn(session) => {
            core.adopt_login(session);
            FfiEvent::SignedIn
        }
        CoreEvent::FocusAuthField { field, clear } => FfiEvent::FocusAuthField {
            field: field.into(),
            clear,
        },
        CoreEvent::PlaylistDeleted { key } => FfiEvent::PlaylistDeleted { key },
        CoreEvent::SearchSettled { query, generation } => {
            FfiEvent::SearchSettled { query, generation }
        }
        CoreEvent::ActiveTrackChanged => FfiEvent::ActiveTrackChanged,
        CoreEvent::PlaybackSeeked => FfiEvent::PlaybackSeeked,
        CoreEvent::PlaylistSaved { playlist } => FfiEvent::PlaylistSaved {
            full_id: playlist.full_id(),
            key: playlist.stable_id(),
        },
        CoreEvent::EditorClosed => FfiEvent::EditorClosed,
        CoreEvent::UploadFinished { all_done } => FfiEvent::UploadFinished { all_done },
        CoreEvent::ExportFinished { full_id, result } => {
            let (path, error) = match result {
                Ok(p) => (Some(p.display().to_string()), None),
                Err(e) => (None, Some(e)),
            };
            FfiEvent::ExportFinished {
                full_id,
                path,
                error,
            }
        }
    }
}

fn flush(core: &mut AppCore, captchas: &mut CaptchaReplies, listener: &dyn CoreListener) {
    let mut dirty = Dirty::NONE;
    let mut out = Vec::new();
    for _ in 0..MAX_FLUSH_ROUNDS {
        dirty |= core.pump();
        let events = core.take_events();
        if events.is_empty() {
            break;
        }
        for ev in events {
            out.push(convert_event(core, captchas, ev));
        }
    }
    dirty |= core.take_dirty();
    if !dirty.is_empty() || !out.is_empty() {
        listener.on_update(dirty.into(), out);
    }
}

fn run_actor(mut core: AppCore, rx: mpsc::Receiver<Msg>, listener: Arc<dyn CoreListener>) {
    let mut captchas = CaptchaReplies::new();
    let mut next_tick = Instant::now() + TICK;
    loop {
        let wait = next_tick.saturating_duration_since(Instant::now());
        match rx.recv_timeout(wait) {
            Ok(Msg::Run(job)) => job(&mut core),
            Ok(Msg::Pump) | Err(RecvTimeoutError::Timeout) => {}
            Ok(Msg::Captcha { id, answer }) => {
                if let Some(reply) = captchas.remove(&id) {
                    let _ = reply.send(answer);
                }
            }
            Ok(Msg::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
        }
        // Tick even under a steady stream of messages.
        if Instant::now() >= next_tick {
            core.tick();
            next_tick = Instant::now() + TICK;
        }
        flush(&mut core, &mut captchas, &*listener);
    }
    for (_, reply) in captchas {
        let _ = reply.send(None);
    }
}

fn find_track(core: &AppCore, full_id: &str) -> Option<Audio> {
    screens::find_track(core, full_id)
}

#[derive(uniffi::Object)]
pub struct VivelyCore {
    tx: mpsc::Sender<Msg>,
    actor: thread::ThreadId,
}

impl Drop for VivelyCore {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Shutdown);
    }
}

impl VivelyCore {
    fn run(&self, f: impl FnOnce(&mut AppCore) + Send + 'static) {
        let _ = self.tx.send(Msg::Run(Box::new(f)));
    }

    /// Synchronous read on the core thread. Returns `None` if the core is
    /// gone or if called from the core thread itself (would deadlock).
    fn query<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut AppCore) -> T + Send + 'static,
    ) -> Option<T> {
        if thread::current().id() == self.actor {
            return None;
        }
        let (tx, rx) = mpsc::sync_channel(1);
        self.tx
            .send(Msg::Run(Box::new(move |core| {
                let _ = tx.send(f(core));
            })))
            .ok()?;
        rx.recv().ok()
    }

    fn with_track(&self, full_id: String, f: impl FnOnce(&mut AppCore, Audio) + Send + 'static) {
        self.run(move |core| {
            if let Some(track) = find_track(core, &full_id) {
                f(core, track);
            }
        });
    }
}

#[uniffi::export]
impl VivelyCore {
    #[uniffi::constructor]
    pub fn new(config: FfiConfig, listener: Box<dyn CoreListener>) -> Result<Arc<Self>, FfiError> {
        install_panic_logger();
        let listener: Arc<dyn CoreListener> = Arc::from(listener);
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<(), String>>(1);
        let host = Arc::new(FfiHost {
            tx: tx.clone(),
            listener: Arc::clone(&listener),
        });
        let paths = CorePaths::new(
            PathBuf::from(config.config_dir),
            PathBuf::from(config.data_dir),
        );
        let core_config = CoreConfig {
            persistence: Arc::new(FileSessionPersistence {
                path: paths.session_file(),
            }),
            paths,
            simulated_audio: config.simulated_audio,
            qr_device_name: config.device_name,
            client_secret: ANDROID_CLIENT_SECRET.into(),
            dev_session: None,
            api_host: config.api_host.as_deref().and_then(|h| h.parse().ok()),
        };

        let handle = thread::Builder::new()
            .name("vively-core".into())
            .spawn(move || {
                let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    TokioBridge::new().map_err(|e| e.to_string()).and_then(|tokio| {
                        build_core(host, tokio, core_config).map_err(|e| e.to_string())
                    })
                }))
                .unwrap_or_else(|p| Err(format!("panic: {}", panic_message(&*p))));
                match built {
                    Ok(mut core) => {
                        let _ = ready_tx.send(Ok(()));
                        if let Some(session) = core.session.clone() {
                            core.activate_session(session);
                        }
                        run_actor(core, rx, listener);
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                    }
                }
            })
            .map_err(|e| FfiError::Init {
                detail: e.to_string(),
            })?;
        let actor = handle.thread().id();
        ready_rx
            .recv()
            .map_err(|_| FfiError::Init {
                detail: "core thread exited".into(),
            })?
            .map_err(|detail| FfiError::Init { detail })?;
        Ok(Arc::new(Self { tx, actor }))
    }

    // ---- snapshots

    pub fn auth(&self) -> FfiAuth {
        self.query(|c| FfiAuth::from_core(c)).unwrap_or_default()
    }

    pub fn library(&self) -> FfiLibrary {
        self.query(|c| FfiLibrary::from_core(c)).unwrap_or_default()
    }

    pub fn playback(&self) -> FfiPlayback {
        self.query(|c| FfiPlayback::from_core(c)).unwrap_or_default()
    }

    /// Cheap position read for the seek bar.
    pub fn position_secs(&self) -> f64 {
        self.query(|c| c.playback_position()).unwrap_or_default()
    }

    // ---- auth

    pub fn submit_auth(&self, input: String) {
        self.run(move |c| c.submit_auth(&input));
    }

    pub fn resend_auth_otp(&self) {
        self.run(|c| c.resend_auth_otp());
    }

    pub fn start_qr_auth(&self) {
        self.run(|c| c.start_qr_auth());
    }

    pub fn cancel_qr_auth(&self) {
        self.run(|c| c.cancel_qr_auth());
    }

    pub fn auth_go_back(&self) {
        self.run(|c| c.auth_go_back());
    }

    pub fn reset_auth(&self) {
        self.run(|c| c.reset_auth());
    }

    /// Answer an `FfiEvent::Captcha`; `None` cancels.
    pub fn answer_captcha(&self, id: u64, answer: Option<String>) {
        let _ = self.tx.send(Msg::Captcha { id, answer });
    }

    pub fn sign_out(&self) {
        self.run(|c| {
            c.end_session();
            c.reset_account();
        });
    }

    // ---- library

    pub fn load_library(&self, force: bool) {
        self.run(move |c| c.load_library(force));
    }

    pub fn load_more_library(&self) {
        self.run(|c| c.load_more_library());
    }

    pub fn toggle_like(&self, full_id: String) {
        self.with_track(full_id, |c, t| c.toggle_like_track(t));
    }

    pub fn settle_search_history(&self, query: String, generation: u64) {
        self.run(move |c| c.settle_search_history(&query, generation));
    }

    // ---- playback

    pub fn play_library(&self, start_index: u32) {
        self.run(move |c| {
            let tracks = c.library_tracks.clone();
            c.play_tracks(
                tracks,
                start_index as usize,
                PlaybackOrigin::Library,
                "Моя музыка",
                None,
            );
        });
    }

    pub fn select_queue_track(&self, full_id: String) {
        self.with_track(full_id, |c, t| c.select_queue_track(t));
    }

    pub fn play_next(&self, full_id: String) {
        self.with_track(full_id, |c, t| c.play_next_track(t));
    }

    pub fn toggle_play_pause(&self) {
        self.run(|c| c.toggle_play_pause());
    }

    /// MediaSession play (idempotent).
    pub fn play(&self) {
        self.run(|c| c.handle_media_command(MediaCommand::Play));
    }

    /// MediaSession pause / audio becoming noisy / focus loss (idempotent).
    pub fn pause(&self) {
        self.run(|c| c.handle_media_command(MediaCommand::Pause));
    }

    pub fn stop(&self) {
        self.run(|c| c.handle_media_command(MediaCommand::Stop));
    }

    pub fn next_track(&self) {
        self.run(|c| c.next_track());
    }

    pub fn prev_track(&self) {
        self.run(|c| c.prev_track());
    }

    pub fn seek_to(&self, secs: f64) {
        self.run(move |c| c.seek_to(secs));
    }

    pub fn set_volume(&self, volume: f32) {
        self.run(move |c| c.set_volume(volume));
    }

    pub fn toggle_shuffle(&self) {
        self.run(|c| c.toggle_shuffle());
    }

    pub fn cycle_repeat(&self) {
        self.run(|c| c.cycle_repeat());
    }

    /// MediaSession repeat (idempotent).
    pub fn set_repeat(&self, repeat: FfiRepeat) {
        self.run(move |c| c.handle_media_command(MediaCommand::SetRepeat(repeat.into())));
    }

    /// MediaSession shuffle (idempotent).
    pub fn set_shuffle(&self, shuffled: bool) {
        self.run(move |c| c.handle_media_command(MediaCommand::SetShuffle(shuffled)));
    }
}

// ---- free helpers for the Android captcha WebView and formatting

#[uniffi::export]
pub fn captcha_interceptor_script() -> String {
    captcha::captcha_interceptor_script().to_owned()
}

#[uniffi::export]
pub fn extract_captcha_success_token(text: String) -> Option<String> {
    captcha::extract_captcha_success_token(&text)
}

#[uniffi::export]
pub fn parse_captcha_nav(url: String) -> Option<FfiCaptchaNav> {
    captcha::parse_captcha_nav_outcome(&url).map(|o| match o {
        CaptchaNavOutcome::Success(token) => FfiCaptchaNav::Success { token },
        CaptchaNavOutcome::Cancel => FfiCaptchaNav::Cancel,
    })
}

/// Vively palette for a theme mode; `tint_hex` is the album accent (none = neutral).
#[uniffi::export]
pub fn resolve_palette(is_dark: bool, tint_hex: Option<String>) -> FfiPalette {
    use vively_app::palette::{DEFAULT_TINT, PaletteTokens, parse_hex_color};
    let (tint, neutral) = match tint_hex.as_deref().and_then(parse_hex_color) {
        Some(c) => (c, false),
        None => (DEFAULT_TINT, true),
    };
    PaletteTokens::resolve(is_dark, tint, neutral).into()
}

#[uniffi::export]
pub fn format_duration(secs: f64) -> String {
    fmt::fmt_duration_f64(secs)
}

#[uniffi::export]
pub fn format_tracks_count(count: i64) -> String {
    fmt::fmt_tracks_count(count)
}

#[uniffi::export]
pub fn format_gb(bytes: i64) -> String {
    fmt::fmt_gb(bytes.max(0) as u64)
}

#[uniffi::export]
pub fn format_friends_count(count: i64) -> String {
    fmt::fmt_friends_count(count)
}

#[uniffi::export]
pub fn format_playlists_count(count: i64) -> String {
    fmt::fmt_playlists_count(count.max(0) as usize)
}
