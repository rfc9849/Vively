use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use gpui_kit::{
    App, AppContext as _, Bounds, Context, Entity, FocusHandle, Focusable, Pixels, ScrollHandle,
    Subscription, UniformListScrollHandle, Window,
    component::input::{InputEvent, InputState, TextareaState},
    point, px,
};
use gpui_webview::WebView;
use lb_wry::WebViewBuilder;
use url::Url;
pub use vively_app::{FileSessionPersistence, auth::encode_qr_matrix};
use vively_app::{AppCore, AuthField, CoreEvent, Host};
use vively_audio::{
    MediaCommand, OsMediaControls, };
use vively_core::{
    VkClient,
    config::{ANDROID_CLIENT_SECRET, USER_AGENT},
    library::StreamMixSession,
    lyrics::LyricsContent,
    models::{
        Album, Audio, CatalogLink, Friend, MusicDestination, Playlist,
    },
    playback::PlaybackOrigin,
    session::{
        DeviceProfile, Session, SessionPersistence, },
    settings::Appearance,
};

use crate::design::{
    artwork::{ArtworkCache, CoverTint, SampledTint, encode_cover_jpeg},
    colors::{DEFAULT_TINT, Palette, RgbColor, parse_hex_color, resolve_is_dark, sync_gpui_theme},
    image_cache::{BoundedImageCache, DECODED_IMAGE_BUDGET},
};

use super::{
    models::{
        UploadInputs, AuthStep, LoadState,
        QrStage,
        LyricsLoadState, LyricsScrollState, active_synced_lyric_index,
    },
    router::{
        ActiveModal, EditorTrackSource, ExportState, PanelTab, Route, RouterState, SectionTarget, ToastItem,
    },
    runtime::{
        AppPaths, CaptchaNavOutcome, CaptchaPrompt, TokioBridge,
        captcha_interceptor_script, extract_captcha_success_token,
        parse_captcha_nav_outcome, probe_network_state, reveal_in_file_manager,
    },
};

pub struct ActiveCaptchaState {
    pub prompt: CaptchaPrompt,
    pub webview: Option<Entity<WebView>>,
}

pub type TrackQueueCache = HashMap<String, (u64, Rc<[Audio]>)>;

pub struct VivelyApp {
    pub focus_handle: FocusHandle,
    pub paths: AppPaths,
    pub tokio: TokioBridge,
    pub vk_client: VkClient,
    pub core: AppCore,
    pub artwork: ArtworkCache,
    pub image_cache: Entity<BoundedImageCache>,
    pub track_list_tops: Rc<RefCell<HashMap<String, Pixels>>>,
    pub track_queue_cache: RefCell<TrackQueueCache>,

    pub router: RouterState,
    pub canvas_scroll: ScrollHandle,
    pub right_scroll: ScrollHandle,
    pub stage_scroll: ScrollHandle,
    pub panel_queue_scroll: ScrollHandle,
    pub stage_queue_scroll: ScrollHandle,
    pub panel_lyrics_scroll: Rc<RefCell<LyricsScrollState>>,
    pub stage_lyrics_scroll: Rc<RefCell<LyricsScrollState>>,

    pub active_tint: SampledTint,
    last_theme_key: Option<(bool, [u8; 3], bool)>,
    pub last_scrub_bounds: Option<Bounds<Pixels>>,
    pub last_stage_scrub_bounds: Option<Bounds<Pixels>>,
    pub scrub_hover_ratio: Option<f32>,
    pub stage_scrub_hover_ratio: Option<f32>,
    pub scrub_drag_ratio: Option<f32>,
    pub stage_scrub_drag_ratio: Option<f32>,
    pub last_vol_bounds: Option<Bounds<Pixels>>,
    pub last_stage_vol_bounds: Option<Bounds<Pixels>>,
    pub last_xfade_bounds: Option<Bounds<Pixels>>,

    pub library_query: String,
    pub collection_query: String,

    pub library_playlists_expanded: bool,
    pub friend_playlists_expanded: bool,
    pub artist_name_hints: HashMap<String, String>,

    pub downloads_query: String,

    pub media_controls: OsMediaControls,
    media_cmd_rx: tokio::sync::mpsc::UnboundedReceiver<MediaCommand>,

    pub active_captcha: Option<ActiveCaptchaState>,

    pub editor_source: EditorTrackSource,
    pub editor_picker_open: bool,
    pub editor_picker_scroll: UniformListScrollHandle,
    pub editor_draft_scroll: ScrollHandle,
    pub editor_picker_query: String,
    pub add_pl_query: String,
    pub add_pl_scroll: ScrollHandle,

    pub upload_inputs: Vec<UploadInputs>,

    pub toasts: Vec<ToastItem>,
    next_toast_id: u64,

    pub top_search_input: Entity<InputState>,
    pub lib_filter_input: Entity<InputState>,
    pub profile_search_input: Entity<InputState>,
    pub col_filter_input: Entity<InputState>,
    pub dl_filter_input: Entity<InputState>,
    pub add_pl_filter_input: Entity<InputState>,
    pub auth_phone_input: Entity<InputState>,
    pub auth_code_input: Entity<InputState>,
    pub auth_password_input: Entity<InputState>,
    pub pl_title_input: Entity<InputState>,
    pub pl_desc_input: Entity<TextareaState>,
    pub pl_picker_search_input: Entity<InputState>,
    pub captcha_manual_input: Entity<InputState>,

    _subscriptions: Vec<Subscription>,
}

