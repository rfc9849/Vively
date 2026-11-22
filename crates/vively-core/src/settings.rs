use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::{Error, Result, session::save_json};

pub const MAX_CROSSFADE: f64 = 12.0;
pub const DEFAULT_CROSSFADE: f64 = 6.0;
pub const UNLIMITED_CACHE: u64 = 0;
pub const DEFAULT_CACHE_LIMIT: u64 = 5 * 1024 * 1024 * 1024;
pub const CACHE_LIMIT_OPTIONS: [u64; 5] = [
    1024 * 1024 * 1024,
    2 * 1024 * 1024 * 1024,
    DEFAULT_CACHE_LIMIT,
    10 * 1024 * 1024 * 1024,
    UNLIMITED_CACHE,
];
pub const SEARCH_HISTORY_LIMIT: usize = 10;

fn default_volume() -> f32 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub crossfade_duration: f64,
    pub broadcast_enabled: bool,
    pub playback_reporting_enabled: bool,
    pub genius_fallback_enabled: bool,
    pub auto_cache_enabled: bool,
    pub downloads_wifi_only: bool,
    pub cache_limit_bytes: u64,
    pub appearance: Appearance,
    #[serde(default = "default_volume")]
    pub volume: f32,
    pub muted: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            crossfade_duration: 0.0,
            broadcast_enabled: false,
            playback_reporting_enabled: true,
            genius_fallback_enabled: true,
            auto_cache_enabled: false,
            downloads_wifi_only: true,
            cache_limit_bytes: DEFAULT_CACHE_LIMIT,
            appearance: Appearance::System,
            volume: 1.0,
            muted: false,
        }
    }
}

impl AppSettings {
    pub fn load(path: &Path) -> Result<Self> {
        let mut settings: Self = match fs::read(path) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|e| Error::decoding("settings", e))?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => return Err(e.into()),
        };
        settings.normalize();
        Ok(settings)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let mut settings = self.clone();
        settings.normalize();
        save_json(path, &settings)
    }

    pub fn normalize(&mut self) {
        self.crossfade_duration = if self.crossfade_duration.is_finite() {
            self.crossfade_duration.clamp(0.0, MAX_CROSSFADE)
        } else {
            0.0
        };
        self.volume = if self.volume.is_finite() {
            self.volume.clamp(0.0, 1.0)
        } else {
            1.0
        };
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchHistory {
    pub entries: Vec<String>,
}

impl SearchHistory {
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read(path) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|e| Error::decoding("search history", e))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        save_json(path, self)
    }

    pub fn record(&mut self, query: &str) -> bool {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return false;
        }
        let lower = trimmed.to_lowercase();
        self.entries.retain(|item| item.to_lowercase() != lower);
        self.entries.insert(0, trimmed.to_owned());
        self.entries.truncate(SEARCH_HISTORY_LIMIT);
        true
    }

    pub fn remove(&mut self, query: &str) {
        self.entries.retain(|item| item != query);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
