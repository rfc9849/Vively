use std::{
    fs,
    future::Future,
    path::{Path, PathBuf},
    process::Command,
};

use directories::{BaseDirs, ProjectDirs};
use futures_channel::oneshot;
pub use vively_app::{
    captcha::{
        CaptchaBridge, CaptchaNavOutcome, CaptchaPrompt, captcha_interceptor_script,
        extract_captcha_success_token, parse_captcha_nav_outcome,
    },
    fmt::{
        fmt_duration, fmt_duration_f64, fmt_friends_count, fmt_gb, fmt_playlists_count,
        fmt_plays_count, fmt_total_duration, fmt_tracks_count,
    },
};
use vively_core::{
    downloads::{NetworkInterface, NetworkState, NetworkStatus},
    session::Session,
};

#[derive(Clone, Debug)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Self {
        if let Ok(custom) = std::env::var("VIVELY_HOME")
            && !custom.trim().is_empty()
        {
            let root = PathBuf::from(custom);
            return Self::from_root(&root);
        }
        if let Some(base) = BaseDirs::new() {
            let root = base.home_dir().join(".vively");
            migrate_legacy_dirs(&root);
            let _ = fs::create_dir_all(&root);
            return Self {
                config_dir: root.clone(),
                data_dir: root,
            };
        }
        let fallback = std::env::temp_dir().join("vively-app");
        Self::from_root(&fallback)
    }

    pub fn from_root(root: &Path) -> Self {
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let _ = fs::create_dir_all(&config_dir);
        let _ = fs::create_dir_all(&data_dir);
        Self {
            config_dir,
            data_dir,
        }
    }

    pub fn session_file(&self) -> PathBuf {
        self.config_dir.join("session.json")
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }

    pub fn device_file(&self) -> PathBuf {
        self.config_dir.join("device.json")
    }

    pub fn search_history_file(&self) -> PathBuf {
        self.config_dir.join("search_history.json")
    }

    pub fn recent_links_file(&self) -> PathBuf {
        self.config_dir.join("recent_links.json")
    }

    pub fn musixmatch_token_file(&self) -> PathBuf {
        self.config_dir.join("musixmatch_token.json")
    }

    pub fn queue_snapshot_file(&self) -> PathBuf {
        self.data_dir.join("queue.json")
    }

    pub fn queue_position_file(&self) -> PathBuf {
        self.data_dir.join("position.json")
    }

    pub fn offline_root(&self) -> PathBuf {
        self.data_dir.join("offline")
    }

    pub fn artwork_cache_dir(&self) -> PathBuf {
        self.data_dir.join("artwork_cache")
    }

    pub fn exports_dir(&self) -> PathBuf {
        let dir = self.data_dir.join("exports");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    pub fn load_dev_session(&self) -> Option<Session> {
        let mut candidates = Vec::new();
        if let Ok(env_path) = std::env::var("VIVELY_DEV_SESSION")
            && !env_path.trim().is_empty()
        {
            candidates.push(PathBuf::from(env_path));
        }
        if cfg!(debug_assertions) {
            candidates.push(self.config_dir.join("dev-session.json"));
            if let Some(user_dirs) = directories::UserDirs::new()
                && let Some(docs) = user_dirs.document_dir()
            {
                candidates.push(docs.join("dev-session.json"));
            }
        }
        for path in candidates {
            if let Ok(bytes) = fs::read(&path)
                && let Ok(session) = serde_json::from_slice::<Session>(&bytes)
            {
                return Some(session);
            }
        }
        None
    }
}

#[derive(Clone)]
pub struct TokioBridge(vively_app::TokioBridge);

impl TokioBridge {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self(vively_app::TokioBridge::new()?))
    }

    /// The shared runtime handed to `vively_app::AppCore`.
    pub fn shared(&self) -> vively_app::TokioBridge {
        self.0.clone()
    }

    pub fn handle(&self) -> tokio::runtime::Handle {
        self.0.handle()
    }

    pub fn spawn_bg<T, F>(
        &self,
        bg: &gpui_kit::BackgroundExecutor,
        fut: F,
    ) -> gpui_kit::Task<Result<T, oneshot::Canceled>>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        bg.spawn(self.0.spawn_oneshot(fut))
    }
}

fn migrate_legacy_dirs(root: &Path) {
    if root.exists() {
        return;
    }
    let Some(proj) = ProjectDirs::from("eu", "r3pr3ss10n", "Vively") else {
        return;
    };
    let mut sources = vec![proj.config_dir().to_path_buf()];
    if proj.data_dir() != proj.config_dir() {
        sources.push(proj.data_dir().to_path_buf());
    }
    if !sources.iter().any(|dir| dir.is_dir()) || fs::create_dir_all(root).is_err() {
        return;
    }
    for dir in sources {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let target = root.join(entry.file_name());
            if !target.exists() {
                let _ = fs::rename(entry.path(), target);
            }
        }
        let _ = fs::remove_dir(&dir);
    }
    let legacy_session = root.join("session_fallback.json");
    let session = root.join("session.json");
    if legacy_session.exists() && !session.exists() {
        let _ = fs::rename(legacy_session, session);
    }
}

pub fn probe_network_state() -> NetworkState {
    NetworkState {
        status: NetworkStatus::Online(NetworkInterface::Wifi),
        expensive: false,
        constrained: false,
    }
}

pub fn reveal_in_file_manager(path: &Path) {
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg("-R").arg(path).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(parent) = path.parent() {
            let _ = Command::new("xdg-open").arg(parent).spawn();
        }
    }
}
