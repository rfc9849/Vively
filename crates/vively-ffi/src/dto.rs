//! Plain records/enums that cross the FFI boundary. Built from `AppCore`
//! state on the core thread; Kotlin only ever sees copies.

use std::collections::HashSet;

use vively_app::{
    AppCore, AuthField, Dirty,
    models::{AuthStep, LoadState, QrStage},
};
use vively_core::{
    downloads::{NetworkInterface, NetworkState, NetworkStatus},
    models::Audio,
    playback::RepeatMode,
};

/// Max upcoming tracks included in a playback snapshot.
const UPCOMING_LIMIT: usize = 50;

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiConfig {
    pub config_dir: String,
    pub data_dir: String,
    /// Shown on the phone that scans the login QR code.
    pub device_name: String,
    /// No output device (tests).
    pub simulated_audio: bool,
    /// Override the VK API host (mock servers in tests).
    #[uniffi(default = None)]
    pub api_host: Option<String>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiTrack {
    pub full_id: String,
    pub stable_id: String,
    pub owner_id: i64,
    pub id: i64,
    pub title: String,
    pub subtitle: Option<String>,
    pub artist: String,
    pub duration_secs: i64,
    pub explicit: bool,
    pub playable: bool,
    pub liked: bool,
    pub disliked: bool,
    pub downloaded: bool,
    pub download_pending: bool,
    /// Full-size cover.
    pub artwork_url: Option<String>,
    /// Small cover for list rows.
    pub thumb_url: Option<String>,
    /// Set when the track links to an album page (`open_track_album`).
    pub album_title: Option<String>,
    pub artist_id: Option<String>,
    pub artist_name: Option<String>,
}

/// Per-snapshot lookup sets, so list conversion stays linear.
pub(crate) struct TrackCtx {
    liked: HashSet<(i64, i64)>,
    downloaded: HashSet<(i64, i64)>,
    pending: HashSet<String>,
}

impl TrackCtx {
    pub(crate) fn new(core: &AppCore) -> Self {
        let mut liked: HashSet<(i64, i64)> = core
            .library_tracks
            .iter()
            .map(|t| (t.owner_id, t.id))
            .collect();
        liked.extend(core.library_foreign_copies.keys().copied());
        Self {
            liked,
            downloaded: core
                .ready_downloads
                .iter()
                .map(|t| (t.owner_id, t.id))
                .collect(),
            pending: core
                .pending_downloads
                .iter()
                .map(|t| t.full_id.clone())
                .collect(),
        }
    }
}

impl FfiTrack {
    pub(crate) fn from_audio(core: &AppCore, ctx: &TrackCtx, a: &Audio) -> Self {
        let key = (a.owner_id, a.id);
        let full_id = a.full_id();
        let album_title = a
            .album
            .as_ref()
            .filter(|al| al.id.unwrap_or(0) != 0)
            .and_then(|al| al.title.clone())
            .filter(|t| !t.is_empty());
        let main = a.main_artists.as_ref().and_then(|list| list.first());
        Self {
            stable_id: a.stable_id(),
            owner_id: a.owner_id,
            id: a.id,
            title: a.title.clone(),
            subtitle: a.subtitle.clone().filter(|s| !s.is_empty()),
            artist: a.artist.clone(),
            duration_secs: a.duration,
            explicit: a.is_explicit,
            playable: a.is_playable(),
            liked: ctx.liked.contains(&key),
            disliked: core.disliked_tracks.contains(&key),
            downloaded: ctx.downloaded.contains(&key),
            download_pending: ctx.pending.contains(&full_id),
            artwork_url: a.artwork_url().map(ToString::to_string),
            thumb_url: a
                .thumbnail_url()
                .or_else(|| a.artwork_url())
                .map(ToString::to_string),
            album_title,
            artist_id: main.map(|m| m.id.clone()).filter(|id| !id.is_empty()),
            artist_name: main
                .map(|m| m.name.trim().to_owned())
                .filter(|n| !n.is_empty())
                .or_else(|| Some(a.artist.clone())),
            full_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum FfiLoad {
    Idle,
    Loading,
    Loaded,
    Failed { message: String },
}

impl Default for FfiLoad {
    fn default() -> Self {
        Self::Idle
    }
}

impl From<&LoadState> for FfiLoad {
    fn from(s: &LoadState) -> Self {
        match s {
            LoadState::Idle => Self::Idle,
            LoadState::Loading => Self::Loading,
            LoadState::Loaded => Self::Loaded,
            LoadState::Failed(m) => Self::Failed { message: m.clone() },
        }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum FfiAuthStep {
    Phone,
    QrLoading,
    QrReady {
        url: String,
        size: u32,
        modules: Vec<bool>,
    },
    QrScanned {
        url: String,
        size: u32,
        modules: Vec<bool>,
        user_name: Option<String>,
    },
    QrConfirmCode {
        url: String,
    },
    QrExpired,
    QrDeclined,
    Otp {
        method_label: String,
        subtitle: String,
        placeholder: String,
        resend_label: String,
        code_len: u32,
    },
    Password {
        had_otp: bool,
    },
}

impl Default for FfiAuthStep {
    fn default() -> Self {
        Self::Phone
    }
}

impl From<&AuthStep> for FfiAuthStep {
    fn from(step: &AuthStep) -> Self {
        match step {
            AuthStep::Phone => Self::Phone,
            AuthStep::Qr(QrStage::Loading) => Self::QrLoading,
            AuthStep::Qr(QrStage::Ready { url, size, modules }) => Self::QrReady {
                url: url.clone(),
                size: *size as u32,
                modules: modules.clone(),
            },
            AuthStep::Qr(QrStage::Scanned {
                url,
                size,
                modules,
                user_name,
            }) => Self::QrScanned {
                url: url.clone(),
                size: *size as u32,
                modules: modules.clone(),
                user_name: user_name.clone(),
            },
            AuthStep::Qr(QrStage::ConfirmCode { url }) => Self::QrConfirmCode { url: url.clone() },
            AuthStep::Qr(QrStage::Expired) => Self::QrExpired,
            AuthStep::Qr(QrStage::Declined) => Self::QrDeclined,
            AuthStep::Otp {
                method_label,
                subtitle,
                placeholder,
                resend_label,
                code_len,
            } => Self::Otp {
                method_label: method_label.clone(),
                subtitle: subtitle.clone(),
                placeholder: placeholder.clone(),
                resend_label: resend_label.clone(),
                code_len: *code_len as u32,
            },
            AuthStep::Password { had_otp } => Self::Password { had_otp: *had_otp },
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct FfiAuth {
    pub step: FfiAuthStep,
    pub loading: bool,
    pub error: Option<String>,
    pub signed_in: bool,
    pub user_name: Option<String>,
    pub user_id: i64,
    pub photo_url: Option<String>,
}

impl FfiAuth {
    pub(crate) fn from_core(core: &AppCore) -> Self {
        let user = core.session.as_ref().and_then(|s| s.user.as_ref());
        let user_name = user
            .map(|u| format!("{} {}", u.first_name, u.last_name).trim().to_owned())
            .filter(|n| !n.is_empty());
        Self {
            step: (&core.auth.step).into(),
            loading: core.auth.loading,
            error: core.auth.error.clone(),
            signed_in: core.session.is_some(),
            user_name,
            user_id: core.session.as_ref().map_or(0, |s| s.user_id),
            photo_url: user.and_then(|u| u.photo_200.as_ref()).map(ToString::to_string),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct FfiLibrary {
    pub state: FfiLoad,
    pub tracks: Vec<FfiTrack>,
    pub total: i64,
    pub can_load_more: bool,
    pub loading_more: bool,
    pub more_failed: bool,
    /// Own upload just removed from the library; offer "restore".
    pub restorable: Option<FfiTrack>,
}

impl FfiLibrary {
    pub(crate) fn from_core(core: &AppCore) -> Self {
        let ctx = TrackCtx::new(core);
        Self {
            state: (&core.library_state).into(),
            tracks: core
                .library_tracks
                .iter()
                .map(|t| FfiTrack::from_audio(core, &ctx, t))
                .collect(),
            restorable: core
                .restorable_own_track
                .as_ref()
                .map(|t| FfiTrack::from_audio(core, &ctx, t)),
            total: core.library_total,
            can_load_more: core.library_can_more,
            loading_more: core.library_loading_more,
            more_failed: core.library_more_failed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiRepeat {
    Off,
    All,
    One,
}

impl Default for FfiRepeat {
    fn default() -> Self {
        Self::Off
    }
}

impl From<FfiRepeat> for RepeatMode {
    fn from(r: FfiRepeat) -> Self {
        match r {
            FfiRepeat::Off => Self::Off,
            FfiRepeat::All => Self::All,
            FfiRepeat::One => Self::One,
        }
    }
}

impl From<RepeatMode> for FfiRepeat {
    fn from(r: RepeatMode) -> Self {
        match r {
            RepeatMode::Off => Self::Off,
            RepeatMode::All => Self::All,
            RepeatMode::One => Self::One,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct FfiPlayback {
    pub current: Option<FfiTrack>,
    pub playing: bool,
    pub loading: bool,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: f32,
    pub muted: bool,
    pub shuffled: bool,
    pub repeat: FfiRepeat,
    pub has_next: bool,
    pub playing_from: String,
    pub upcoming: Vec<FfiTrack>,
    /// Album accent colour of the current track ("#rrggbb"), drives the theme tint.
    pub tint_hex: Option<String>,
}

impl FfiPlayback {
    pub(crate) fn from_core(core: &AppCore) -> Self {
        let current = core.queue.current();
        let ctx = TrackCtx::new(core);
        Self {
            current: current.map(|t| FfiTrack::from_audio(core, &ctx, t)),
            playing: core.audio_engine.is_playing(),
            loading: core.track_loading,
            position_secs: core.playback_position(),
            duration_secs: current.map_or(0.0, |t| t.duration as f64),
            volume: core.settings.volume,
            muted: core.settings.muted,
            shuffled: core.queue.is_shuffled(),
            repeat: core.queue.repeat_mode.into(),
            has_next: core.queue.next_index().is_some(),
            playing_from: core.playing_from.clone(),
            upcoming: core
                .queue
                .upcoming()
                .iter()
                .take(UPCOMING_LIMIT)
                .map(|t| FfiTrack::from_audio(core, &ctx, t))
                .collect(),
            tint_hex: current
                .and_then(|t| t.album.as_ref())
                .and_then(|a| a.main_color.clone())
                .filter(|h| !h.trim().is_empty()),
        }
    }
}

/// Vively design tokens as packed 0xAARRGGBB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct FfiPalette {
    pub is_dark: bool,
    pub base: u32,
    pub ground: u32,
    pub rail: u32,
    pub raised: u32,
    pub stage_bg: u32,
    pub dock: u32,
    pub fill: u32,
    pub fill_2: u32,
    pub hover: u32,
    pub active: u32,
    pub line: u32,
    pub art_ring: u32,
    pub ink: u32,
    pub ink_2: u32,
    pub ink_3: u32,
    pub hi: u32,
    pub cta_bg: u32,
    pub cta_fg: u32,
    pub cta_hover: u32,
    pub live_dot: u32,
    pub danger: u32,
    pub scrim: u32,
}

impl From<vively_app::palette::PaletteTokens> for FfiPalette {
    fn from(t: vively_app::palette::PaletteTokens) -> Self {
        Self {
            is_dark: t.is_dark,
            base: t.base.argb(),
            ground: t.ground.argb(),
            rail: t.rail.argb(),
            raised: t.raised.argb(),
            stage_bg: t.stage_bg.argb(),
            dock: t.dock.argb(),
            fill: t.fill.argb(),
            fill_2: t.fill_2.argb(),
            hover: t.hover.argb(),
            active: t.active.argb(),
            line: t.line.argb(),
            art_ring: t.art_ring.argb(),
            ink: t.ink.argb(),
            ink_2: t.ink_2.argb(),
            ink_3: t.ink_3.argb(),
            hi: t.hi.argb(),
            cta_bg: t.cta_bg.argb(),
            cta_fg: t.cta_fg.argb(),
            cta_hover: t.cta_hover.argb(),
            live_dot: t.live_dot.argb(),
            danger: t.danger.argb(),
            scrim: t.scrim.argb(),
        }
    }
}

/// Which state slices changed; re-pull only the matching snapshots.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, uniffi::Record)]
pub struct FfiDirty {
    pub session: bool,
    pub auth: bool,
    pub settings: bool,
    pub home: bool,
    pub library: bool,
    pub playlists: bool,
    pub pages: bool,
    pub friends: bool,
    pub search: bool,
    pub downloads: bool,
    pub playback: bool,
    pub position: bool,
    pub lyrics: bool,
    pub tint: bool,
    pub editor: bool,
    pub upload: bool,
}

impl From<Dirty> for FfiDirty {
    fn from(d: Dirty) -> Self {
        Self {
            session: d.contains(Dirty::SESSION),
            auth: d.contains(Dirty::AUTH),
            settings: d.contains(Dirty::SETTINGS),
            home: d.contains(Dirty::HOME),
            library: d.contains(Dirty::LIBRARY),
            playlists: d.contains(Dirty::PLAYLISTS),
            pages: d.contains(Dirty::PAGES),
            friends: d.contains(Dirty::FRIENDS),
            search: d.contains(Dirty::SEARCH),
            downloads: d.contains(Dirty::DOWNLOADS),
            playback: d.contains(Dirty::PLAYBACK),
            position: d.contains(Dirty::POSITION),
            lyrics: d.contains(Dirty::LYRICS),
            tint: d.contains(Dirty::TINT),
            editor: d.contains(Dirty::EDITOR),
            upload: d.contains(Dirty::UPLOAD),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiAuthField {
    Phone,
    Code,
    Password,
}

impl From<AuthField> for FfiAuthField {
    fn from(f: AuthField) -> Self {
        match f {
            AuthField::Phone => Self::Phone,
            AuthField::Code => Self::Code,
            AuthField::Password => Self::Password,
        }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum FfiEvent {
    Toast {
        message: String,
    },
    /// Show the captcha (WebView on `redirect_url`, or image + text field),
    /// then call `answer_captcha(id, ...)` exactly once.
    Captcha {
        id: u64,
        redirect_url: Option<String>,
        image_url: Option<String>,
    },
    FocusAuthField {
        field: FfiAuthField,
        clear: bool,
    },
    SignedIn,
    ActiveTrackChanged,
    PlaybackSeeked,
    PlaylistDeleted {
        key: String,
    },
    PlaylistSaved {
        full_id: String,
        key: String,
    },
    EditorClosed,
    UploadFinished {
        all_done: bool,
    },
    ExportFinished {
        full_id: String,
        path: Option<String>,
        error: Option<String>,
    },
    /// If search results are still on screen, call `settle_search_history`.
    SearchSettled {
        query: String,
        generation: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiNetworkStatus {
    Unknown,
    Offline,
    Wifi,
    Cellular,
    Wired,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct FfiNetwork {
    pub status: FfiNetworkStatus,
    /// Metered connection.
    pub expensive: bool,
    /// Data saver / restricted background data.
    pub constrained: bool,
}

impl From<FfiNetwork> for NetworkState {
    fn from(n: FfiNetwork) -> Self {
        let status = match n.status {
            FfiNetworkStatus::Unknown => NetworkStatus::Unknown,
            FfiNetworkStatus::Offline => NetworkStatus::Offline,
            FfiNetworkStatus::Wifi => NetworkStatus::Online(NetworkInterface::Wifi),
            FfiNetworkStatus::Cellular => NetworkStatus::Online(NetworkInterface::Cellular),
            FfiNetworkStatus::Wired => NetworkStatus::Online(NetworkInterface::Wired),
            FfiNetworkStatus::Other => NetworkStatus::Online(NetworkInterface::Other),
        };
        NetworkState {
            status,
            expensive: n.expensive,
            constrained: n.constrained,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum FfiCaptchaNav {
    Success { token: Option<String> },
    Cancel,
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FfiError {
    // Not `message`: it would clash with Kotlin's `Throwable.message`.
    #[error("core init failed: {detail}")]
    Init { detail: String },
}
