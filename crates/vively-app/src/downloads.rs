use std::{collections::HashSet, path::Path, sync::Arc};

use vively_core::{
    Error,
    api::Page,
    downloads::{
        CacheUsage, DownloadEngine, DownloadEvent, DownloadPriority, DownloadRequest,
        DownloadSourceKind, DownloadState, OfflineStore,
    },
    models::{Audio, Playlist},
    session::unix_now,
};

use crate::app::{AppCore, Dirty};

fn manual_request(source: DownloadSourceKind, offset: Option<usize>) -> DownloadRequest {
    DownloadRequest {
        priority: DownloadPriority::Manual,
        manual_pin: true,
        source: Some(source),
        source_id: None,
        source_offset: offset,
    }
}

impl AppCore {
    /// Open the per-account offline store and download engine.
    pub fn attach_offline_store(&mut self, root: &Path, user_id: i64) {
        let Ok(store) = OfflineStore::open(root, user_id) else {
            return;
        };
        let _ = store.recover_interrupted();
        let store = Arc::new(store);
        if let Ok(engine) = DownloadEngine::new(Arc::clone(&store), self.service.clone()) {
            self.download_engine = Some(Arc::new(engine));
        }
        self.offline_store = Some(store);
        self.sync_downloads_snapshot();
    }

    pub fn sync_downloads_snapshot(&mut self) {
        let Some(store) = &self.offline_store else {
            return;
        };
        self.ready_downloads = store.ready_tracks().unwrap_or_default();
        self.pending_downloads = store.pending_tracks().unwrap_or_default();
        self.offline_collections = store.collections().unwrap_or_default();
        self.cache_usage = store.usage().unwrap_or(CacheUsage {
            total: 0,
            pinned: 0,
            track_count: 0,
        });
        self.mark(Dirty::DOWNLOADS);
    }

    pub fn is_downloaded(&self, track: &Audio) -> bool {
        self.ready_downloads
            .iter()
            .any(|t| t.id == track.id && t.owner_id == track.owner_id)
    }

    pub fn is_download_pending(&self, track: &Audio) -> bool {
        let fid = track.full_id();
        self.pending_downloads.iter().any(|t| t.full_id == fid)
    }

    pub fn is_collection_downloaded(&self, playlist: &Playlist) -> bool {
        let fid = playlist.full_id();
        self.offline_collections.iter().any(|c| c.full_id == fid)
    }

    fn remove_download_entry(&mut self, full_id: &str) {
        if let Some(engine) = &self.download_engine {
            let _ = engine.remove(full_id);
        } else if let Some(store) = &self.offline_store {
            let _ = store.remove(full_id);
        }
        self.download_progress.remove(full_id);
    }

    fn redrain(&mut self) {
        self.trigger_downloads_drain();
    }

    /// Full ids that must never be evicted (the playing track).
    pub fn protected_download_ids(&self) -> HashSet<String> {
        self.queue.current().map(|t| t.full_id()).into_iter().collect()
    }

    pub fn download_single_track(&mut self, track: Audio) {
        let Some(store) = self.offline_store.clone() else {
            return;
        };
        let req = manual_request(DownloadSourceKind::Track, None);
        if store.enqueue(&track, &req, unix_now()).is_ok() {
            self.sync_downloads_snapshot();
            self.toast("Трек добавлен в загрузку");
            self.redrain();
        }
    }

    pub fn remove_downloaded_track(&mut self, full_id: &str) {
        self.remove_download_entry(full_id);
        self.sync_downloads_snapshot();
        self.toast("Загрузка удалена");
    }

    pub fn cancel_all_pending_downloads(&mut self) {
        if self.pending_downloads.is_empty() {
            return;
        }
        let ids: Vec<String> = self
            .pending_downloads
            .iter()
            .map(|item| item.full_id.clone())
            .collect();
        for fid in &ids {
            self.remove_download_entry(fid);
        }
        self.sync_downloads_snapshot();
        self.toast("Очередь загрузки отменена");
    }