impl Focusable for VivelyApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl VivelyApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> anyhow::Result<Self> {
        let paths = AppPaths::discover();
        seed_legacy_macos_device_id(&paths.device_file());
        let persistence: Arc<dyn SessionPersistence> = Arc::new(FileSessionPersistence {
            path: paths.session_file(),
        });
        Self::new_with_config(paths, persistence, false, window, cx)
    }

    pub fn new_with_config(
        paths: AppPaths,
        persistence: Arc<dyn SessionPersistence>,
        simulated_audio: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<Self> {
        let tokio = TokioBridge::new()?;
        let secret = std::env::var("VIVELY_VK_CLIENT_SECRET")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| ANDROID_CLIENT_SECRET.to_owned());
        let (wake_tx, mut wake_rx) = futures_channel::mpsc::unbounded::<()>();
        let core = vively_app::build_core(
            Arc::new(DesktopHost { wake: wake_tx }),
            tokio.shared(),
            vively_app::CoreConfig {
                paths: vively_app::CorePaths::new(paths.config_dir.clone(), paths.data_dir.clone()),
                persistence,
                simulated_audio,
                qr_device_name: qr_device_name().into(),
                client_secret: secret,
                dev_session: paths.load_dev_session(),
                api_host: None,
            },
        )?;
        let vk_client = core.vk_client.clone();
        let initial_session = core.session.clone();
        let artwork = ArtworkCache::new(paths.artwork_cache_dir(), core.downloader.client().clone());

        let (media_controls, media_cmd_rx) = if simulated_audio {
            OsMediaControls::disabled()
        } else {
            OsMediaControls::new()
        };

        let top_search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Треки, альбомы, артисты..."));
        let lib_filter_input = cx.new(|cx| InputState::new(window, cx).placeholder("Фильтр"));
        let col_filter_input = cx.new(|cx| InputState::new(window, cx).placeholder("Фильтр"));
        let profile_search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Поиск по музыке"));
        let dl_filter_input = cx.new(|cx| InputState::new(window, cx).placeholder("Фильтр"));
        let add_pl_filter_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Найти плейлист…"));
        let auth_phone_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("+7 999 000-00-00"));
        let auth_code_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Код подтверждения"));
        let auth_password_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Пароль")
                .masked(true)
        });
        let pl_title_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Название плейлиста"));
        let pl_desc_input =
            cx.new(|cx| TextareaState::new(window, cx).placeholder("Описание (необязательно)"));
        let pl_picker_search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Фильтр по моей музыке…"));
        let captcha_manual_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Код или токен с картинки"));

        let image_cache = BoundedImageCache::new(DECODED_IMAGE_BUDGET, cx);
        let mut subscriptions = Vec::new();
        subscriptions.push(window.observe_window_visibility({
            let image_cache = image_cache.clone();
            move |_, window, cx| {
                if !window.is_visible() {
                    image_cache.update(cx, |cache, cx| cache.clear(window, cx));
                    window.refresh();
                }
            }
        }));
        subscriptions.push(cx.subscribe_in(&top_search_input, window, {
            let input = top_search_input.clone();
            move |this, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::Focus => {
                    if this.router.current() != &Route::Search {
                        this.navigate_to(Route::Search, window, cx);
                    }
                }
                InputEvent::Change => {
                    let q = input.read(cx).value().to_string();
                    this.on_search_query_edited(q, window, cx);
                }
                InputEvent::PressEnter { .. } => {
                    this.commit_search_now(window, cx);
                }
                _ => {}
            }
        }));

        subscriptions.push(cx.subscribe_in(&lib_filter_input, window, {
            let input = lib_filter_input.clone();
            move |this, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.library_query = input.read(cx).value().to_string();
                    cx.notify();
                }
            }
        }));

        subscriptions.push(cx.subscribe_in(&profile_search_input, window, {
            let input = profile_search_input.clone();
            move |this, _, ev: &InputEvent, _, cx| match ev {
                InputEvent::Change => {
                    if input.read(cx).value().trim().is_empty() && this.core.profile_search.is_some() {
                        this.core.profile_search = None;
                        cx.notify();
                    }
                }
                InputEvent::PressEnter { .. } => {
                    let query = input.read(cx).value().trim().to_string();
                    if let Route::FriendMusic(owner_id) = this.router.current().clone() {
                        this.search_in_profile(owner_id, query, cx);
                    }
                }
                _ => {}
            }
        }));

        subscriptions.push(cx.subscribe_in(&col_filter_input, window, {
            let input = col_filter_input.clone();
            move |this, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.collection_query = input.read(cx).value().to_string();
                    cx.notify();
                }
            }
        }));

        subscriptions.push(cx.subscribe_in(&dl_filter_input, window, {
            let input = dl_filter_input.clone();
            move |this, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.downloads_query = input.read(cx).value().to_string();
                    cx.notify();
                }
            }
        }));

        subscriptions.push(cx.subscribe_in(&add_pl_filter_input, window, {
            let input = add_pl_filter_input.clone();
            move |this, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.add_pl_query = input.read(cx).value().to_string();
                    cx.notify();
                }
            }
        }));

        subscriptions.push(cx.subscribe_in(&auth_phone_input, window, {
            move |this, _, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::PressEnter { .. }) {
                    this.submit_auth_primary(window, cx);
                } else if matches!(ev, InputEvent::Change) {
                    cx.notify();
                }
            }
        }));

        subscriptions.push(cx.subscribe_in(&auth_code_input, window, {
            move |this, _, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::PressEnter { .. }) {
                    this.submit_auth_primary(window, cx);
                } else if matches!(ev, InputEvent::Change) {
                    cx.notify();
                }
            }
        }));

        subscriptions.push(cx.subscribe_in(&auth_password_input, window, {
            move |this, _, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::PressEnter { .. }) {
                    this.submit_auth_primary(window, cx);
                } else if matches!(ev, InputEvent::Change) {
                    cx.notify();
                }
            }
        }));

        subscriptions.push(cx.subscribe_in(
            &pl_title_input,
            window,
            |this, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::Change => {
                    if this.core.editor_error.is_some() {
                        this.core.editor_error = None;
                    }
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => {
                    match &this.router.modal {
                        Some(ActiveModal::PlaylistEditor { editing: Some(_) }) => {
                            this.save_playlist_editor(window, cx);
                        }
                        Some(ActiveModal::PlaylistEditor { editing: None }) => {
                            this.pl_picker_search_input
                                .update(cx, |input, cx| input.focus(window, cx));
                        }
                        _ => {}
                    }
                }
                _ => {}
            },
        ));

        subscriptions.push(cx.subscribe_in(&pl_picker_search_input, window, {
            let input = pl_picker_search_input.clone();
            move |this, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change | InputEvent::PressEnter { .. }) {
                    let q = input.read(cx).value().to_string();
                    let immediate = matches!(ev, InputEvent::PressEnter { .. });
                    this.on_editor_picker_query_change(q, immediate, cx);
                }
            }
        }));

        let mut app = Self {
            focus_handle: cx.focus_handle(),
            paths,
            tokio: tokio.clone(),
            vk_client,
            core,
            artwork,
            image_cache,
            track_list_tops: Rc::default(),
            track_queue_cache: RefCell::default(),
            router: RouterState::default(),
            canvas_scroll: ScrollHandle::new(),
            right_scroll: ScrollHandle::new(),
            stage_scroll: ScrollHandle::new(),
            panel_queue_scroll: ScrollHandle::new(),
            stage_queue_scroll: ScrollHandle::new(),
            panel_lyrics_scroll: Rc::default(),
            stage_lyrics_scroll: Rc::default(),
            active_tint: SampledTint::default(),
            last_theme_key: None,
            last_scrub_bounds: None,
            last_stage_scrub_bounds: None,
            scrub_hover_ratio: None,
            stage_scrub_hover_ratio: None,
            scrub_drag_ratio: None,
            stage_scrub_drag_ratio: None,
            last_vol_bounds: None,
            last_stage_vol_bounds: None,
            last_xfade_bounds: None,
            library_query: String::new(),
            collection_query: String::new(),
            library_playlists_expanded: false,
            friend_playlists_expanded: false,
            artist_name_hints: HashMap::new(),
            downloads_query: String::new(),
            media_controls,
            media_cmd_rx,
            active_captcha: None,
            editor_source: EditorTrackSource::Likes,
            editor_picker_open: false,
            editor_picker_scroll: UniformListScrollHandle::new(),
            editor_draft_scroll: ScrollHandle::new(),
            editor_picker_query: String::new(),
            add_pl_query: String::new(),
            add_pl_scroll: ScrollHandle::new(),
            upload_inputs: Vec::new(),
            toasts: Vec::new(),
            next_toast_id: 1,
            top_search_input,
            lib_filter_input,
            profile_search_input,
            col_filter_input,
            dl_filter_input,
            add_pl_filter_input,
            auth_phone_input,
            auth_code_input,
            auth_password_input,
            pl_title_input,
            pl_desc_input,
            pl_picker_search_input,
            captcha_manual_input,
            _subscriptions: subscriptions,
        };

        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                let ok = this.update_in(cx, |this, window, cx| {
                    this.poll_channels(window, cx);
                    this.on_periodic_tick(cx);
                });
                if ok.is_err() {
                    break;
                }
            }
        })
        .detach();

        cx.spawn_in(window, async move |this, cx| {
            while wake_rx.next().await.is_some() {
                if this
                    .update_in(cx, |this, window, cx| this.pump_core(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        if let Some(session) = initial_session {
            app.activate_signed_in_session(session, cx);
        }

        Ok(app)
    }

    pub fn palette(&self, window: Option<&Window>) -> Palette {
        let is_dark = resolve_is_dark(self.core.settings.appearance, window.map(|w| w.appearance()));
        Palette::resolve(is_dark, self.active_tint.color, self.active_tint.is_neutral)
    }

    pub fn sync_theme(&mut self, window: Option<&Window>, cx: &mut App) -> Palette {
        let pal = self.palette(window);
        let key = (
            pal.is_dark,
            [
                (pal.tint.r * 255.0) as u8,
                (pal.tint.g * 255.0) as u8,
                (pal.tint.b * 255.0) as u8,
            ],
            pal.is_neutral,
        );
        if self.last_theme_key != Some(key) {
            self.last_theme_key = Some(key);
            sync_gpui_theme(&pal, cx);
        }
        pal
    }

    pub fn access_token(&self) -> Option<String> {
        self.core.access_token()
    }

    pub fn signed_in_user_id(&self) -> Option<i64> {
        self.core.signed_in_user_id()
    }

    pub fn push_toast(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        let id = self.next_toast_id;
        self.next_toast_id += 1;
        self.toasts.push(ToastItem {
            id,
            message: message.into(),
            expires_at: Instant::now() + Duration::from_millis(2800),
        });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
        cx.notify();
    }

    pub fn resolve_artwork(&self, url: Option<&Url>, _cx: &mut Context<Self>) -> Option<PathBuf> {
        let url = url?;
        if let Some(path) = self.artwork.lookup_memory(url) {
            return Some(path);
        }
        self.start_artwork_fetch(url);
        None
    }

    pub fn resolve_full_artwork(
        &self,
        url: Option<&Url>,
        _cx: &mut Context<Self>,
    ) -> Option<PathBuf> {
        let url = url?;
        if let Some(path) = self.artwork.lookup_full(url) {
            return Some(path);
        }
        self.start_artwork_fetch(url);
        None
    }

    fn start_artwork_fetch(&self, url: &Url) {
        if self.artwork.begin_fetch(url) {
            let cache = self.artwork.clone();
            let target_url = url.clone();
            let offline_cand = self
                .core.offline_store
                .as_ref()
                .map(|s| s.paths.artwork_file(url));
            let is_active_cover = self
                .core.queue
                .current()
                .and_then(|t| t.artwork_url())
                .is_some_and(|u| u == url);
            self.tokio.handle().spawn(async move {
                let _ = cache
                    .fetch_or_load(target_url, offline_cand, is_active_cover)
                    .await;
            });
        }
    }

    pub fn tint_for_artwork(&self, url: Option<&Url>, vk_hex: Option<&str>) -> Option<RgbColor> {
        if let Some(url) = url
            && let Some(sampled) = self.artwork.cached_tint(url)
        {
            return (!sampled.is_neutral).then_some(sampled.color);
        }
        if let Some(hex) = vk_hex
            && let Some(c) = parse_hex_color(hex)
        {
            return Some(c);
        }
        None
    }

    pub fn refresh_active_track_tint(&mut self) {
        let Some(track) = self.core.queue.current() else {
            self.active_tint = SampledTint::default();
            return;
        };
        if let Some(url) = track.artwork_url() {
            match self.artwork.tint_status(url) {
                CoverTint::Ready(tint) => {
                    self.active_tint = tint;
                    return;
                }
                CoverTint::Pending => {
                    if let Some(path) = self.artwork.begin_tint_sample(url) {
                        let cache = self.artwork.clone();
                        let url = url.clone();
                        self.tokio.handle().spawn_blocking(move || {
                            let _ = cache.sample_file_off_thread(url, path);
                        });
                    } else {
                        self.start_artwork_fetch(url);
                    }
                    return;
                }
                CoverTint::Unavailable => {}
            }
        }
        if let Some(hex) = track.album.as_ref().and_then(|a| a.main_color.as_deref())
            && let Some(color) = parse_hex_color(hex)
        {
            self.active_tint = SampledTint {
                color,
                is_neutral: false,
            };
            return;
        }
        self.active_tint = SampledTint::default();
    }

    fn activate_signed_in_session(&mut self, session: Session, cx: &mut Context<Self>) {
        self.core.activate_session(session);
        cx.notify();
    }

    pub fn reset_account_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.core.reset_account();
        self.reset_account_ui(window, cx);
    }

    /// Desktop-only per-account state (inputs, scroll, tint, routing).
    fn reset_account_ui(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor_picker_open = false;
        self.editor_picker_query.clear();
        self.collection_query.clear();
        self.add_pl_query.clear();
        self.library_query.clear();
        self.library_playlists_expanded = false;
        self.artist_name_hints.clear();
        self.upload_inputs.clear();
        self.reset_lyrics_scroll();
        self.top_search_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.lib_filter_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.dl_filter_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.active_tint = SampledTint {
            color: DEFAULT_TINT,
            is_neutral: true,
        };
        self.track_list_tops.borrow_mut().clear();
        self.track_queue_cache.get_mut().clear();
        self.router.reset();
    }

    pub fn sign_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.core.end_session();
        self.reset_account_state(window, cx);
        cx.notify();
    }

    pub fn navigate_to(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        let changed = self.router.navigate(route.clone());
        if changed {
            self.clear_collection_filter(window, cx);
        }
        self.reset_canvas_scroll(changed);
        self.ensure_route_loaded(&route, cx);
        cx.notify();
    }

    pub fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(route) = self.router.go_back() {
            self.clear_collection_filter(window, cx);
            self.reset_canvas_scroll(true);
            self.ensure_route_loaded(&route, cx);
            cx.notify();
        }
    }

    pub fn go_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(route) = self.router.go_forward() {
            self.clear_collection_filter(window, cx);
            self.reset_canvas_scroll(true);
            self.ensure_route_loaded(&route, cx);
            cx.notify();
        }
    }

    pub fn clear_collection_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.collection_query.is_empty() {
            self.collection_query.clear();
            self.col_filter_input
                .update(cx, |s, cx| s.set_value("", window, cx));
        }
    }

    pub fn clear_library_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.library_query.is_empty() {
            self.library_query.clear();
            self.lib_filter_input
                .update(cx, |s, cx| s.set_value("", window, cx));
            cx.notify();
        }
    }

    pub fn clear_downloads_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.downloads_query.is_empty() {
            self.downloads_query.clear();
            self.dl_filter_input
                .update(cx, |s, cx| s.set_value("", window, cx));
            cx.notify();
        }
    }

    pub fn search_in_vk(&mut self, query: String, window: &mut Window, cx: &mut Context<Self>) {
        let trimmed = query.trim().to_owned();
        self.top_search_input
            .update(cx, |s, cx| s.set_value(trimmed, window, cx));
        self.commit_search_now(window, cx);
    }

    fn reset_canvas_scroll(&mut self, route_changed: bool) {
        if route_changed {
            self.track_list_tops.borrow_mut().clear();
            self.track_queue_cache.get_mut().clear();
        }
        self.canvas_scroll
            .set_offset(gpui_kit::point(gpui_kit::px(0.0), gpui_kit::px(0.0)));
    }

    pub fn refresh_current_route(&mut self, cx: &mut Context<Self>) {
        let route = self.router.current().clone();
        match route {
            Route::Home => self.load_home(true, cx),
            Route::Library => {
                self.load_library(true, cx);
                self.load_playlists(true, cx);
            }
            Route::Friends => self.load_friends(true, cx),
            Route::FriendMusic(fid) => {
                if let Some(f) = self.friend_profile(fid) {
                    self.load_friend_music(f, true, cx);
                }
            }
            Route::Collection(id) => {
                if let Some(pl) = self.find_playlist_by_any_id(&id) {
                    self.load_playlist_detail(pl, true, cx);
                }
            }
            Route::Artist(aid) => self.load_artist(&aid, true, cx),
            Route::Section(target) => self.load_section(target, true, cx),
            Route::Downloads | Route::DownloadedCol(_) => {
                self.sync_downloads_snapshot();
                self.trigger_downloads_drain(cx);
                cx.notify();
            }
            Route::Search => {
                if !self.core.search_query.trim().is_empty() {
                    let q = self.core.search_query.clone();
                    self.run_search_request(q, false, cx);
                }
            }
        }
    }

    fn ensure_route_loaded(&mut self, route: &Route, cx: &mut Context<Self>) {
        match route {
            Route::Home => self.load_home(false, cx),
            Route::Library => {
                self.load_library(false, cx);
                self.load_playlists(false, cx);
            }
            Route::Friends => self.load_friends(false, cx),
            Route::FriendMusic(fid) => {
                if let Some(f) = self.friend_profile(*fid) {
                    self.load_friend_music(f, false, cx);
                }
            }
            Route::Collection(id) => {
                if let Some(pl) = self.find_playlist_by_any_id(id) {
                    self.load_playlist_detail(pl, false, cx);
                }
            }
            Route::Artist(aid) => self.load_artist(aid, false, cx),
            Route::Section(target) => self.load_section(target.clone(), false, cx),
            Route::Downloads | Route::DownloadedCol(_) => {
                self.sync_downloads_snapshot();
            }
            Route::Search => {}
        }
    }

    /// Apply finished core jobs and route core events into gpui.
    pub fn pump_core(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dirty = self.core.pump();
        let events = self.core.take_events();
        let had_events = !events.is_empty();
        for event in events {
            match event {
                CoreEvent::Toast(message) => self.push_toast(message, cx),
                CoreEvent::CaptchaRequested(prompt) => self.present_captcha(prompt, window, cx),
                CoreEvent::LoggedIn(session) => self.finish_login(session, window, cx),
                CoreEvent::FocusAuthField { field, clear } => {
                    let input = match field {
                        AuthField::Phone => self.auth_phone_input.clone(),
                        AuthField::Code => self.auth_code_input.clone(),
                        AuthField::Password => self.auth_password_input.clone(),
                    };
                    input.update(cx, |s, cx| {
                        if clear {
                            s.set_value("", window, cx);
                        }
                        s.focus(window, cx);
                    });
                }
                CoreEvent::PlaylistDeleted { key } => {
                    if matches!(self.router.current(), Route::Collection(cur) if cur == &key) {
                        self.navigate_to(Route::Library, window, cx);
                    }
                }
                CoreEvent::PlaylistSaved { playlist } => {
                    if matches!(self.router.modal, Some(ActiveModal::PlaylistEditor { .. })) {
                        self.router.modal = None;
                    }
                    self.open_playlist(playlist, window, cx);
                }
                CoreEvent::EditorClosed => {
                    if matches!(self.router.modal, Some(ActiveModal::AddTracksToPlaylist { .. })) {
                        self.router.modal = None;
                    }
                }
                CoreEvent::UploadFinished { all_done } => {
                    if all_done && matches!(self.router.modal, Some(ActiveModal::AudioUpload)) {
                        self.router.modal = None;
                    }
                }
                CoreEvent::ExportFinished { full_id, result } => {
                    if let Some(ActiveModal::ShareFile { track, state }) = &mut self.router.modal
                        && track.full_id() == full_id
                    {
                        *state = match result {
                            Ok(path) => ExportState::Ready(path),
                            Err(err) => ExportState::Failed(err),
                        };
                    }
                }
                CoreEvent::ActiveTrackChanged => {
                    self.reset_lyrics_scroll();
                    self.refresh_active_track_tint();
                }
                CoreEvent::PlaybackSeeked => self.resume_lyrics_auto_scroll(),
                CoreEvent::SearchSettled { query, generation } => {
                    if self.router.current() == &Route::Search {
                        self.core.settle_search_history(&query, generation);
                    }
                }
            }
        }
        if had_events || !dirty.is_empty() {
            cx.notify();
        }
    }

    pub fn submit_auth_primary(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = match &self.core.auth.step {
            AuthStep::Phone => Some(&self.auth_phone_input),
            AuthStep::Otp { .. } | AuthStep::Qr(QrStage::ConfirmCode { .. }) => {
                Some(&self.auth_code_input)
            }
            AuthStep::Password { .. } => Some(&self.auth_password_input),
            AuthStep::Qr(_) => None,
        };
        let input = field
            .map(|f| f.read(cx).value().to_string())
            .unwrap_or_default();
        self.core.submit_auth(&input);
        self.pump_core(window, cx);
    }

    pub fn resend_auth_otp(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.core.resend_auth_otp();
        self.pump_core(window, cx);
    }

    pub fn start_qr_auth(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.core.start_qr_auth();
        self.pump_core(window, cx);
    }

    pub fn cancel_qr_auth(&mut self) {
        self.core.cancel_qr_auth();
    }

    pub fn auth_go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.core.auth_go_back();
        self.pump_core(window, cx);
    }

    fn finish_login(&mut self, session: Session, window: &mut Window, cx: &mut Context<Self>) {
        let switched = self
            .core
            .session
            .as_ref()
            .is_some_and(|prev| prev.user_id != session.user_id);
        if switched {
            self.reset_account_ui(window, cx);
        }
        if matches!(self.router.modal, Some(ActiveModal::Auth)) {
            self.router.modal = None;
        }
        self.core.adopt_login(session);
        cx.notify();
    }

    pub fn present_captcha(
        &mut self,
        prompt: CaptchaPrompt,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(prev) = self.active_captcha.take() {
            if let Some(wv) = prev.webview {
                wv.update(cx, |w, _| w.hide());
            }
            let _ = prev.prompt.reply.send(None);
        }

        let mut webview_entity = None;
        if let Some(redirect_url) = prompt.challenge.redirect_url.clone() {
            let (tok_tx, mut tok_rx) = futures_channel::mpsc::unbounded::<CaptchaNavOutcome>();
            let ipc_tx = tok_tx.clone();
            let nav_tx = tok_tx;
            let url_str = redirect_url.to_string();

            let builder = WebViewBuilder::new()
                .with_url(&url_str)
                .with_user_agent(USER_AGENT)
                .with_initialization_script(captcha_interceptor_script())
                .with_ipc_handler(move |req| {
                    let body = req.body();
                    if let Some(token) = extract_captcha_success_token(body) {
                        let _ = ipc_tx.unbounded_send(CaptchaNavOutcome::Success(Some(token)));
                    }
                })
                .with_navigation_handler(move |uri| {
                    if let Some(outcome) = parse_captcha_nav_outcome(&uri) {
                        let _ = nav_tx.unbounded_send(outcome);
                    }
                    true
                });

            if let Ok(webview) = WebView::build(builder, window, cx) {
                webview_entity = Some(cx.new(|_| webview));

                let prompt_id = prompt.id;
                cx.spawn(async move |this, cx| {
                    while let Some(outcome) = tok_rx.next().await {
                        let done = this.update(cx, |this, cx| {
                            if this
                                .active_captcha
                                .as_ref()
                                .is_some_and(|c| c.prompt.id == prompt_id)
                            {
                                match outcome {
                                    CaptchaNavOutcome::Success(Some(tok)) => {
                                        this.finish_captcha(Some(tok), cx);
                                        return true;
                                    }
                                    CaptchaNavOutcome::Cancel => {
                                        this.finish_captcha(None, cx);
                                        return true;
                                    }
                                    CaptchaNavOutcome::Success(None) => {}
                                }
                            }
                            false
                        });
                        if matches!(done, Ok(true) | Err(_)) {
                            break;
                        }
                    }
                })
                .detach();
            }
        }

        self.captcha_manual_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.active_captcha = Some(ActiveCaptchaState {
            prompt,
            webview: webview_entity,
        });
        cx.notify();
    }

    pub fn finish_captcha(&mut self, answer: Option<String>, cx: &mut Context<Self>) {
        if let Some(active) = self.active_captcha.take() {
            if let Some(wv) = active.webview {
                wv.update(cx, |w, _| w.hide());
            }
            let _ = active.prompt.reply.send(answer);
            cx.notify();
        }
    }

    pub fn submit_manual_captcha(&mut self, cx: &mut Context<Self>) {
        let val = self.captcha_manual_input.read(cx).value().trim().to_owned();
        if val.is_empty() {
            return;
        }
        if let Ok(url) = Url::parse(&val)
            && matches!(url.scheme(), "https" | "http" | "vk" | "vkconnect")
        {
            match parse_captcha_nav_outcome(&val) {
                Some(CaptchaNavOutcome::Success(Some(token))) => {
                    self.finish_captcha(Some(token), cx)
                }
                Some(CaptchaNavOutcome::Cancel) => self.finish_captcha(None, cx),
                _ => self.push_toast(
                    "В ссылке нет кода подтверждения. Скопируйте адрес после прохождения проверки.",
                    cx,
                ),
            }
        } else {
            self.finish_captcha(Some(val), cx);
        }
    }

    pub fn load_home(&mut self, force: bool, cx: &mut Context<Self>) {
        self.core.load_home(force);
        cx.notify();
    }

    pub fn load_library(&mut self, force: bool, cx: &mut Context<Self>) {
        self.core.load_library(force);
        cx.notify();
    }

    pub fn load_more_library(&mut self, cx: &mut Context<Self>) {
        let picker = matches!(
            self.router.modal,
            Some(
                ActiveModal::PlaylistEditor { editing: None }
                    | ActiveModal::AddTracksToPlaylist { .. }
            )
        ) && self.editor_source == EditorTrackSource::Likes;
        let library =
            self.router.current() == &Route::Library && self.library_query.trim().is_empty();
        if !(library || picker) {
            return;
        }
        self.core.load_more_library();
        cx.notify();
    }

    pub fn load_playlists(&mut self, force: bool, cx: &mut Context<Self>) {
        self.core.load_playlists(force);
        cx.notify();
    }

    pub fn find_playlist_by_any_id(&self, id: &str) -> Option<Playlist> {
        self.core.find_playlist_by_any_id(id)
    }

    pub fn open_playlist(
        &mut self,
        playlist: Playlist,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = playlist.stable_id();
        self.record_recent_playlist(&playlist);
        self.core.ensure_playlist_entry(&playlist);
        self.load_playlist_detail(playlist, false, cx);
        self.navigate_to(Route::Collection(key), window, cx);
    }

    pub fn play_playlist_immediately(
        &mut self,
        playlist: Playlist,
        offline_only: bool,
        cx: &mut Context<Self>,
    ) {
        self.core.play_playlist_immediately(playlist, offline_only);
        cx.notify();
    }

    pub fn open_album(&mut self, album: &Album, window: &mut Window, cx: &mut Context<Self>) {
        if album.id.unwrap_or(0) == 0 {
            return;
        }
        let playlist = Playlist::from_album(album);
        self.open_playlist(playlist, window, cx);
    }

    pub fn load_playlist_detail(
        &mut self,
        playlist: Playlist,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        self.core.load_playlist_detail(playlist, force);
        cx.notify();
    }

    pub fn load_more_playlist_tracks(&mut self, key: &str, cx: &mut Context<Self>) {
        self.core.load_more_playlist_tracks(key);
        cx.notify();
    }

    pub fn open_artist(
        &mut self,
        artist_id: impl Into<String>,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = artist_id.into();
        if id.is_empty() {
            return;
        }
        let name = name.trim();
        if !name.is_empty() && name != id {
            self.artist_name_hints.insert(id.clone(), name.to_owned());
        }
        self.load_artist(&id, false, cx);
        self.navigate_to(Route::Artist(id), window, cx);
    }

    pub fn load_artist(&mut self, artist_id: &str, force: bool, cx: &mut Context<Self>) {
        self.core.load_artist(artist_id, force);
        cx.notify();
    }

    pub fn open_catalog_link(
        &mut self,
        link: &CatalogLink,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if link.is_my_audios() {
            self.navigate_to(Route::Library, window, cx);
            return;
        }
        if let Some(domain) = link.artist_domain() {
            self.open_artist(domain, &link.title, window, cx);
            return;
        }
        if let Some(dest) = link.destination() {
            match dest {
                MusicDestination::Playlist(pl) => self.open_playlist(*pl, window, cx),
                MusicDestination::CatalogUrl { url, title } => {
                    self.open_section(SectionTarget::Url { url, title }, window, cx);
                }
            }
        }
    }

    pub fn open_section(
        &mut self,
        target: SectionTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.load_section(target.clone(), false, cx);
        self.navigate_to(Route::Section(target), window, cx);
    }

    pub fn load_section(&mut self, target: SectionTarget, force: bool, cx: &mut Context<Self>) {
        self.core.load_section(target, force);
        cx.notify();
    }

    pub fn load_more_section(&mut self, target: SectionTarget, cx: &mut Context<Self>) {
        if self.router.current() != &Route::Section(target.clone()) {
            return;
        }
        self.core.load_more_section(target);
        cx.notify();
    }

    pub fn load_friends(&mut self, force: bool, cx: &mut Context<Self>) {
        self.core.load_friends(force);
        cx.notify();
    }

    fn friend_profile(&self, id: i64) -> Option<Friend> {
        self.core.friend_profile(id)
    }

    pub fn open_playlist_owner(
        &mut self,
        owner: Friend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.signed_in_user_id() == Some(owner.id) {
            self.navigate_to(Route::Library, window, cx);
        } else {
            self.open_friend(owner, window, cx);
        }
    }

    pub fn open_friend(&mut self, friend: Friend, window: &mut Window, cx: &mut Context<Self>) {
        let fid = friend.id;
        self.friend_playlists_expanded = false;
        if self.core.profile_search.take().is_some() {
            self.profile_search_input
                .update(cx, |s, cx| s.set_value("", window, cx));
        }
        self.load_friend_music(friend, false, cx);
        self.navigate_to(Route::FriendMusic(fid), window, cx);
    }

    pub fn load_friend_music(&mut self, friend: Friend, force: bool, cx: &mut Context<Self>) {
        self.core.load_friend_music(friend, force);
        cx.notify();
    }

    pub fn load_more_friend_playlists(&mut self, fid: i64, cx: &mut Context<Self>) {
        self.core.load_more_friend_playlists(fid);
        cx.notify();
    }

    pub fn load_more_friend_tracks(&mut self, fid: i64, cx: &mut Context<Self>) {
        self.core.load_more_friend_tracks(fid);
        cx.notify();
    }

    fn on_search_query_edited(
        &mut self,
        query: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.core.search_query = query.clone();
        if self.router.current() != &Route::Search {
            self.navigate_to(Route::Search, window, cx);
        }
        self.core.edit_search_query(query);
        cx.notify();
    }

    pub fn commit_search_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let q = self.top_search_input.read(cx).value().to_string();
        self.core.search_query = q.clone();
        if self.router.current() != &Route::Search {
            self.navigate_to(Route::Search, window, cx);
        }
        self.core.commit_search(q);
        cx.notify();
    }

    pub fn clear_search_box(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.core.clear_search();
        self.top_search_input.update(cx, |s, cx| {
            s.set_value("", window, cx);
            s.focus(window, cx);
        });
        cx.notify();
    }

    pub fn pick_search_history(
        &mut self,
        entry: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.top_search_input
            .update(cx, |s, cx| s.set_value(entry.clone(), window, cx));
        self.core.pick_search(entry);
        cx.notify();
    }

    pub fn delete_search_history(&mut self, entry: &str, cx: &mut Context<Self>) {
        self.core.delete_search_history(entry);
        cx.notify();
    }

    pub fn clear_all_search_history(&mut self, cx: &mut Context<Self>) {
        self.core.clear_search_history();
        cx.notify();
    }

    fn run_search_request(&mut self, query: String, settle_history: bool, cx: &mut Context<Self>) {
        self.core.run_search_request(query, settle_history);
        cx.notify();
    }

    pub fn is_liked(&self, track: &Audio) -> bool {
        self.core.is_liked(track)
    }

    pub fn is_downloaded(&self, track: &Audio) -> bool {
        self.core.is_downloaded(track)
    }

    pub fn canvas_near_bottom(&self) -> bool {
        let viewport = self.canvas_scroll.bounds().size.height;
        if viewport <= gpui_kit::px(0.0) {
            return false;
        }
        let remaining = self.canvas_scroll.max_offset().y + self.canvas_scroll.offset().y;
        remaining < viewport * 1.5
    }

    pub fn filtered_library_tracks(&self) -> Vec<Audio> {
        vively_app::library::filter_tracks(&self.core.library_tracks, &self.library_query)
    }

    pub fn is_download_pending(&self, track: &Audio) -> bool {
        self.core.is_download_pending(track)
    }

    pub fn is_collection_downloaded(&self, playlist: &Playlist) -> bool {
        self.core.is_collection_downloaded(playlist)
    }

    pub fn toggle_like_track(&mut self, track: Audio, cx: &mut Context<Self>) {
        self.core.toggle_like_track(track);
        cx.notify();
    }

    pub fn restore_own_removed_track(&mut self, cx: &mut Context<Self>) {
        self.core.restore_own_removed_track();
        cx.notify();
    }

    pub fn play_tracks(
        &mut self,
        tracks: Vec<Audio>,
        start_index: usize,
        origin: PlaybackOrigin,
        from_label: impl Into<String>,
        mix_session: Option<StreamMixSession>,
        cx: &mut Context<Self>,
    ) {
        self.core
            .play_tracks(tracks, start_index, origin, from_label, mix_session);
        cx.notify();
    }

    pub fn play_next_track(&mut self, track: Audio, cx: &mut Context<Self>) {
        self.core.play_next_track(track);
        cx.notify();
    }

    pub fn start_vk_mix(
        &mut self,
        mix_id: String,
        entity_id: Option<String>,
        options_json: Option<String>,
        from_label: String,
        cx: &mut Context<Self>,
    ) {
        self.core
            .start_vk_mix(mix_id, entity_id, options_json, from_label);
        cx.notify();
    }

    pub fn toggle_or_start_home_mix(&mut self, cx: &mut Context<Self>) {
        self.core.toggle_or_start_home_mix();
        cx.notify();
    }

    pub fn toggle_mix_option(
        &mut self,
        category_id: &str,
        option_id: &str,
        cx: &mut Context<Self>,
    ) {
        self.core.toggle_mix_option(category_id, option_id);
        cx.notify();
    }

    pub fn clear_mix_picks(&mut self, cx: &mut Context<Self>) {
        self.core.clear_mix_picks();
        cx.notify();
    }

    pub fn start_track_mix(&mut self, seed: Audio, cx: &mut Context<Self>) {
        self.core.start_track_mix(seed);
        cx.notify();
    }

    pub fn toggle_play_pause(&mut self, cx: &mut Context<Self>) {
        self.core.toggle_play_pause();
        cx.notify();
    }

    pub fn next_track(&mut self, cx: &mut Context<Self>) {
        self.core.next_track();
        cx.notify();
    }

    pub fn prev_track(&mut self, cx: &mut Context<Self>) {
        self.core.prev_track();
        cx.notify();
    }

    pub fn seek_to(&mut self, secs: f64, cx: &mut Context<Self>) {
        self.core.seek_to(secs);
        self.resume_lyrics_auto_scroll();
        cx.notify();
    }

    pub fn seek_relative(&mut self, delta_secs: f64, cx: &mut Context<Self>) {
        self.core.seek_relative(delta_secs);
        self.resume_lyrics_auto_scroll();
        cx.notify();
    }

    pub fn playback_position(&self) -> f64 {
        self.core.playback_position()
    }

    pub fn resume_lyrics_auto_scroll(&self) {
        self.panel_lyrics_scroll.borrow_mut().resume_auto_scroll();
        self.stage_lyrics_scroll.borrow_mut().resume_auto_scroll();
    }

    pub fn reset_lyrics_scroll(&self) {
        self.right_scroll.set_offset(point(px(0.0), px(0.0)));
        self.stage_scroll.set_offset(point(px(0.0), px(0.0)));
        self.panel_lyrics_scroll.borrow_mut().reset();
        self.stage_lyrics_scroll.borrow_mut().reset();
    }

    pub fn sync_lyrics_scroll(&self, vw: f32, window: &Window, cx: &App) {
        let synced = self
            .core.session
            .is_some()
            .then(|| {
                let track = self.core.queue.current()?;
                let sid = track.stable_id();
                let LyricsLoadState::Loaded(Some(lyr)) = self.core.lyrics_states.get(&sid)? else {
                    return None;
                };
                let LyricsContent::Synced(lines) = &lyr.content else {
                    return None;
                };
                let elapsed_ms = (self.playback_position() * 1000.0).round() as i64;
                Some((sid, active_synced_lyric_index(lines, elapsed_ms)))
            })
            .flatten();

        let panel_visible = !self.router.stage_open
            && self.router.panel_open
            && vw > 1180.0
            && self.router.panel_tab == PanelTab::Lyrics;
        let stage_visible = self.router.stage_open
            && self.router.stage_view == Some(PanelTab::Lyrics)
            && vw >= 860.0;

        let is_playing = self.core.audio_engine.is_playing();
        let reduce_motion = cx.reduce_motion();
        let now = cx.background_executor().now();
        let mut needs_frame = false;

        if panel_visible && let Some((ref sid, active_line)) = synced {
            needs_frame |= self.panel_lyrics_scroll.borrow_mut().prepare_render(
                &self.right_scroll,
                sid,
                active_line,
                is_playing,
                reduce_motion,
                now,
            );
        } else {
            self.panel_lyrics_scroll.borrow_mut().mark_hidden();
        }

        if stage_visible && let Some((ref sid, active_line)) = synced {
            needs_frame |= self.stage_lyrics_scroll.borrow_mut().prepare_render(
                &self.stage_scroll,
                sid,
                active_line,
                is_playing,
                reduce_motion,
                now,
            );
        } else {
            self.stage_lyrics_scroll.borrow_mut().mark_hidden();
        }

        if needs_frame {
            window.request_animation_frame();
        }
    }

    pub fn set_volume(&mut self, volume: f32, cx: &mut Context<Self>) {
        self.core.set_volume(volume);
        cx.notify();
    }

    pub fn toggle_mute(&mut self, cx: &mut Context<Self>) {
        self.core.toggle_mute();
        cx.notify();
    }

    pub fn toggle_shuffle(&mut self, cx: &mut Context<Self>) {
        self.core.toggle_shuffle();
        cx.notify();
    }

    pub fn cycle_repeat(&mut self, cx: &mut Context<Self>) {
        self.core.cycle_repeat();
        cx.notify();
    }

    pub fn select_queue_track(&mut self, track: Audio, cx: &mut Context<Self>) {
        self.core.select_queue_track(track);
        cx.notify();
    }

    pub fn remove_upcoming_at(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.core.remove_upcoming_at(offset);
        cx.notify();
    }

    pub fn move_upcoming_item(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        self.core.move_upcoming_item(from, to);
        cx.notify();
    }

    pub fn clear_upcoming_queue(&mut self, cx: &mut Context<Self>) {
        self.core.clear_upcoming_queue();
        cx.notify();
    }

    pub fn save_queue_as_playlist(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tracks = self.core.queue_tracks_for_export();
        self.router.stage_open = false;
        self.open_playlist_editor_with_tracks(None, tracks, window, cx);
    }

    fn poll_channels(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let mut handled = false;
        while let Ok(cmd) = self.media_cmd_rx.try_recv() {
            self.core.handle_media_command(cmd);
            handled = true;
        }
        if handled {
            cx.notify();
        }
    }

    fn on_periodic_tick(&mut self, cx: &mut Context<Self>) {
        let now_inst = Instant::now();
        let before_toasts = self.toasts.len();
        self.toasts.retain(|t| t.expires_at > now_inst);
        let mut dirty = self.toasts.len() != before_toasts;

        if self.artwork.take_dirty() {
            self.refresh_active_track_tint();
            dirty = true;
        }

        self.core.tick();

        let core = &self.core;
        let cur_track = core.queue.current();
        let dur = cur_track.map(|t| t.duration as f64).unwrap_or(0.0);
        self.media_controls.publish(
            cur_track,
            core.audio_engine.is_playing(),
            core.current_elapsed,
            dur,
            core.audio_engine.effective_volume(),
            core.queue.repeat_mode,
            core.queue.is_shuffled(),
            core.queue.next_index().is_some(),
            !core.queue.tracks().is_empty(),
        );

        if dirty {
            cx.notify();
        }
    }

    pub fn ensure_lyrics_for_current(&mut self, cx: &mut Context<Self>) {
        self.core.ensure_lyrics_for_current();
        cx.notify();
    }

    pub fn sync_downloads_snapshot(&mut self) {
        self.core.sync_downloads_snapshot();
    }

    pub fn download_single_track(&mut self, track: Audio, cx: &mut Context<Self>) {
        self.core.download_single_track(track);
        cx.notify();
    }

    pub fn remove_downloaded_track(&mut self, full_id: &str, cx: &mut Context<Self>) {
        self.core.remove_downloaded_track(full_id);
        cx.notify();
    }

    pub fn cancel_all_pending_downloads(&mut self, cx: &mut Context<Self>) {
        self.core.cancel_all_pending_downloads();
        cx.notify();
    }

    pub fn toggle_download_collection(
        &mut self,
        playlist: Playlist,
        tracks: Vec<Audio>,
        cx: &mut Context<Self>,
    ) {
        self.core.toggle_download_collection(playlist, tracks);
        cx.notify();
    }

    pub fn bulk_download_library(&mut self, max_tracks: Option<usize>, cx: &mut Context<Self>) {
        if self.core.offline_store.is_none() || self.access_token().is_none() {
            return;
        }
        self.router.modal = None;
        self.core.bulk_download_library(max_tracks);
        cx.notify();
    }

    pub fn clear_auto_cache_only(&mut self, cx: &mut Context<Self>) {
        self.core.clear_auto_cache();
        cx.notify();
    }

    pub fn clear_all_downloads_action(&mut self, cx: &mut Context<Self>) {
        self.core.clear_all_downloads();
        cx.notify();
    }

    pub fn trigger_downloads_drain(&mut self, cx: &mut Context<Self>) {
        self.core.trigger_downloads_drain();
        cx.notify();
    }

    pub fn toggle_follow_playlist(&mut self, playlist: Playlist, cx: &mut Context<Self>) {
        self.core.toggle_follow_playlist(playlist);
        cx.notify();
    }

    pub fn request_delete_playlist(&mut self, playlist: Playlist, cx: &mut Context<Self>) {
        self.router.modal = Some(ActiveModal::ConfirmDeletePlaylist { playlist });
        cx.notify();
    }

    pub fn delete_own_playlist(
        &mut self,
        playlist: Playlist,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(
            self.router.modal,
            Some(ActiveModal::ConfirmDeletePlaylist { .. })
        ) {
            self.router.modal = None;
        }
        self.core.delete_own_playlist(playlist);
        cx.notify();
    }

    pub fn open_add_to_playlist_modal(
        &mut self,
        track: Audio,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.add_pl_query.clear();
        self.core.add_pl_added.clear();
        self.core.add_pl_busy.clear();
        self.add_pl_scroll = ScrollHandle::new();
        self.add_pl_filter_input.update(cx, |s, cx| {
            s.set_value("", window, cx);
            s.focus(window, cx);
        });
        self.router.modal = Some(ActiveModal::AddToPlaylist { track });
        self.load_playlists(false, cx);
        cx.notify();
    }

    pub fn clear_add_pl_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.add_pl_query.is_empty() {
            self.add_pl_query.clear();
            self.add_pl_filter_input
                .update(cx, |s, cx| s.set_value("", window, cx));
            cx.notify();
        }
    }

    pub fn add_track_to_playlist(
        &mut self,
        track: Audio,
        playlist: Playlist,
        cx: &mut Context<Self>,
    ) {
        self.core.add_track_to_playlist(track, playlist);
        cx.notify();
    }

    pub fn remove_track_from_playlist(
        &mut self,
        track: Audio,
        playlist: Playlist,
        cx: &mut Context<Self>,
    ) {
        self.core.remove_track_from_playlist(track, playlist);
        cx.notify();
    }

    pub fn is_disliked(&self, track: &Audio) -> bool {
        self.core.is_disliked(track)
    }

    pub fn toggle_dislike_track(&mut self, track: Audio, cx: &mut Context<Self>) {
        self.core.toggle_dislike_track(track);
        cx.notify();
    }

    pub fn move_track_in_playlist(
        &mut self,
        track: Audio,
        playlist: Playlist,
        up: bool,
        cx: &mut Context<Self>,
    ) {
        self.core.move_track_in_playlist(track, playlist, up);
        cx.notify();
    }

    pub fn save_playlist_copy(&mut self, playlist: Playlist, cx: &mut Context<Self>) {
        self.core.save_playlist_copy(playlist);
        cx.notify();
    }

    pub fn toggle_artist_follow(&mut self, artist_id: String, cx: &mut Context<Self>) {
        self.core.toggle_artist_follow(artist_id);
        cx.notify();
    }

    pub fn toggle_owner_follow(&mut self, owner_id: i64, cx: &mut Context<Self>) {
        self.core.toggle_owner_follow(owner_id);
        cx.notify();
    }

    pub fn search_in_profile(&mut self, owner_id: i64, query: String, cx: &mut Context<Self>) {
        self.core.search_in_profile(owner_id, query);
        cx.notify();
    }

    pub fn open_playlist_editor(
        &mut self,
        editing: Option<Playlist>,
        seed_track: Option<Audio>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_playlist_editor_with_tracks(
            editing,
            seed_track.into_iter().collect(),
            window,
            cx,
        );
    }

    pub fn open_playlist_editor_with_tracks(
        &mut self,
        editing: Option<Playlist>,
        seed_tracks: Vec<Audio>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let is_new = editing.is_none();
        let (title, desc, hidden) = match &editing {
            Some(pl) => (
                pl.title.clone(),
                pl.description.clone().unwrap_or_default(),
                pl.no_discover.unwrap_or(false),
            ),
            None => (String::new(), String::new(), false),
        };
        self.core.begin_playlist_editor(seed_tracks, hidden);
        self.editor_picker_open = false;
        self.editor_picker_scroll = UniformListScrollHandle::new();
        self.editor_draft_scroll = ScrollHandle::new();
        self.editor_picker_query.clear();
        self.editor_source = EditorTrackSource::Likes;
        self.pl_title_input.update(cx, |s, cx| {
            s.set_value(title, window, cx);
            s.focus(window, cx);
        });
        self.pl_desc_input
            .update(cx, |s, cx| s.set_value(desc, window, cx));
        self.pl_picker_search_input.update(cx, |s, cx| {
            s.set_value("", window, cx);
            s.set_placeholder("Фильтр по моей музыке…", window, cx);
        });
        self.router.modal = Some(ActiveModal::PlaylistEditor { editing });
        if is_new {
            self.load_library(false, cx);
        }
        cx.notify();
    }

    pub fn open_add_tracks_to_playlist(
        &mut self,
        playlist: Playlist,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.core.begin_add_tracks();
        self.editor_picker_open = true;
        self.editor_picker_scroll = UniformListScrollHandle::new();
        self.editor_picker_query.clear();
        self.editor_source = EditorTrackSource::Likes;
        self.pl_picker_search_input.update(cx, |s, cx| {
            s.set_value("", window, cx);
            s.set_placeholder("Фильтр по моей музыке…", window, cx);
            s.focus(window, cx);
        });
        self.router.modal = Some(ActiveModal::AddTracksToPlaylist { playlist });
        self.load_library(false, cx);
        cx.notify();
    }

    pub fn submit_add_tracks_to_playlist(&mut self, cx: &mut Context<Self>) {
        let playlist = match &self.router.modal {
            Some(ActiveModal::AddTracksToPlaylist { playlist }) => playlist.clone(),
            _ => return,
        };
        self.core.submit_add_tracks_to_playlist(playlist);
        cx.notify();
    }

    pub fn modal_busy(&self) -> bool {
        match self.router.modal {
            Some(ActiveModal::PlaylistEditor { .. } | ActiveModal::AddTracksToPlaylist { .. }) => {
                self.core.editor_saving
            }
            Some(ActiveModal::AudioUpload) => self.core.upload_in_flight,
            _ => false,
        }
    }

    pub fn dismiss_modal(&mut self, cx: &mut Context<Self>) {
        if self.modal_busy() {
            return;
        }
        if matches!(
            self.router.modal,
            Some(ActiveModal::PlaylistEditor { .. } | ActiveModal::AddTracksToPlaylist { .. })
        ) {
            self.editor_picker_open = false;
            self.editor_picker_query.clear();
            self.core.editor_search_gen += 1;
            self.core.editor_picker_generation += 1;
        }
        if matches!(self.router.modal, Some(ActiveModal::AddToPlaylist { .. })) {
            self.add_pl_query.clear();
            self.core.add_pl_added.clear();
            self.core.add_pl_busy.clear();
        }
        if matches!(self.router.modal, Some(ActiveModal::Auth)) {
            self.core.reset_auth();
        }
        self.router.modal = None;
        cx.notify();
    }

    fn reset_editor_picker(&mut self, playlist: Option<Playlist>) {
        self.core.reset_editor_picker(playlist);
        self.editor_picker_scroll = UniformListScrollHandle::new();
    }

    fn sync_editor_picker_placeholder(&self, window: &mut Window, cx: &mut Context<Self>) {
        let placeholder = match self.editor_source {
            EditorTrackSource::Likes => "Фильтр по моей музыке…",
            EditorTrackSource::Playlists if self.core.editor_picker_playlist.is_some() => {
                "Фильтр по трекам плейлиста…"
            }
            EditorTrackSource::Playlists => "Фильтр по плейлистам…",
            EditorTrackSource::Search => "Поиск треков во ВКонтакте…",
        };
        self.pl_picker_search_input
            .update(cx, |s, cx| s.set_placeholder(placeholder, window, cx));
    }

    pub fn editor_picker_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.core.editor_picker_playlist.is_some() {
            self.reset_editor_picker(None);
            self.editor_picker_query.clear();
            self.pl_picker_search_input
                .update(cx, |s, cx| s.set_value("", window, cx));
            self.sync_editor_picker_placeholder(window, cx);
            self.load_editor_playlist_page(cx);
        } else {
            self.editor_picker_open = false;
        }
        cx.notify();
    }

    pub fn set_editor_source(
        &mut self,
        source: EditorTrackSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let was_drilled = self.core.editor_picker_playlist.is_some();
        self.editor_source = source;
        self.reset_editor_picker(None);
        if was_drilled {
            self.editor_picker_query.clear();
            self.pl_picker_search_input
                .update(cx, |s, cx| s.set_value("", window, cx));
        }
        self.sync_editor_picker_placeholder(window, cx);
        match source {
            EditorTrackSource::Likes => self.load_library(false, cx),
            EditorTrackSource::Playlists => self.load_editor_playlist_page(cx),
            EditorTrackSource::Search => {
                let q = self.editor_picker_query.clone();
                if !q.trim().is_empty() {
                    self.search_editor_tracks(q, true, cx);
                }
            }
        }
        cx.notify();
    }

    pub fn pick_editor_playlist(
        &mut self,
        playlist: Playlist,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reset_editor_picker(Some(playlist));
        self.editor_picker_query.clear();
        self.pl_picker_search_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.sync_editor_picker_placeholder(window, cx);
        self.load_editor_playlist_page(cx);
    }

    pub fn clear_editor_picker_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor_picker_query.clear();
        self.editor_picker_scroll = UniformListScrollHandle::new();
        self.pl_picker_search_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        if self.editor_source == EditorTrackSource::Search {
            self.core.editor_search_gen += 1;
            self.core.editor_search_tracks.clear();
            self.core.editor_search_state = LoadState::Idle;
        }
        cx.notify();
    }

    pub fn search_editor_query_in_vk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.editor_picker_query.clone();
        self.editor_source = EditorTrackSource::Search;
        self.reset_editor_picker(None);
        self.sync_editor_picker_placeholder(window, cx);
        self.pl_picker_search_input
            .update(cx, |s, cx| s.focus(window, cx));
        self.search_editor_tracks(query, true, cx);
        cx.notify();
    }

    pub fn on_editor_picker_query_change(
        &mut self,
        query: String,
        immediate: bool,
        cx: &mut Context<Self>,
    ) {
        self.editor_picker_query = query.clone();
        self.editor_picker_scroll = UniformListScrollHandle::new();
        if self.editor_source == EditorTrackSource::Search {
            self.search_editor_tracks(query, immediate, cx);
        } else {
            cx.notify();
        }
    }

    pub fn load_editor_playlist_page(&mut self, cx: &mut Context<Self>) {
        if !matches!(
            self.router.modal,
            Some(
                ActiveModal::PlaylistEditor { editing: None }
                    | ActiveModal::AddTracksToPlaylist { .. }
            )
        ) || self.editor_source != EditorTrackSource::Playlists
        {
            return;
        }
        self.core.load_editor_picker_page();
        cx.notify();
    }

    pub fn toggle_editor_draft_track(&mut self, track: Audio, cx: &mut Context<Self>) {
        self.core.toggle_editor_draft_track(track);
        cx.notify();
    }

    pub fn move_editor_draft_track(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        self.core.move_editor_draft_track(from, to);
        cx.notify();
    }

    pub fn clear_editor_draft_tracks(&mut self, cx: &mut Context<Self>) {
        self.core.clear_editor_draft_tracks();
        cx.notify();
    }

    pub fn add_editor_tracks_bulk(&mut self, tracks: Vec<Audio>, cx: &mut Context<Self>) {
        self.core.add_editor_tracks_bulk(tracks);
        cx.notify();
    }

    pub fn remove_editor_tracks_bulk(&mut self, tracks: &[Audio], cx: &mut Context<Self>) {
        self.core.remove_editor_tracks_bulk(tracks);
        cx.notify();
    }

    pub fn clear_editor_cover(&mut self, cx: &mut Context<Self>) {
        self.core.clear_editor_cover();
        cx.notify();
    }

    pub fn search_editor_tracks(&mut self, query: String, immediate: bool, cx: &mut Context<Self>) {
        self.editor_picker_scroll = UniformListScrollHandle::new();
        self.core.search_editor_tracks(query, immediate);
        cx.notify();
    }

    pub fn pick_playlist_cover(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let picked = rfd::AsyncFileDialog::new()
                .add_filter("Изображения", &["jpg", "jpeg", "png", "webp"])
                .pick_file()
                .await;
            let Some(handle) = picked else {
                return;
            };
            let path = handle.path().to_path_buf();
            let Ok(raw) = fs::read(&path) else {
                return;
            };
            let Ok(jpeg) = encode_cover_jpeg(&raw) else {
                let _ = this.update(cx, |this, cx| {
                    this.push_toast("Не удалось обработать обложку", cx);
                });
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.core.set_editor_cover(jpeg, path);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn save_playlist_editor(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let editing = match &self.router.modal {
            Some(ActiveModal::PlaylistEditor { editing }) => editing.clone(),
            _ => return,
        };
        let title = self.pl_title_input.read(cx).value().to_string();
        let desc = self.pl_desc_input.read(cx).value().to_string();
        self.core.save_playlist_editor(editing, &title, &desc);
        cx.notify();
    }

    pub fn pick_audio_files_for_upload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            let files = rfd::AsyncFileDialog::new()
                .add_filter("MP3 аудио", &["mp3"])
                .pick_files()
                .await;
            let Some(files) = files.filter(|f| !f.is_empty()) else {
                return;
            };
            let paths: Vec<PathBuf> = files.iter().map(|h| h.path().to_path_buf()).collect();
            let _ = this.update_in(cx, |this, window, cx| {
                this.core.set_upload_files(paths);
                let inputs: Vec<UploadInputs> = this
                    .core
                    .upload_items
                    .iter()
                    .map(|item| UploadInputs {
                        artist: cx.new(|cx| {
                            InputState::new(window, cx)
                                .placeholder("Исполнитель")
                                .default_value(item.artist.clone())
                        }),
                        title: cx.new(|cx| {
                            InputState::new(window, cx)
                                .placeholder("Название")
                                .default_value(item.title.clone())
                        }),
                    })
                    .collect();
                this.upload_inputs = inputs;
                this.router.modal = Some(ActiveModal::AudioUpload);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn confirm_audio_upload(&mut self, cx: &mut Context<Self>) {
        if self.core.upload_in_flight {
            return;
        }
        for (item, inputs) in self.core.upload_items.iter_mut().zip(&self.upload_inputs) {
            item.artist = inputs.artist.read(cx).value().to_string();
            item.title = inputs.title.read(cx).value().to_string();
        }
        self.core.confirm_audio_upload();
        cx.notify();
    }

    pub fn export_track_mp3(&mut self, track: Audio, cx: &mut Context<Self>) {
        self.router.modal = Some(ActiveModal::ShareFile {
            track: track.clone(),
            state: ExportState::Exporting,
        });
        self.core.export_track_mp3(track);
        cx.notify();
    }

    pub fn save_exported_mp3_as(&mut self, source_path: PathBuf, cx: &mut Context<Self>) {
        let default_name = source_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "track.mp3".into());
        cx.spawn(async move |this, cx| {
            let dest = rfd::AsyncFileDialog::new()
                .set_file_name(&default_name)
                .add_filter("MP3", &["mp3"])
                .save_file()
                .await;
            if let Some(dest) = dest {
                let out_res = fs::copy(&source_path, dest.path());
                let _ = this.update(cx, |this, cx| match out_res {
                    Ok(_) => this.push_toast("MP3 сохранён", cx),
                    Err(err) => this.push_toast(format!("Не удалось сохранить: {err}"), cx),
                });
            }
        })
        .detach();
    }

    pub fn reveal_exported_mp3(&mut self, path: &Path, cx: &mut Context<Self>) {
        reveal_in_file_manager(path);
        self.push_toast("Открыто в файловом менеджере", cx);
    }

    pub fn toggle_theme_quick(&mut self, window: &Window, cx: &mut Context<Self>) {
        let currently_dark = resolve_is_dark(self.core.settings.appearance, Some(window.appearance()));
        self.core.set_appearance(if currently_dark {
            Appearance::Light
        } else {
            Appearance::Dark
        });
        cx.notify();
    }

    pub fn set_appearance(&mut self, appearance: Appearance, cx: &mut Context<Self>) {
        self.core.set_appearance(appearance);
        cx.notify();
    }

    pub fn set_crossfade_secs(&mut self, secs: f64, cx: &mut Context<Self>) {
        self.core.set_crossfade_secs(secs);
        cx.notify();
    }

    pub fn toggle_crossfade_default(&mut self, cx: &mut Context<Self>) {
        self.core.toggle_crossfade_default();
        cx.notify();
    }

    pub fn toggle_genius_fallback(&mut self, cx: &mut Context<Self>) {
        self.core.toggle_genius_fallback();
        cx.notify();
    }

    pub fn toggle_auto_cache(&mut self, cx: &mut Context<Self>) {
        self.core.toggle_auto_cache();
        cx.notify();
    }

    pub fn toggle_wifi_only(&mut self, cx: &mut Context<Self>) {
        self.core.toggle_wifi_only();
        cx.notify();
    }

    pub fn set_cache_limit(&mut self, limit_bytes: u64, cx: &mut Context<Self>) {
        self.core.set_cache_limit(limit_bytes);
        cx.notify();
    }

    pub fn toggle_broadcast_setting(&mut self, cx: &mut Context<Self>) {
        self.core.toggle_broadcast_setting();
        cx.notify();
    }

    pub fn toggle_reporting_setting(&mut self, cx: &mut Context<Self>) {
        self.core.toggle_reporting_setting();
        cx.notify();
    }

    fn record_recent_playlist(&mut self, pl: &Playlist) {
        self.core.record_recent_playlist(pl);
    }

}

fn seed_legacy_macos_device_id(device_path: &Path) {
    if device_path.exists() {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = std::process::Command::new("defaults")
            .args(["read", "eu.r3pr3ss10n.r3vk", "device_id"])
            .output()
            && out.status.success()
        {
            let raw = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            if raw.len() >= 8 && raw.bytes().all(|b| b.is_ascii_alphanumeric()) {
                let profile = DeviceProfile { device_id: raw };
                if let Ok(bytes) = serde_json::to_vec(&profile) {
                    if let Some(parent) = device_path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::write(device_path, bytes);
                }
            }
        }
    }
}

fn qr_device_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "Vively (macOS)"
    } else if cfg!(target_os = "windows") {
        "Vively (Windows)"
    } else if cfg!(target_os = "linux") {
        "Vively (Linux)"
    } else {
        "Vively"
    }
}

struct DesktopHost {
    wake: futures_channel::mpsc::UnboundedSender<()>,
}

impl Host for DesktopHost {
    fn wake(&self) {
        let _ = self.wake.unbounded_send(());
    }

    fn network_state(&self) -> vively_core::downloads::NetworkState {
        probe_network_state()
    }
}
