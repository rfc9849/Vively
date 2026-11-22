use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::PathBuf,
};

use crate::{
    AudioService, Error, Result,
    api::Page,
    models::{Audio, Playlist, Thumb},
};

const MORE_PAGE_SIZE: usize = 1000;

pub struct LibrarySnapshot {
    pub revision: u64,
    pub tracks: Vec<Audio>,
    pub total_count: i64,
    pub can_load_more: bool,
    pub foreign_copies: HashMap<(i64, i64), i64>,
}

pub struct MusicLibrary {
    service: AudioService,
    pub owner_id: i64,
    tracks: Vec<Audio>,
    total_count: i64,
    next_offset: usize,
    exhausted: bool,
    revision: u64,
    foreign_copies: HashMap<(i64, i64), i64>,
    copy_history_path: Option<PathBuf>,
}

impl MusicLibrary {
    pub fn new(service: AudioService, owner_id: i64) -> Self {
        Self {
            service,
            owner_id,
            tracks: Vec::new(),
            total_count: 0,
            next_offset: 0,
            exhausted: false,
            revision: 0,
            foreign_copies: HashMap::new(),
            copy_history_path: None,
        }
    }

    pub fn with_copy_history(service: AudioService, owner_id: i64, path: PathBuf) -> Self {
        let mut library = Self::new(service, owner_id);
        if let Ok(bytes) = fs::read(&path)
            && let Ok(entries) = serde_json::from_slice::<Vec<((i64, i64), i64)>>(&bytes)
        {
            library.foreign_copies = entries.into_iter().collect();
        }
        library.copy_history_path = Some(path);
        library
    }

    fn save_copy_history(&self) {
        if let Some(path) = &self.copy_history_path {
            let entries: Vec<_> = self
                .foreign_copies
                .iter()
                .map(|(&id, &copy)| (id, copy))
                .collect();
            let _ = crate::session::save_json(path, &entries);
        }
    }

    fn reconcile_copy_history(&mut self) {
        if self.foreign_copies.is_empty()
            || self.can_load_more()
            || self.tracks.len() as i64 != self.total_count
        {
            return;
        }
        let saved: HashSet<_> = self
            .tracks
            .iter()
            .filter(|t| t.owner_id == self.owner_id)
            .map(|t| t.id)
            .collect();
        let before = self.foreign_copies.len();
        self.foreign_copies.retain(|_, copy| saved.contains(copy));
        if before != self.foreign_copies.len() {
            self.save_copy_history();
        }
    }

    pub fn tracks(&self) -> &[Audio] {
        &self.tracks
    }

    pub fn reset(&mut self, owner_id: i64) {
        self.owner_id = owner_id;
        self.tracks.clear();
        self.total_count = 0;
        self.next_offset = 0;
        self.exhausted = false;
        self.revision += 1;
        self.foreign_copies.clear();
        self.copy_history_path = None;
    }

    pub fn snapshot(&self) -> LibrarySnapshot {
        LibrarySnapshot {
            revision: self.revision,
            tracks: self.tracks.clone(),
            total_count: self.total_count,
            can_load_more: self.can_load_more(),
            foreign_copies: self.foreign_copies.clone(),
        }
    }

    fn prepend_track(&mut self, track: Audio) {
        if !self
            .tracks
            .iter()
            .any(|t| (t.owner_id, t.id) == (track.owner_id, track.id))
        {
            self.tracks.insert(0, track);
            self.total_count += 1;
            if self.next_offset > 0 {
                self.next_offset += 1;
            }
            self.exhausted = false;
        }
        self.revision += 1;
    }

    pub fn prepend_uploaded(&mut self, track: Audio) {
        self.prepend_track(track);
    }

    pub fn total_count(&self) -> i64 {
        self.total_count
    }
    pub fn can_load_more(&self) -> bool {
        !self.exhausted && (self.next_offset as i64) < self.total_count
    }

    pub fn filtered_tracks(&self, query: &str) -> Vec<&Audio> {
        let query = query.trim().to_lowercase();
        self.tracks
            .iter()
            .filter(|t| {
                query.is_empty()
                    || t.title.to_lowercase().contains(&query)
                    || t.artist.to_lowercase().contains(&query)
            })
            .collect()
    }

    pub fn is_liked(&self, track: &Audio) -> bool {
        let id = (track.owner_id, track.id);
        self.foreign_copies.contains_key(&id)
            || self.tracks.iter().any(|t| (t.owner_id, t.id) == id)
    }

    pub async fn refresh(&mut self, token: &str) -> Result<()> {
        let page = self
            .service
            .library(self.owner_id, Page::default(), token)
            .await?;
        self.next_offset = 100;
        self.total_count = page.count;
        self.exhausted = page.items.is_empty();
        self.tracks = page.items;
        self.reconcile_copy_history();
        self.revision += 1;
        Ok(())
    }