    pub fn toggle_download_collection(&mut self, playlist: Playlist, tracks: Vec<Audio>) {
        let Some(store) = self.offline_store.clone() else {
            return;
        };
        if self.is_collection_downloaded(&playlist) {
            let _ = store.remove_collection(&playlist.full_id());
            self.sync_downloads_snapshot();
            self.toast("Коллекция убрана из загрузок");
            return;
        }
        if store
            .save_collection(&playlist, &tracks, unix_now())
            .is_ok()
        {
            self.sync_downloads_snapshot();
            self.toast("Коллекция поставлена в очередь загрузки");
            self.redrain();
        }
    }

    pub fn bulk_download_library(&mut self, max_tracks: Option<usize>) {
        let (Some(store), Some(token), Some(uid)) = (
            self.offline_store.clone(),
            self.access_token(),
            self.signed_in_user_id(),
        ) else {
            return;
        };
        self.toast("Подготовка загрузки медиатеки...");

        let account = self.account_generation;
        let service = self.service.clone();
        let initial = self.library_tracks.clone();
        let target = max_tracks.unwrap_or(5000);
        self.spawn(
            async move {
                let mut collected = initial;
                let mut offset = collected.len();
                while collected.len() < target {
                    let page = service
                        .library(uid, Page { offset, count: 100 }, &token)
                        .await?;
                    if page.items.is_empty() {
                        break;
                    }
                    offset += page.items.len();
                    collected.extend(page.items);
                    if offset >= page.count as usize {
                        break;
                    }
                }
                collected.truncate(target);
                let now = unix_now();
                for (idx, tr) in collected.iter().enumerate() {
                    if tr.is_playable() {
                        let req = manual_request(DownloadSourceKind::Library, Some(idx));
                        let _ = store.enqueue(tr, &req, now);
                    }
                }
                Ok::<_, Error>(collected.len())
            },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                match res {
                    Ok(count) => {
                        core.sync_downloads_snapshot();
                        core.toast(format!("В очередь добавлено {count} треков"));
                        core.redrain();
                    }
                    Err(err) => core.toast(format!("Ошибка загрузки: {err}")),
                }
            },
        );
    }

    pub fn clear_auto_cache(&mut self) {
        let Some(store) = self.offline_store.clone() else {
            return;
        };
        let _ = store.clear_auto_cache(&self.protected_download_ids());
        self.sync_downloads_snapshot();
        self.toast("Автокэш очищен");
    }

    pub fn clear_all_downloads(&mut self) {
        if let Some(engine) = &self.download_engine {
            let _ = engine.clear_all();
        } else if let Some(store) = &self.offline_store {
            let _ = store.clear_all();
        }
        self.download_progress.clear();
        self.sync_downloads_snapshot();
        self.toast("Все загрузки удалены");
    }

    /// Drain queued downloads, never evicting the playing track.
    pub fn trigger_downloads_drain(&mut self) {
        if !self
            .pending_downloads
            .iter()
            .any(|job| job.state == DownloadState::Queued)
        {
            return;
        }
        if self.drain_in_flight {
            self.drain_requested = true;
            return;
        }
        let (Some(engine), Some(token)) = (self.download_engine.clone(), self.access_token())
        else {
            return;
        };
        self.drain_in_flight = true;
        let account = self.account_generation;
        let settings = self.settings.clone();
        let protected = self.protected_download_ids();
        let network = self.network_state();
        let spawner = self.spawner();
        self.spawn(
            async move {
                let _ = engine
                    .drain(&token, network, &settings, &protected, &|ev| {
                        spawner.post(move |core| core.on_download_event(account, ev));
                    })
                    .await;
            },
            move |core, ()| {
                if core.account_generation != account {
                    return;
                }
                core.drain_in_flight = false;
                core.sync_downloads_snapshot();
                if std::mem::take(&mut core.drain_requested) {
                    core.redrain();
                }
            },
        );
    }

    fn on_download_event(&mut self, account: u64, ev: DownloadEvent) {
        if self.account_generation != account {
            return;
        }
        match ev {
            DownloadEvent::Progress { full_id, fraction } => {
                self.download_progress.insert(full_id, fraction);
            }
            DownloadEvent::State { full_id, .. }
            | DownloadEvent::Failed { full_id, .. }
            | DownloadEvent::Evicted { full_id }
            | DownloadEvent::Removed { full_id } => {
                self.download_progress.remove(&full_id);
                self.sync_downloads_snapshot();
            }
        }
        self.mark(Dirty::DOWNLOADS);
    }
}
