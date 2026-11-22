//! Builds a ready-to-use [`AppCore`] from on-disk paths. Shared by every
//! front-end so platform code only supplies paths, a [`Host`] and options.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use futures_util::StreamExt;
use vively_audio::DualAudioEngine;
use vively_core::{
    AudioService, Result, VkClient,
    config::VkConfig,
    lyrics::LyricsProvider,
    media::Mp3StreamDownloader,
    session::{DeviceProfile, Session, SessionPersistence, SessionStore, SessionTokenRefresher},
    settings::{AppSettings, SearchHistory},
};

use crate::{
    app::{AppCore, CoreDeps, CoreEvent, Host},
    captcha::CaptchaBridge,
    runtime::TokioBridge,
};

/// Where the core keeps its files. `config_dir` holds small JSON state,
/// `data_dir` holds caches, the offline store and exports.
#[derive(Clone, Debug)]
pub struct CorePaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl CorePaths {
    pub fn new(config_dir: PathBuf, data_dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&config_dir);
        let _ = fs::create_dir_all(&data_dir);
        Self {
            config_dir,
            data_dir,
        }
    }

    pub fn from_root(root: &Path) -> Self {
        Self::new(root.join("config"), root.join("data"))
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
    pub fn exports_dir(&self) -> PathBuf {
        self.data_dir.join("exports")
    }
}

pub struct CoreConfig {
    pub paths: CorePaths,
    pub persistence: Arc<dyn SessionPersistence>,
    /// No output device (tests, headless runs).
    pub simulated_audio: bool,
    pub qr_device_name: String,
    pub client_secret: String,
    /// Adopted only if no session is stored yet.
    pub dev_session: Option<Session>,
    /// Override the VK API host (mock servers in tests).
    pub api_host: Option<url::Url>,
}

/// Build the core. Blocks briefly on the tokio runtime to install the token
/// refresher and captcha resolver, so call it from a non-runtime thread.
pub fn build_core(host: Arc<dyn Host>, tokio: TokioBridge, config: CoreConfig) -> Result<AppCore> {
    let CoreConfig {
        paths,
        persistence,
        simulated_audio,
        qr_device_name,
        client_secret,
        dev_session,
        api_host,
    } = config;

    let settings = AppSettings::load(&paths.settings_file()).unwrap_or_default();
    let device = match DeviceProfile::load_or_create(&paths.device_file()) {
        Ok(d) => d,
        Err(_) => {
            let _ = fs::remove_file(paths.device_file());
            DeviceProfile::load_or_create(&paths.device_file())?
        }
    };
    let mut vk_config = VkConfig::android(client_secret)?;
    if let Some(host) = api_host {
        vk_config.api_host = host;
    }
    let vk_client = VkClient::new(vk_config, device.device_id)?;

    let session_store = match SessionStore::new(Arc::clone(&persistence)) {
        Ok(store) => Arc::new(store),
        Err(_) => {
            let _ = persistence.delete();
            Arc::new(SessionStore::new(Arc::new(FileSessionPersistence {
                path: paths.session_file(),
            }))?)
        }
    };
    if session_store.session()?.is_none()
        && let Some(dev) = dev_session
    {
        let _ = session_store.adopt(dev);
    }
    let initial_session = session_store.session()?;

    let refresher = Arc::new(SessionTokenRefresher::new(
        &vk_client,
        Arc::clone(&session_store),
    ));
    let (captcha_bridge, mut captcha_rx) = CaptchaBridge::new();
    {
        let client = vk_client.clone();
        tokio.handle().block_on(async move {
            client.set_token_refresher(Some(refresher)).await;
            client.set_captcha_resolver(Some(captcha_bridge)).await;
        });
    }

    let service = AudioService::new(vk_client.clone());
    let lyrics_provider = Arc::new(tokio::sync::Mutex::new(
        LyricsProvider::new(service.clone())?
            .with_musixmatch_token_path(paths.musixmatch_token_file()),
    ));
    let mut audio_engine = if simulated_audio {
        DualAudioEngine::new_simulated()?
    } else {
        DualAudioEngine::new()?
    };
    audio_engine.set_volume(settings.volume);
    audio_engine.set_muted(settings.muted);

    let core = AppCore::new(
        host,
        tokio,
        CoreDeps {
            vk_client,
            service,
            session_store,
            initial_session,
            search_history: SearchHistory::load(&paths.search_history_file()).unwrap_or_default(),
            search_history_path: paths.search_history_file(),
            settings,
            settings_path: paths.settings_file(),
            data_dir: paths.data_dir.clone(),
            recent_links_path: paths.recent_links_file(),
            exports_dir: paths.exports_dir(),
            downloader: Mp3StreamDownloader::new()?,
            queue_snapshot_path: paths.queue_snapshot_file(),
            queue_position_path: paths.queue_position_file(),
            audio_engine,
            lyrics_provider,
            qr_device_name,
        },
    );

    let spawner = core.spawner();
    core.fire(async move {
        while let Some(prompt) = captcha_rx.next().await {
            spawner.post(move |core| core.emit(CoreEvent::CaptchaRequested(prompt)));
        }
    });

    Ok(core)
}

/// JSON session file, owner-only permissions on unix.
pub struct FileSessionPersistence {
    pub path: PathBuf,
}

impl SessionPersistence for FileSessionPersistence {
    fn load(&self) -> Result<Option<Session>> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| vively_core::Error::Decode {
                    context: "saved session".into(),
                    source: e,
                }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn save(&self, session: &Session) -> Result<()> {
        let bytes = serde_json::to_vec(session).map_err(|e| vively_core::Error::Decode {
            context: "session".into(),
            source: e,
        })?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        #[cfg(unix)]
        {
            use std::io::Write as _;
            use std::os::unix::fs::OpenOptionsExt as _;
            let mut f = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .mode(0o600)
                .open(&self.path)?;
            f.write_all(&bytes)?;
        }
        #[cfg(not(unix))]
        {
            fs::write(&self.path, bytes)?;
        }
        Ok(())
    }

    fn delete(&self) -> Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
