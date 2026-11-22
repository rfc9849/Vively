use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::future::try_join_all;

use super::*;
use crate::{
    AudioService, Error, Result,
    api::Page,
    media::{Mp3StreamDownloader, Mp3Tag},
    models::{Audio, Playlist},
    playback::PlaybackOrigin,
    session::unix_now,
    settings::AppSettings,
};

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    State {
        full_id: String,
        state: DownloadState,
    },
    Progress {
        full_id: String,
        fraction: f64,
    },
    Failed {
        full_id: String,
        message: String,
    },
    Evicted {
        full_id: String,
    },
    Removed {
        full_id: String,
    },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DownloadSummary {
    pub ready: usize,
    pub failed: usize,
}

pub struct DownloadEngine {
    pub store: Arc<OfflineStore>,
    service: AudioService,
    downloader: Mp3StreamDownloader,
    run_lock: tokio::sync::Mutex<()>,
    running: Mutex<HashMap<String, Arc<tokio::sync::Notify>>>,
}

impl DownloadEngine {
    pub fn new(store: Arc<OfflineStore>, service: AudioService) -> Result<Self> {
        Ok(Self {
            store,
            service,
            downloader: Mp3StreamDownloader::new()?,
            run_lock: tokio::sync::Mutex::new(()),
            running: Mutex::new(HashMap::new()),
        })
    }

    pub fn remove(&self, full_id: &str) -> Result<()> {
        let running = self
            .running
            .lock()
            .map_err(|_| Error::InvalidState("Download lock is poisoned".into()))?;
        if let Some(cancel) = running.get(full_id) {
            cancel.notify_one();
        }
        self.store.remove(full_id)
    }

    pub fn clear_all(&self) -> Result<()> {
        let running = self
            .running
            .lock()
            .map_err(|_| Error::InvalidState("Download lock is poisoned".into()))?;
        for cancel in running.values() {
            cancel.notify_one();
        }
        self.store.clear_all()
    }

    pub async fn drain(
        &self,
        token: &str,
        network: NetworkState,
        settings: &AppSettings,
        protected: &HashSet<String>,
        events: &(dyn Fn(DownloadEvent) + Send + Sync),
    ) -> Result<DownloadSummary> {
        let _guard = self.run_lock.lock().await;
        self.store.recover_interrupted()?;
        let mut summary = DownloadSummary::default();
        if !network.is_online() {
            return Ok(summary);
        }
        let include_auto = !settings.downloads_wifi_only || network.allows_unmetered_transfer();
        loop {
            let mut jobs = Vec::new();
            for _ in 0..2 {
                if let Some(job) = self.store.claim_next(include_auto)? {
                    jobs.push(job);
                }
            }
            if jobs.is_empty() {
                break;
            }
            let results =
                try_join_all(jobs.into_iter().map(|job| self.run_job(job, token, events))).await?;
            for ready in results.into_iter().flatten() {
                if ready {
                    summary.ready += 1;
                } else {
                    summary.failed += 1;
                }
            }
            for id in self
                .store
                .enforce_limit(settings.cache_limit_bytes, protected)?
            {
                events(DownloadEvent::Evicted { full_id: id });
            }
        }
        Ok(summary)
    }

    async fn run_job(
        &self,
        job: CachedTrack,
        token: &str,
        events: &(dyn Fn(DownloadEvent) + Send + Sync),
    ) -> Result<Option<bool>> {
        let full_id = job.full_id.clone();
        let cancel = Arc::new(tokio::sync::Notify::new());
        self.running
            .lock()
            .map_err(|_| Error::InvalidState("Download lock is poisoned".into()))?
            .insert(full_id.clone(), cancel.clone());
        let _guard = RunningJob {
            running: &self.running,
            full_id: full_id.clone(),
        };
        if self.store.track(&full_id)?.is_none() {
            return Ok(None);
        }
        tokio::select! {
            biased;
            _ = cancel.notified() => {
                events(DownloadEvent::Removed { full_id });
                Ok(None)
            }
            result = self.perform_job(job, token, events) => result.map(Some),
        }
    }

    async fn perform_job(
        &self,
        job: CachedTrack,
        token: &str,
        events: &(dyn Fn(DownloadEvent) + Send + Sync),
    ) -> Result<bool> {
        events(DownloadEvent::State {
            full_id: job.full_id.clone(),
            state: DownloadState::Downloading,
        });
        let mut last_error = String::new();
        for attempt in 0..3u32 {
            let outcome = self.download_job(&job, token, attempt > 0, events).await;
            match outcome {
                Ok(()) => {
                    events(DownloadEvent::Progress {
                        full_id: job.full_id.clone(),
                        fraction: 1.0,
                    });
                    events(DownloadEvent::State {
                        full_id: job.full_id,
                        state: DownloadState::Ready,
                    });
                    return Ok(true);
                }
                Err(error) => {
                    last_error = error.to_string();
                    if attempt < 2 {
                        tokio::time::sleep(Duration::from_secs(if attempt == 0 { 2 } else { 8 }))
                            .await;
                    }
                }
            }
        }
        self.store.fail(&job.full_id, 3, &last_error)?;
        events(DownloadEvent::State {
            full_id: job.full_id.clone(),
            state: DownloadState::Failed,
        });
        events(DownloadEvent::Failed {
            full_id: job.full_id,
            message: last_error,
        });
        Ok(false)
    }

