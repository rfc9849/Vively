use std::{collections::HashSet, fs, path::Path};

use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::{
    AudioService, Error, Result,
    models::Audio,
    session::{save_json, unix_now},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }
    pub fn event_value(self) -> &'static str {
        match self {
            Self::Off => "none",
            Self::All => "all",
            Self::One => "one",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackOrigin {
    Library,
    #[default]
    Other,
}

#[derive(Debug, Clone, Default)]
pub struct PlaybackQueue {
    tracks: Vec<Audio>,
    unshuffled: Vec<Audio>,
    current_index: Option<usize>,
    pub repeat_mode: RepeatMode,
    shuffled: bool,
    origin: PlaybackOrigin,
    inserted: HashSet<String>,
    stale: HashSet<String>,
}

impl PlaybackQueue {
    pub fn tracks(&self) -> &[Audio] {
        &self.tracks
    }
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }
    pub fn current(&self) -> Option<&Audio> {
        self.tracks.get(self.current_index?)
    }
    pub fn is_shuffled(&self) -> bool {
        self.shuffled
    }
    pub fn upcoming(&self) -> &[Audio] {
        self.current_index
            .and_then(|i| self.tracks.get(i + 1..))
            .unwrap_or_default()
    }

    pub fn origin(&self, track: &Audio) -> PlaybackOrigin {
        if self.inserted.contains(&track.full_id()) {
            PlaybackOrigin::Other
        } else {
            self.origin
        }
    }

    pub fn replace(&mut self, tracks: Vec<Audio>, start: usize, origin: PlaybackOrigin) -> bool {
        let Some(anchor) = tracks.get(start).cloned() else {
            return false;
        };
        self.tracks = tracks.clone();
        self.unshuffled = tracks;
        self.current_index = Some(start);
        self.origin = origin;
        self.inserted.clear();
        self.stale.clear();
        if self.shuffled {
            self.shuffle_around(Some(anchor));
        }
        true
    }

    pub fn play_next(&mut self, track: Audio) {
        let Some(index) = self.current_index else {
            self.replace(vec![track], 0, PlaybackOrigin::Other);
            return;
        };
        let anchor = self.current().map(Audio::stable_id);
        self.inserted.insert(track.full_id());
        self.tracks.insert(index + 1, track.clone());
        let position = self
            .unshuffled
            .iter()
            .position(|a| Some(a.stable_id()) == anchor)
            .map(|i| i + 1)
            .unwrap_or(self.unshuffled.len());
        self.unshuffled.insert(position, track);
    }

    pub fn append(&mut self, tracks: Vec<Audio>) {
        self.unshuffled.extend(tracks.clone());
        self.tracks.extend(tracks);
    }

    pub fn needs_more(&self) -> bool {
        self.current_index
            .is_some_and(|i| self.tracks.len().saturating_sub(i + 1) <= 5)
    }

    pub fn select(&mut self, index: usize) -> bool {
        if index >= self.tracks.len() {
            return false;
        }
        self.current_index = Some(index);
        true
    }

    pub fn select_track(&mut self, track: &Audio) -> bool {
        let start = self.current_index.map(|i| i + 1).unwrap_or(0);
        let index = self
            .tracks
            .iter()
            .enumerate()
            .skip(start)
            .find(|(_, a)| *a == track)
            .or_else(|| self.tracks.iter().enumerate().find(|(_, a)| *a == track))
            .map(|(i, _)| i);
        index.is_some_and(|i| self.select(i))
    }

    pub fn next_index(&self) -> Option<usize> {
        let i = self.current_index?;
        if self.repeat_mode == RepeatMode::One {
            return Some(i);
        }
        if i + 1 < self.tracks.len() {
            Some(i + 1)
        } else if self.repeat_mode == RepeatMode::All && !self.tracks.is_empty() {
            Some(0)
        } else {
            None
        }
    }

    pub fn advance(&mut self) -> bool {
        self.next_index().is_some_and(|i| self.select(i))
    }

    pub fn previous(&mut self, position: f64) -> usize {
        let i = self.current_index.unwrap_or(0);
        let index = if position > 3.0 {
            i
        } else {
            i.saturating_sub(1)
        };
        self.select(index);
        index
    }

    pub fn remove_upcoming(&mut self, offsets: &[usize]) {
        let Some(current) = self.current_index else {
            return;
        };
        let indices: HashSet<_> = offsets
            .iter()
            .filter_map(|o| current.checked_add(1)?.checked_add(*o))
            .collect();
        self.tracks = self
            .tracks
            .iter()
            .enumerate()
            .filter(|(i, _)| !indices.contains(i))
            .map(|(_, a)| a.clone())
            .collect();
        if !self.shuffled {
            self.unshuffled = self.tracks.clone();
        }
    }

    pub fn move_upcoming(&mut self, from: usize, to: usize) -> bool {
        let Some(current) = self.current_index else {
            return false;
        };
        let remaining = self.tracks.len().saturating_sub(current + 1);
        if from >= remaining || to >= remaining {
            return false;
        }
        let track = self.tracks.remove(current + 1 + from);
        self.tracks.insert(current + 1 + to, track);
        if !self.shuffled {
            self.unshuffled = self.tracks.clone();
        }
        true
    }

    pub fn set_shuffled(&mut self, shuffled: bool) {
        if shuffled == self.shuffled {
            return;
        }
        let current = self.current().cloned();
        self.shuffled = shuffled;
        if shuffled {
            self.shuffle_around(current);
        } else {
            self.tracks = self.unshuffled.clone();
            self.current_index = current.and_then(|a| self.tracks.iter().position(|t| *t == a));
        }
    }

    fn shuffle_around(&mut self, anchor: Option<Audio>) {
        let mut rest: Vec<_> = self
            .unshuffled
            .iter()
            .filter(|a| Some(*a) != anchor.as_ref())
            .cloned()
            .collect();
        rest.shuffle(&mut rand::rng());
        self.tracks = anchor.into_iter().chain(rest).collect();
        self.current_index = (!self.tracks.is_empty()).then_some(0);
    }

    pub fn replace_track(&mut self, fresh: Audio) {
        let id = fresh.full_id();
        for track in self.tracks.iter_mut().chain(self.unshuffled.iter_mut()) {
            if track.full_id() == id {
                *track = fresh.clone();
            }
        }
        self.stale.remove(&id);
    }

    pub fn is_stale(&self, track: &Audio) -> bool {
        self.stale.contains(&track.full_id())
    }

    pub fn snapshot(&self, user_id: i64, now: i64) -> Option<PlaybackSnapshot> {
        let mut index = self.current_index?;
        let mut tracks = self.tracks.clone();
        let mut unshuffled_tracks = self.shuffled.then(|| self.unshuffled.clone());
        if tracks.len() > 600 {
            let start = index.saturating_sub(100);
            tracks = tracks[start..tracks.len().min(start + 600)].to_vec();
            index -= start;
            unshuffled_tracks = None;
        }
        Some(PlaybackSnapshot {
            user_id,
            saved_at: now,
            tracks,
            unshuffled_tracks,
            current_index: index,
            repeat_mode: self.repeat_mode,
            is_shuffled: self.shuffled,
            origin: self.origin,
            inserted_track_ids: self.inserted.clone(),
            stale_track_ids: self.stale.clone(),
        })
    }

    pub fn restore(snapshot: PlaybackSnapshot, user_id: i64, now: i64) -> Result<Self> {
        if snapshot.user_id != user_id || snapshot.current_index >= snapshot.tracks.len() {
            return Err(Error::InvalidState(
                "Saved queue does not match the session or has an invalid index".into(),
            ));
        }
        let mut stale = snapshot.stale_track_ids;
        if now.saturating_sub(snapshot.saved_at) >= 30 * 60 {
            stale.extend(snapshot.tracks.iter().map(Audio::full_id));
        }
        Ok(Self {
            unshuffled: snapshot
                .unshuffled_tracks
                .unwrap_or_else(|| snapshot.tracks.clone()),
            tracks: snapshot.tracks,
            current_index: Some(snapshot.current_index),
            repeat_mode: snapshot.repeat_mode,
            shuffled: snapshot.is_shuffled,
            origin: snapshot.origin,
            inserted: snapshot.inserted_track_ids,
            stale,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaybackSnapshot {
    pub user_id: i64,
    pub saved_at: i64,
    pub tracks: Vec<Audio>,
    pub unshuffled_tracks: Option<Vec<Audio>>,
    pub current_index: usize,
    pub repeat_mode: RepeatMode,
    pub is_shuffled: bool,
    pub origin: PlaybackOrigin,
    pub inserted_track_ids: HashSet<String>,
    pub stale_track_ids: HashSet<String>,
}

impl PlaybackSnapshot {
    pub fn save(&self, path: &Path) -> Result<()> {
        save_json(path, self)
    }
    pub fn load(path: &Path) -> Result<Option<Self>> {
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| Error::decoding("saved queue", e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaybackPosition {
    pub track_id: String,
    pub time: f64,
}

impl PlaybackPosition {
    pub fn time_for(&self, track: &Audio) -> f64 {
        if self.track_id == track.stable_id() && self.time.is_finite() {
            self.time.max(0.0)
        } else {
            0.0
        }
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        save_json(path, self)
    }
    pub fn load(path: &Path) -> Result<Option<Self>> {
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| Error::decoding("saved position", e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaybackEvent {
    #[serde(rename = "e")]
    pub event: String,
    pub audio_id: String,
    pub uuid: i32,
    pub shuffle: bool,
    pub reason: String,
    pub start_time: i64,
    pub playback_started_at: i64,
    pub track_code: String,
    pub streaming_type: String,
    pub duration: i64,
    #[serde(rename = "repeat")]
    pub repeat_mode: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "prev_audio_id")]
    pub previous_audio_id: Option<String>,
}

#[derive(Clone)]
struct TrackSession {
    track: Audio,
    start_time: i64,
    position: f64,
}

#[derive(Default)]
pub struct PlaybackReporter {
    current: Option<TrackSession>,
    paused: Option<TrackSession>,
}

impl PlaybackReporter {
    pub fn reset(&mut self) {
        self.current = None;
        self.paused = None;
    }

    pub fn update_position(&mut self, track: &Audio, position: f64) {
        if let Some(current) = &mut self.current
            && current.track.stable_id() == track.stable_id()
            && position.is_finite()
        {
            current.position = position.max(0.0);
        }
    }

    pub fn sync(
        &mut self,
        queue: &PlaybackQueue,
        playing: bool,
        position: f64,
        enabled: bool,
        now: i64,
    ) -> Vec<PlaybackEvent> {
        if !enabled {
            self.reset();
            return Vec::new();
        }
        let track = playing.then(|| queue.current()).flatten();
        if self
            .current
            .as_ref()
            .is_some_and(|s| track.is_some_and(|t| t.stable_id() == s.track.stable_id()))
        {
            return Vec::new();
        }
        let mut events = Vec::new();
        let mut replaced = None;
        if let Some(ending) = self.current.take() {
            events.push(make_event(
                "music_stop_playback",
                &ending,
                if track.is_some() { "next" } else { "pause" },
                queue,
                None,
            ));
            if track.is_some() {
                replaced = Some(ending.track.full_id());
                self.paused = None;
            } else {
                self.paused = Some(ending);
            }
        }
        if let Some(track) = track {
            let resumed = self
                .paused
                .take()
                .filter(|s| s.track.stable_id() == track.stable_id());
            let reason = if resumed.is_some() {
                "continue"
            } else if replaced.is_some() {
                "next"
            } else {
                "new"
            };
            let session = TrackSession {
                track: track.clone(),
                start_time: resumed.as_ref().map(|s| s.start_time).unwrap_or(now),
                position: resumed.map(|s| s.position).unwrap_or(position.max(0.0)),
            };
            events.push(make_event(
                "music_start_playback",
                &session,
                reason,
                queue,
                replaced,
            ));
            self.current = Some(session);
        }
        events
    }
}

fn make_event(
    name: &str,
    session: &TrackSession,
    reason: &str,
    queue: &PlaybackQueue,
    previous: Option<String>,
) -> PlaybackEvent {
    let position = session.position.round() as i64;
    PlaybackEvent {
        event: name.into(),
        audio_id: session.track.full_id(),
        uuid: rand::random(),
        shuffle: queue.shuffled,
        reason: reason.into(),
        start_time: session.start_time,
        playback_started_at: position,
        track_code: session.track.track_code.clone().unwrap_or_default(),
        streaming_type: "online".into(),
        duration: position,
        repeat_mode: queue.repeat_mode.event_value().into(),
        state: "app".into(),
        previous_audio_id: previous,
    }
}

#[derive(Default)]
pub struct StatusBroadcaster {
    track_id: Option<String>,
}

impl StatusBroadcaster {
    pub fn reset(&mut self) {
        self.track_id = None;
    }
    pub async fn sync(
        &mut self,
        service: &AudioService,
        track: Option<&Audio>,
        playing: bool,
        enabled: bool,
        token: &str,
    ) -> Result<()> {
        let desired = if playing && enabled { track } else { None };
        let id = desired.map(Audio::full_id);
        if id == self.track_id {
            return Ok(());
        }
        service.set_broadcast(desired, token).await?;
        self.track_id = id;
        Ok(())
    }
}

pub fn snapshot_now(queue: &PlaybackQueue, user_id: i64) -> Option<PlaybackSnapshot> {
    queue.snapshot(user_id, unix_now())
}