    pub async fn load_more(&mut self, token: &str) -> Result<()> {
        if !self.can_load_more() {
            return Ok(());
        }
        let page = self
            .service
            .library(
                self.owner_id,
                Page {
                    offset: self.next_offset,
                    count: MORE_PAGE_SIZE,
                },
                token,
            )
            .await?;
        self.next_offset = self.next_offset.saturating_add(MORE_PAGE_SIZE);
        self.total_count = page.count;
        self.exhausted = page.items.is_empty();
        let mut known: HashSet<_> = self.tracks.iter().map(|a| (a.owner_id, a.id)).collect();
        self.tracks.extend(
            page.items
                .into_iter()
                .filter(|a| known.insert((a.owner_id, a.id))),
        );
        self.reconcile_copy_history();
        self.revision += 1;
        Ok(())
    }

    pub async fn toggle_like(&mut self, track: &Audio, token: &str) -> Result<bool> {
        let id = (track.owner_id, track.id);
        if self.is_liked(track) {
            let (owner_id, audio_id, access_key) = match self.foreign_copies.get(&id) {
                Some(&copy_id) => (self.owner_id, copy_id, None),
                None => (track.owner_id, track.id, track.access_key.as_deref()),
            };
            let result = self
                .service
                .remove_from_library(owner_id, audio_id, access_key, token)
                .await?;
            if result <= 0 {
                return Err(Error::InvalidState("VK did not remove the track".into()));
            }
            if owner_id == self.owner_id {
                let before = self.foreign_copies.len();
                self.foreign_copies
                    .retain(|_, copy_id| *copy_id != audio_id);
                if before != self.foreign_copies.len() {
                    self.save_copy_history();
                }
            }
            let len = self.tracks.len();
            self.tracks
                .retain(|t| (t.owner_id, t.id) != (owner_id, audio_id));
            if self.tracks.len() < len {
                self.next_offset = self.next_offset.saturating_sub(1);
            }
            self.total_count = self.total_count.saturating_sub(1).max(0);
            self.revision += 1;
            return Ok(false);
        }

        let saved = if track.owner_id == self.owner_id {
            if self.service.restore_to_library(track, token).await? <= 0 {
                return Err(Error::InvalidState("VK did not restore the track".into()));
            }
            track.clone()
        } else {
            let copy_id = self.service.add_to_library(track, token).await?;
            if copy_id <= 0 {
                return Err(Error::InvalidState(
                    "VK did not return a saved track ID".into(),
                ));
            }
            self.foreign_copies.insert(id, copy_id);
            self.save_copy_history();
            let mut copy = track.clone();
            copy.owner_id = self.owner_id;
            copy.id = copy_id;
            copy.access_key = None;
            copy
        };
        self.prepend_track(saved);
        Ok(true)
    }
}

#[derive(Debug, Clone)]
pub struct StreamMixSession {
    pub mix_id: String,
    pub entity_id: Option<String>,
    pub options_json: Option<String>,
}

impl StreamMixSession {
    pub fn tuned(
        mix_id: impl Into<String>,
        options: BTreeMap<String, Vec<String>>,
    ) -> Result<Self> {
        let options: BTreeMap<_, _> = options
            .into_iter()
            .filter(|(_, values)| !values.is_empty())
            .collect();
        let options_json = if options.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&options).map_err(|e| Error::decoding("mix settings", e))?)
        };
        Ok(Self {
            mix_id: mix_id.into(),
            entity_id: None,
            options_json,
        })
    }

    pub async fn initial(&self, service: &AudioService, token: &str) -> Result<Vec<Audio>> {
        self.fetch(service, token, 20).await
    }

    pub async fn next_batch(&self, service: &AudioService, token: &str) -> Result<Vec<Audio>> {
        self.fetch(service, token, 15).await
    }

    async fn fetch(&self, service: &AudioService, token: &str, count: usize) -> Result<Vec<Audio>> {
        service
            .stream_mix_audios(
                &self.mix_id,
                self.entity_id.as_deref(),
                count,
                self.options_json.as_deref(),
                token,
            )
            .await
    }
}

impl AudioService {
    pub async fn upload_playlist_cover(
        &self,
        playlist: &Playlist,
        jpeg: Vec<u8>,
        token: &str,
    ) -> Result<Thumb> {
        let url = self.cover_upload_url(playlist, token).await?;
        let response = self.upload_cover(jpeg, url).await?;
        let value: serde_json::Value =
            serde_json::from_str(&response).map_err(|e| Error::decoding("cover upload", e))?;
        if value.get("sha").is_none() {
            return Err(Error::InvalidState("Cover upload was rejected".into()));
        }
        let saved = self.save_cover(playlist.owner_id, &response, token).await?;
        self.set_playlist_cover(playlist, &saved, token).await
    }

    pub async fn upload_and_save_audio(
        &self,
        mp3: Vec<u8>,
        artist: &str,
        title: &str,
        token: &str,
    ) -> Result<Audio> {
        let url = self.audio_upload_url(token).await?;
        let ticket = self.upload_audio(mp3, url).await?;
        self.save_audio(&ticket, artist, title, token).await
    }
}