    async fn download_job(
        &self,
        job: &CachedTrack,
        token: &str,
        force: bool,
        events: &(dyn Fn(DownloadEvent) + Send + Sync),
    ) -> Result<()> {
        let track = self.live_track(job, token, force).await?;
        let url = track
            .stream_url()
            .ok_or_else(|| Error::Media("Track has no stream URL".into()))?;
        let artwork = match track.artwork_url() {
            Some(url) => match self.store.paths.artwork_data(url)? {
                Some(data) => Some(data),
                None => self.downloader.fetch_data(url.clone()).await.ok(),
            },
            None => None,
        };
        let artwork_file = if let (Some(url), Some(data)) = (track.artwork_url(), &artwork) {
            self.store
                .paths
                .store_artwork(url, data)?
                .file_name()
                .map(|v| v.to_string_lossy().into_owned())
        } else {
            None
        };
        let destination = self.store.paths.track_file(&job.full_id)?;
        let tag = Mp3Tag {
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.and_then(|a| a.title),
            artwork,
        };
        let on_progress = |fraction: f64| {
            events(DownloadEvent::Progress {
                full_id: job.full_id.clone(),
                fraction: fraction.clamp(0.0, 1.0),
            })
        };
        let bytes = self
            .downloader
            .download(url, &destination, &tag, &on_progress)
            .await?;
        if self.store.track(&job.full_id)?.is_none() {
            std::fs::remove_file(destination)?;
            return Err(Error::InvalidState("Download was removed".into()));
        }
        self.store
            .complete(&job.full_id, bytes, artwork_file.as_deref())?;
        Ok(())
    }

    async fn live_track(&self, job: &CachedTrack, token: &str, force: bool) -> Result<Audio> {
        let stored = job.audio()?;
        if !force && job.url_is_fresh(unix_now()) && stored.stream_url().is_some() {
            return Ok(stored);
        }
        let fresh = resolve_stream_url(&self.service, job, self.store.user_id, token)
            .await?
            .ok_or_else(|| Error::Media("Could not refresh the track URL".into()))?;
        self.store.update_live_track(&fresh, unix_now())?;
        Ok(fresh)
    }
}

struct RunningJob<'a> {
    running: &'a Mutex<HashMap<String, Arc<tokio::sync::Notify>>>,
    full_id: String,
}

impl Drop for RunningJob<'_> {
    fn drop(&mut self) {
        if let Ok(mut running) = self.running.lock() {
            running.remove(&self.full_id);
        }
    }
}

pub async fn resolve_stream_url(
    service: &AudioService,
    record: &CachedTrack,
    user_id: i64,
    token: &str,
) -> Result<Option<Audio>> {
    let id = record
        .access_key
        .as_ref()
        .map(|k| format!("{}_{k}", record.full_id))
        .unwrap_or_else(|| record.full_id.clone());
    if let Ok(tracks) = service.tracks_by_id(&[id], token).await
        && let Some(track) = tracks
            .into_iter()
            .find(|t| t.full_id() == record.full_id && t.stream_url().is_some())
    {
        return Ok(Some(track));
    }
    let page = Page {
        offset: record.source_offset.unwrap_or(0).saturating_sub(50),
        count: 100,
    };
    let tracks = match record.source_kind {
        Some(DownloadSourceKind::Library) => service.library(user_id, page, token).await?.items,
        Some(DownloadSourceKind::Playlist) => {
            let Some((owner, playlist)) =
                record.source_id.as_deref().and_then(|s| s.split_once('_'))
            else {
                return Ok(None);
            };
            let (Ok(owner), Ok(playlist)) = (owner.parse(), playlist.parse()) else {
                return Ok(None);
            };
            service
                .playlist_tracks_page(&Playlist::new(playlist, owner, ""), page, token)
                .await?
                .items
        }
        _ => return Ok(None),
    };
    Ok(tracks
        .into_iter()
        .find(|t| t.full_id() == record.full_id && t.stream_url().is_some()))
}

#[derive(Default)]
pub struct AutoCache {
    pending: Option<(Audio, i64)>,
}

impl AutoCache {
    pub fn track_started(
        &mut self,
        store: &OfflineStore,
        track: &Audio,
        origin: PlaybackOrigin,
        settings: &AppSettings,
        now: i64,
    ) -> Result<()> {
        store.mark_played(&track.full_id(), now)?;
        self.pending = if origin == PlaybackOrigin::Library
            && settings.auto_cache_enabled
            && track.is_playable()
        {
            Some((track.clone(), now.saturating_add(15)))
        } else {
            None
        };
        Ok(())
    }

    pub fn cancel(&mut self) {
        self.pending = None;
    }

    pub fn tick(
        &mut self,
        store: &OfflineStore,
        network: NetworkState,
        settings: &AppSettings,
        now: i64,
    ) -> Result<bool> {
        if self
            .pending
            .as_ref()
            .is_none_or(|(_, deadline)| now < *deadline)
        {
            return Ok(false);
        }
        let (track, _) = self
            .pending
            .take()
            .ok_or_else(|| Error::InvalidState("No auto-cache track is pending".into()))?;
        if !settings.auto_cache_enabled
            || !network.is_online()
            || (settings.downloads_wifi_only && !network.allows_unmetered_transfer())
            || store.track(&track.full_id())?.is_some()
        {
            return Ok(false);
        }
        store.enqueue(
            &track,
            &DownloadRequest {
                priority: DownloadPriority::Auto,
                manual_pin: false,
                source: Some(DownloadSourceKind::Track),
                source_id: None,
                source_offset: None,
            },
            now,
        )?;
        Ok(true)
    }
}
