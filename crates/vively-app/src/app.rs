use std::{
    collections::{BTreeMap, HashMap, HashSet},
    future::Future,
    ops::{BitOr, BitOrAssign},
    path::PathBuf,
    sync::{Arc, mpsc},
};

use vively_audio::DualAudioEngine;

use vively_core::{
    AudioService, VkClient,
    catalog::HomeBlock,
    downloads::{
        AutoCache, CacheUsage, CachedTrack, DownloadCollection, DownloadEngine, NetworkState, OfflineStore,
    },
    library::{MusicLibrary, StreamMixSession},
    lyrics::LyricsProvider,
    media::Mp3StreamDownloader,
    models::{Audio, Friend, MixSettings, Playlist, StreamMix},
    playback::{PlaybackQueue, PlaybackReporter, StatusBroadcaster},
    session::{Session, SessionStore},
    settings::{AppSettings, SearchHistory},
};

use crate::{
    auth::{AuthField, AuthState},
    captcha::CaptchaPrompt,
    models::{
        ArtistPageData, CatalogPageData, FriendMusicData, LoadState, LyricsLoadState,
        PlaylistDetailData,
        ProfileSearchData, RecentQuickItem, SearchResultsData, UploadItem, default_mix_settings,
    },
    runtime::TokioBridge,
};

/// Platform services the core needs from whoever embeds it.
pub trait Host: Send + Sync + 'static {
    /// Core state changed or a background job finished; the embedder should
    /// call [`AppCore::pump`] on the thread that owns the core. May be called
    /// from any thread.
    fn wake(&self);
    /// Current connectivity, used by auto-cache and downloads.
    fn network_state(&self) -> NetworkState;
}

/// One-shot notifications for the UI layer. State lives on [`AppCore`];
/// events are only for things that are not state (toasts, prompts, focus).
pub enum CoreEvent {
    Toast(String),
    CaptchaRequested(CaptchaPrompt),
    /// A login flow finished; the embedder activates the account.
    LoggedIn(Session),
    /// Move keyboard focus to an auth input, optionally clearing it first.
    FocusAuthField { field: AuthField, clear: bool },
    /// A playlist was deleted; leave its page if it is open.
    PlaylistDeleted { key: String },
    /// A typed query's results stayed up long enough. If search is still on
    /// screen, answer with [`AppCore::settle_search_history`].
    SearchSettled { query: String, generation: u64 },
    /// The current queue track changed (new load, crossfade promotion,
    /// restore). Reset per-track UI such as lyrics scroll and tint.
    ActiveTrackChanged,
    /// The position jumped (seek, restart); resume lyrics auto-scroll.
    PlaybackSeeked,
    /// The playlist editor saved; close it and show the playlist.
    PlaylistSaved { playlist: Playlist },
    /// The add-tracks editor finished; close it.
    EditorClosed,
    /// The upload batch finished; close the dialog if everything uploaded.
    UploadFinished { all_done: bool },
    /// An MP3 export finished for the track with this full id.
    ExportFinished {
        full_id: String,
        result: Result<PathBuf, String>,
    },
}

/// Which slices of state changed since the last pump.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Dirty(u32);

impl Dirty {
    pub const NONE: Self = Self(0);
    pub const SESSION: Self = Self(1 << 0);
    pub const AUTH: Self = Self(1 << 1);
    pub const SETTINGS: Self = Self(1 << 2);
    pub const HOME: Self = Self(1 << 3);
    pub const LIBRARY: Self = Self(1 << 4);
    pub const PLAYLISTS: Self = Self(1 << 5);
    pub const PAGES: Self = Self(1 << 6);
    pub const FRIENDS: Self = Self(1 << 7);
    pub const SEARCH: Self = Self(1 << 8);
    pub const DOWNLOADS: Self = Self(1 << 9);
    pub const PLAYBACK: Self = Self(1 << 10);
    pub const POSITION: Self = Self(1 << 11);
    pub const LYRICS: Self = Self(1 << 12);
    pub const TINT: Self = Self(1 << 13);
    pub const EDITOR: Self = Self(1 << 14);
    pub const UPLOAD: Self = Self(1 << 15);
    pub const ALL: Self = Self(u32::MAX);

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl BitOr for Dirty {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for Dirty {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

type Job = Box<dyn FnOnce(&mut AppCore) + Send>;

/// Cloneable handle for running futures on the core's tokio runtime and
/// applying their results back onto the core.
#[derive(Clone)]
pub struct Spawner {
    /// A handle, not the runtime: background tasks hold spawners, and the
    /// runtime must never be dropped from inside one of its own tasks.
    handle: tokio::runtime::Handle,
    tx: mpsc::Sender<Job>,
    host: Arc<dyn Host>,
}

impl Spawner {
    /// Run `fut` in the background, then run `apply` with its output on the
    /// core's owning thread during the next [`AppCore::pump`].
    pub fn spawn<T, F, A>(&self, fut: F, apply: A)
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
        A: FnOnce(&mut AppCore, T) + Send + 'static,
    {
        let tx = self.tx.clone();
        let host = Arc::clone(&self.host);
        self.handle.spawn(async move {
            let out = fut.await;
            let job: Job = Box::new(move |core| apply(core, out));
            if tx.send(job).is_ok() {
                host.wake();
            }
        });
    }

    /// Queue `apply` to run on the core's owning thread at the next pump,
    /// without a future (e.g. from a progress callback).
    pub fn post<A>(&self, apply: A)
    where
        A: FnOnce(&mut AppCore) + Send + 'static,
    {
        let job: Job = Box::new(apply);
        if self.tx.send(job).is_ok() {
            self.host.wake();
        }
    }

    /// Fire-and-forget background work whose result the core does not need.
    pub fn fire<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.handle.spawn(fut);
    }
}

/// Everything the core is built from. Grows as more state moves in.
pub struct CoreDeps {
    pub vk_client: VkClient,
    pub service: AudioService,
    pub session_store: Arc<SessionStore>,
    pub initial_session: Option<Session>,
    pub search_history: SearchHistory,
    pub search_history_path: PathBuf,
    pub settings: AppSettings,
    pub settings_path: PathBuf,
    pub queue_snapshot_path: PathBuf,
    pub queue_position_path: PathBuf,
    /// Built by the embedder (real or simulated output device).
    pub audio_engine: DualAudioEngine,
    pub lyrics_provider: Arc<tokio::sync::Mutex<LyricsProvider>>,
    /// Per-account data root (library copy history, offline store).
    pub data_dir: PathBuf,
    pub recent_links_path: PathBuf,
    pub exports_dir: PathBuf,
    pub downloader: Mp3StreamDownloader,
    /// Shown to the user on the phone that scans the login QR code.
    pub qr_device_name: String,
}

pub struct AppCore {
    pub tokio: TokioBridge,
    pub vk_client: VkClient,
    pub service: AudioService,
    pub session_store: Arc<SessionStore>,
    pub session: Option<Session>,
    /// Bumped on sign-out / account switch; stale completions compare it.
    pub account_generation: u64,
    pub auth: AuthState,

    pub home_state: LoadState,
    pub home_blocks: Vec<HomeBlock>,
    pub primary_mix: Option<StreamMix>,
    pub mix_settings: MixSettings,

    pub library: Arc<tokio::sync::Mutex<MusicLibrary>>,
    pub library_state: LoadState,
    pub library_tracks: Vec<Audio>,
    pub library_revision: u64,
    pub library_foreign_copies: HashMap<(i64, i64), i64>,
    pub library_pending_likes: HashSet<(i64, i64)>,
    pub library_total: i64,
    pub library_can_more: bool,
    pub library_loading_more: bool,
    pub library_more_failed: bool,
    pub restorable_own_track: Option<Audio>,

    pub playlists: Vec<Playlist>,
    pub playlists_state: LoadState,
    pub playlists_generation: u64,

    pub playlist_details: HashMap<String, PlaylistDetailData>,
    pub playlist_detail_generations: HashMap<String, u64>,
    pub artist_pages: HashMap<String, ArtistPageData>,
    pub catalog_pages: HashMap<String, CatalogPageData>,

    pub friends: Vec<Friend>,
    pub friends_total: i64,
    pub friends_state: LoadState,
    pub friend_pages: HashMap<i64, FriendMusicData>,

    pub search_query: String,
    pub search_state: LoadState,
    pub search_results: SearchResultsData,
    pub search_history: SearchHistory,
    pub search_history_path: PathBuf,
    pub search_generation: u64,

    pub settings: AppSettings,

    pub offline_store: Option<Arc<OfflineStore>>,
    pub download_engine: Option<Arc<DownloadEngine>>,
    pub ready_downloads: Vec<Audio>,
    pub pending_downloads: Vec<CachedTrack>,
    pub download_progress: HashMap<String, f64>,
    pub offline_collections: Vec<DownloadCollection>,
    pub cache_usage: CacheUsage,
    pub drain_in_flight: bool,
    pub drain_requested: bool,

    pub settings_path: PathBuf,
    pub queue_snapshot_path: PathBuf,
    pub queue_position_path: PathBuf,

    pub audio_engine: DualAudioEngine,
    pub queue: PlaybackQueue,
    pub reporter: PlaybackReporter,
    pub broadcaster: Arc<tokio::sync::Mutex<StatusBroadcaster>>,
    pub auto_cache: AutoCache,
    pub playing_from: String,
    pub active_mix_session: Option<StreamMixSession>,
    pub mix_picks: BTreeMap<String, Vec<String>>,
    pub current_elapsed: f64,
    pub pending_restore_pos: Option<f64>,
    pub track_loading: bool,
    pub load_generation: u64,
    pub mix_generation: u64,
    pub preloading_stable_id: Option<String>,
    pub extending_mix: bool,
    pub position_save_ticks: u32,

    pub lyrics_provider: Arc<tokio::sync::Mutex<LyricsProvider>>,
    pub lyrics_states: HashMap<String, LyricsLoadState>,

    pub data_dir: PathBuf,
    pub recent_items: Vec<RecentQuickItem>,
    pub recent_links_path: PathBuf,
    pub disliked_tracks: HashSet<(i64, i64)>,
    pub followed_owners: HashSet<i64>,
    pub profile_search: Option<ProfileSearchData>,
    /// "Add to playlist" picker: requests in flight / already added.
    pub add_pl_busy: HashSet<String>,
    pub add_pl_added: HashSet<String>,

    pub exports_dir: PathBuf,
    pub downloader: Mp3StreamDownloader,

    pub editor_picker_playlist: Option<Playlist>,
    pub editor_picker_playlists: Vec<Playlist>,
    pub editor_picker_tracks: Vec<Audio>,
    pub editor_picker_state: LoadState,
    pub editor_picker_more: bool,
    pub editor_picker_offset: usize,
    pub editor_picker_generation: u64,
    pub editor_error: Option<String>,
    pub editor_search_state: LoadState,
    pub editor_search_tracks: Vec<Audio>,
    pub editor_search_gen: u64,
    pub editor_draft_tracks: Vec<Audio>,
    pub editor_hidden: bool,
    pub editor_cover_bytes: Option<Vec<u8>>,
    pub editor_cover_preview: Option<PathBuf>,
    pub editor_saving: bool,

    pub upload_items: Vec<UploadItem>,
    pub upload_in_flight: bool,

    spawner: Spawner,
    jobs: mpsc::Receiver<Job>,
    host: Arc<dyn Host>,
    dirty: Dirty,
    events: Vec<CoreEvent>,
    pumping: bool,
    wake_requested: bool,
}

impl AppCore {
    pub fn new(host: Arc<dyn Host>, tokio: TokioBridge, deps: CoreDeps) -> Self {
        let (tx, jobs) = mpsc::channel();
        let spawner = Spawner {
            handle: tokio.handle(),
            tx,
            host: Arc::clone(&host),
        };
        let owner_id = deps.initial_session.as_ref().map_or(0, |s| s.user_id);
        Self {
            auth: AuthState::new(deps.vk_client.clone(), deps.qr_device_name),
            vk_client: deps.vk_client,
            library: Arc::new(tokio::sync::Mutex::new(MusicLibrary::new(
                deps.service.clone(),
                owner_id,
            ))),
            service: deps.service,
            session_store: deps.session_store,
            session: deps.initial_session,
            account_generation: 0,
            home_state: LoadState::Idle,
            home_blocks: Vec::new(),
            primary_mix: None,
            mix_settings: default_mix_settings(),
            library_state: LoadState::Idle,
            library_tracks: Vec::new(),
            library_revision: 0,
            library_foreign_copies: HashMap::new(),
            library_pending_likes: HashSet::new(),
            library_total: 0,
            library_can_more: false,
            library_loading_more: false,
            library_more_failed: false,
            restorable_own_track: None,
            playlists: Vec::new(),
            playlists_state: LoadState::Idle,
            playlists_generation: 0,
            playlist_details: HashMap::new(),
            playlist_detail_generations: HashMap::new(),
            artist_pages: HashMap::new(),
            catalog_pages: HashMap::new(),
            friends: Vec::new(),
            friends_total: 0,
            friends_state: LoadState::Idle,
            friend_pages: HashMap::new(),
            search_query: String::new(),
            search_state: LoadState::Idle,
            search_results: SearchResultsData::default(),
            search_history: deps.search_history,
            search_history_path: deps.search_history_path,
            search_generation: 0,
            settings: deps.settings,
            offline_store: None,
            download_engine: None,
            ready_downloads: Vec::new(),
            pending_downloads: Vec::new(),
            download_progress: HashMap::new(),
            offline_collections: Vec::new(),
            cache_usage: CacheUsage {
                total: 0,
                pinned: 0,
                track_count: 0,
            },
            drain_in_flight: false,
            drain_requested: false,
            settings_path: deps.settings_path,
            queue_snapshot_path: deps.queue_snapshot_path,
            queue_position_path: deps.queue_position_path,
            audio_engine: deps.audio_engine,
            queue: PlaybackQueue::default(),
            reporter: PlaybackReporter::default(),
            broadcaster: Arc::new(tokio::sync::Mutex::new(StatusBroadcaster::default())),
            auto_cache: AutoCache::default(),
            playing_from: "Моя музыка".into(),
            active_mix_session: None,
            mix_picks: BTreeMap::new(),
            current_elapsed: 0.0,
            pending_restore_pos: None,
            track_loading: false,
            load_generation: 0,
            mix_generation: 0,
            preloading_stable_id: None,
            extending_mix: false,
            position_save_ticks: 0,
            lyrics_provider: deps.lyrics_provider,
            lyrics_states: HashMap::new(),
            recent_items: std::fs::read(&deps.recent_links_path)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default(),
            recent_links_path: deps.recent_links_path,
            data_dir: deps.data_dir,
            disliked_tracks: HashSet::new(),
            followed_owners: HashSet::new(),
            profile_search: None,
            add_pl_busy: HashSet::new(),
            add_pl_added: HashSet::new(),
            exports_dir: deps.exports_dir,
            downloader: deps.downloader,
            editor_picker_playlist: None,
            editor_picker_playlists: Vec::new(),
            editor_picker_tracks: Vec::new(),
            editor_picker_state: LoadState::Idle,
            editor_picker_more: true,
            editor_picker_offset: 0,
            editor_picker_generation: 0,
            editor_error: None,
            editor_search_state: LoadState::Idle,
            editor_search_tracks: Vec::new(),
            editor_search_gen: 0,
            editor_draft_tracks: Vec::new(),
            editor_hidden: false,
            editor_cover_bytes: None,
            editor_cover_preview: None,
            editor_saving: false,
            upload_items: Vec::new(),
            upload_in_flight: false,
            tokio,
            spawner,
            jobs,
            host,
            dirty: Dirty::NONE,
            events: Vec::new(),
            pumping: false,
            wake_requested: false,
        }
    }

    pub fn spawner(&self) -> Spawner {
        self.spawner.clone()
    }

    pub fn spawn<T, F, A>(&self, fut: F, apply: A)
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
        A: FnOnce(&mut AppCore, T) + Send + 'static,
    {
        self.spawner.spawn(fut, apply);
    }

    pub fn fire<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.spawner.fire(fut);
    }

    /// Run `apply` on the core thread after `delay`. The timer is created
    /// inside the runtime: building `tokio::time::sleep` on the core/UI
    /// thread panics ("no reactor running").
    pub fn after<A>(&self, delay: std::time::Duration, apply: A)
    where
        A: FnOnce(&mut AppCore) + Send + 'static,
    {
        self.spawner
            .spawn(async move { tokio::time::sleep(delay).await }, move |core, ()| apply(core));
    }

    /// Changes made outside a pump (direct calls from the UI) still need the
    /// host to come back and drain events/dirty bits, so request one wake.
    fn request_wake(&mut self) {
        if !self.pumping && !self.wake_requested {
            self.wake_requested = true;
            self.host.wake();
        }
    }

    pub fn mark(&mut self, dirty: Dirty) {
        self.dirty |= dirty;
        self.request_wake();
    }

    pub fn emit(&mut self, event: CoreEvent) {
        self.events.push(event);
        self.request_wake();
    }

    pub fn toast(&mut self, message: impl Into<String>) {
        self.emit(CoreEvent::Toast(message.into()));
    }

    pub fn network_state(&self) -> NetworkState {
        self.host.network_state()
    }

    /// Apply every completed background job, returning what changed
    /// (including anything marked before the pump). Follow with
    /// [`AppCore::take_events`].
    pub fn pump(&mut self) -> Dirty {
        self.pumping = true;
        self.wake_requested = false;
        loop {
            let jobs: Vec<Job> = self.jobs.try_iter().collect();
            if jobs.is_empty() {
                break;
            }
            for job in jobs {
                job(self);
            }
        }
        self.pumping = false;
        self.take_dirty()
    }

    pub fn take_dirty(&mut self) -> Dirty {
        std::mem::take(&mut self.dirty)
    }

    pub fn has_events(&self) -> bool {
        !self.events.is_empty()
    }

    pub fn take_events(&mut self) -> Vec<CoreEvent> {
        std::mem::take(&mut self.events)
    }
}
