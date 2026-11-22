use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

use crate::{AudioService, Error, Result, models::Audio, session::save_json};

const GENIUS_BASE_URL: &str = "https://api.genius.com";
const MUSIXMATCH_BASE_URL: &str = "https://apic-appmobile.musixmatch.com/ws/1.1";
const MUSIXMATCH_APP_ID: &str = "mac-ios-v2.0";
const MUSIXMATCH_APP_VERSION: &str = "1.37.2";
const MUSIXMATCH_USER_AGENT: &str =
    "Musixmatch/2025120901 CFNetwork/3860.300.31 Darwin/25.2.0";
const MUSIXMATCH_TOKEN_TTL_SECS: u64 = 7 * 24 * 60 * 60;
const MUSIXMATCH_TOKEN_RETRY_COOLDOWN: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LyricsSource {
    Vk,
    Musixmatch,
    Genius,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackLyrics {
    pub content: LyricsContent,
    pub source: LyricsSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LyricsContent {
    Plain(Vec<String>),
    Synced(Vec<SyncedLine>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncedLine {
    pub id: usize,
    pub begin: i64,
    pub end: i64,
    pub text: Option<String>,
}

impl SyncedLine {
    pub fn is_interlude(&self) -> bool {
        self.text.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VkLyricsResponse {
    pub lyrics: VkLyricsPayload,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VkLyricsPayload {
    pub language: Option<String>,
    pub text: Option<Vec<String>>,
    pub timestamps: Option<Vec<LyricsStamp>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricsStamp {
    pub begin: Option<i64>,
    pub end: Option<i64>,
    pub line: Option<String>,
    pub interlude: Option<bool>,
}

impl TrackLyrics {
    pub fn is_synced(&self) -> bool {
        matches!(self.content, LyricsContent::Synced(_))
    }

    pub fn from_vk(response: VkLyricsResponse) -> Option<Self> {
        let lines: Vec<_> = response
            .lyrics
            .timestamps
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .filter_map(|(id, stamp)| {
                let text = if stamp.interlude == Some(true) {
                    None
                } else {
                    Some(stamp.line?.trim().to_owned()).filter(|s| !s.is_empty())
                };
                if stamp.interlude != Some(true) && text.is_none() {
                    return None;
                }
                Some(SyncedLine {
                    id,
                    begin: stamp.begin?,
                    end: stamp.end?,
                    text,
                })
            })
            .collect();
        if lines.iter().any(|l| !l.is_interlude()) {
            return Some(Self {
                content: LyricsContent::Synced(lines),
                source: LyricsSource::Vk,
            });
        }
        let lines = tidy(response.lyrics.text.unwrap_or_default());
        (!lines.is_empty()).then_some(Self {
            content: LyricsContent::Plain(lines),
            source: LyricsSource::Vk,
        })
    }

    pub fn from_plain(text: &str, source: LyricsSource) -> Option<Self> {
        let lines = match source {
            LyricsSource::Genius => clean_genius_lyrics(text),
            LyricsSource::Musixmatch => clean_musixmatch_plain(text),
            LyricsSource::Vk => tidy(text.lines().map(str::to_owned).collect()),
        };
        (!lines.is_empty()).then_some(Self {
            content: LyricsContent::Plain(lines),
            source,
        })
    }
}

fn clean_line_whitespace(line: &str) -> String {
    line.chars()
        .filter(|c| !matches!(c, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}' | '\r'))
        .map(|c| if c == '\u{00a0}' { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_owned()
}

fn tidy(raw: Vec<String>) -> Vec<String> {
    let mut collapsed = Vec::with_capacity(raw.len());
    let mut prev_empty = true;
    for line in raw {
        let cleaned = clean_line_whitespace(&line);
        if cleaned.is_empty() {
            if !prev_empty {
                collapsed.push(String::new());
                prev_empty = true;
            }
        } else {
            collapsed.push(cleaned);
            prev_empty = false;
        }
    }
    while collapsed.last().is_some_and(String::is_empty) {
        collapsed.pop();
    }
    collapsed
}

fn strip_bracket_tags_in_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '['
            && let Some(rel_end) = chars[i + 1..].iter().position(|&c| c == ']')
        {
            i = i + 1 + rel_end + 1;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn clean_genius_lyrics(text: &str) -> Vec<String> {
    let raw_lines: Vec<String> = text.lines().map(clean_line_whitespace).collect();
    let mut processed = Vec::with_capacity(raw_lines.len());
    let mut i = 0;
    while i < raw_lines.len() {
        let line = &raw_lines[i];
        if line.is_empty() {
            processed.push(String::new());
            i += 1;
            continue;
        }
        if line.starts_with('[') && !line.contains(']') {
            let max_end = (i + 3).min(raw_lines.len().saturating_sub(1));
            if let Some(close_idx) = (i + 1..=max_end).find(|&j| raw_lines[j].contains(']')) {
                let tail = raw_lines[close_idx]
                    .split_once(']')
                    .map(|(_, after)| strip_bracket_tags_in_line(after))
                    .unwrap_or_default();
                processed.push(String::new());
                if !tail.is_empty() {
                    processed.push(tail);
                }
                i = close_idx + 1;
                continue;
            }
        }
        let cleaned = strip_bracket_tags_in_line(line);
        processed.push(cleaned);
        i += 1;
    }
    tidy(processed)
}

fn clean_musixmatch_plain(text: &str) -> Vec<String> {
    let mut kept = Vec::new();
    for line in text.lines() {
        let cleaned = clean_line_whitespace(line);
        if cleaned.starts_with("*******") && cleaned.contains("Commercial use") {
            break;
        }
        kept.push(cleaned);
    }
    tidy(kept)
}

fn strip_parentheticals(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut round = 0usize;
    let mut square = 0usize;
    for ch in text.chars() {
        match ch {
            '(' => round += 1,
            ')' if round > 0 => round -= 1,
            '[' => square += 1,
            ']' if square > 0 => square -= 1,
            _ if round == 0 && square == 0 => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn primary_artist_for_track(track: &Audio) -> &str {
    track
        .main_artists
        .as_ref()
        .and_then(|artists| artists.first())
        .map(|a| a.name.trim())
        .filter(|name| !name.is_empty())
        .unwrap_or(track.artist.as_str())
}

#[derive(Clone)]
pub struct GeniusService {
    client: reqwest::Client,
    base_url: String,
}

impl GeniusService {
    pub fn new() -> Result<Self> {
        Self::with_base_url(GENIUS_BASE_URL)
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .build()?,
            base_url: base_url.into().trim_end_matches('/').to_owned(),
        })
    }

    async fn request<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T> {
        let response = self
            .client
            .get(format!("{}{path}", self.base_url))
            .header(
                "user-agent",
                "Genius/4.2.1 (Android; Android 10; google Pixel 3)",
            )
            .header("x-genius-android-version", "4.2.1")
            .header("accept", "application/json")
            .query(query)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(Error::Http(response.status().as_u16()));
        }
        let bytes = response.bytes().await?;
        serde_json::from_slice(&bytes).map_err(|e| Error::decoding("Genius response", e))
    }

    async fn search_song_id(
        &self,
        query: &str,
        wanted_artists: &[String],
        wanted_titles: &[String],
    ) -> Result<Option<i64>> {
        let response: SearchEnvelope = self.request("/search/song", &[("q", query)]).await?;
        let best = response
            .response
            .sections
            .into_iter()
            .filter(|s| s.kind == "song")
            .flat_map(|s| s.hits)
            .filter(|h| h.kind == "song")
            .map(|h| h.result)
            .find(|h| {
                h.lyrics_state.as_deref() == Some("complete")
                    && h.instrumental != Some(true)
                    && [h.artist_names.as_deref(), h.primary_artist_names.as_deref()]
                        .into_iter()
                        .flatten()
                        .map(normalize)
                        .any(|a| wanted_artists.iter().any(|w| overlaps(&a, w)))
                    && {
                        let hit_title = normalize(h.title.as_deref().unwrap_or_default());
                        wanted_titles.iter().any(|w| overlaps(&hit_title, w))
                    }
            });
        Ok(best.map(|b| b.id))
    }

    pub async fn lyrics(&self, artist: &str, title: &str) -> Result<Option<TrackLyrics>> {
        self.lyrics_with_primary(artist, None, title).await
    }

    pub async fn lyrics_for_track(&self, track: &Audio) -> Result<Option<TrackLyrics>> {
        let primary = primary_artist_for_track(track);
        let primary_opt = (primary != track.artist).then_some(primary);
        self.lyrics_with_primary(&track.artist, primary_opt, &track.title)
            .await
    }

    async fn lyrics_with_primary(
        &self,
        artist: &str,
        primary_artist: Option<&str>,
        title: &str,
    ) -> Result<Option<TrackLyrics>> {
        let mut wanted_artists = vec![normalize(artist)];
        if let Some(primary) = primary_artist {
            let norm = normalize(primary);
            if !norm.is_empty() && !wanted_artists.contains(&norm) {
                wanted_artists.push(norm);
            }
        }
        let mut wanted_titles = vec![normalize(title)];
        let clean_title = strip_parentheticals(title);
        let norm_clean_title = normalize(&clean_title);
        if !norm_clean_title.is_empty() && !wanted_titles.contains(&norm_clean_title) {
            wanted_titles.push(norm_clean_title);
        }

        let query_artist = primary_artist.unwrap_or(artist);
        let primary_query = format!("{query_artist} {title}");
        let mut song_id = self
            .search_song_id(&primary_query, &wanted_artists, &wanted_titles)
            .await?;

        if song_id.is_none() && clean_title != title && !clean_title.is_empty() {
            let fallback_query = format!("{query_artist} {clean_title}");
            song_id = self
                .search_song_id(&fallback_query, &wanted_artists, &wanted_titles)
                .await?;
        }

        let Some(song_id) = song_id else {
            return Ok(None);
        };
        let response: SongEnvelope = self
            .request(&format!("/songs/{song_id}"), &[("text_format", "plain")])
            .await?;
        let song = response.response.song;
        if song.instrumental == Some(true) {
            return Ok(None);
        }
        Ok(song
            .lyrics
            .and_then(|l| l.plain)
            .and_then(|t| TrackLyrics::from_plain(&t, LyricsSource::Genius)))
    }
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn is_valid_musixmatch_token(token: &str) -> bool {
    let trimmed = token.trim();
    !trimmed.is_empty()
        && !trimmed.chars().all(|c| c == '0')
        && !trimmed.contains("UpgradeOnly")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedMusixmatchToken {
    token: String,
    created_at: u64,
}

#[derive(Default)]
struct MusixmatchTokenState {
    token: Option<String>,
    created_at: u64,
    loaded_from_disk: bool,
    last_failure: Option<Instant>,
}

#[derive(Clone)]
pub struct MusixmatchService {
    client: reqwest::Client,
    base_url: String,
    token_state: Arc<Mutex<MusixmatchTokenState>>,
    token_path: Option<PathBuf>,
}

impl MusixmatchService {
    pub fn new() -> Result<Self> {
        Self::with_base_url(MUSIXMATCH_BASE_URL)
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .cookie_store(true)
                .timeout(Duration::from_secs(15))
                .build()?,
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            token_state: Arc::new(Mutex::new(MusixmatchTokenState::default())),
            token_path: None,
        })
    }

    pub fn with_token_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.token_path = Some(path.into());
        self
    }

    async fn get_json(&self, path: &str, query: &[(&str, &str)]) -> Result<Value> {
        let response = self
            .client
            .get(format!("{}{path}", self.base_url))
            .header("accept", "application/json")
            .header("accept-language", "en-US,en;q=0.9")
            .header("x-mxm-app-version", MUSIXMATCH_APP_VERSION)
            .header("user-agent", MUSIXMATCH_USER_AGENT)
            .query(query)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(Error::Http(response.status().as_u16()));
        }
        let bytes = response.bytes().await?;
        serde_json::from_slice(&bytes).map_err(|e| Error::decoding("Musixmatch response", e))
    }

    async fn acquire_token(&self, force_refresh: bool) -> Result<(String, bool)> {
        let mut guard = self.token_state.lock().await;
        let now = unix_now_secs();

        if !guard.loaded_from_disk {
            guard.loaded_from_disk = true;
            if let Some(path) = &self.token_path
                && let Some(saved) = load_persisted_token(path)
                && is_valid_musixmatch_token(&saved.token)
                && now < saved.created_at.saturating_add(MUSIXMATCH_TOKEN_TTL_SECS)
            {
                guard.token = Some(saved.token);
                guard.created_at = saved.created_at;
            }
        }

        if !force_refresh
            && let Some(token) = &guard.token
            && now < guard.created_at.saturating_add(MUSIXMATCH_TOKEN_TTL_SECS)
        {
            return Ok((token.clone(), false));
        }

        if let Some(failed_at) = guard.last_failure
            && failed_at.elapsed() < MUSIXMATCH_TOKEN_RETRY_COOLDOWN
        {
            return Err(Error::Http(401));
        }

        let payload = match self
            .get_json("/token.get", &[("app_id", MUSIXMATCH_APP_ID)])
            .await
        {
            Ok(v) => v,
            Err(err) => {
                guard.last_failure = Some(Instant::now());
                return Err(err);
            }
        };

        let status = mxm_status_code(&payload);
        let token = payload
            .pointer("/message/body/user_token")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|t| is_valid_musixmatch_token(t))
            .map(str::to_owned);

        let Some(token) = token.filter(|_| status == Some(200)) else {
            guard.last_failure = Some(Instant::now());
            return Err(Error::Http(status.unwrap_or(401)));
        };

        guard.token = Some(token.clone());
        guard.created_at = now;
        guard.last_failure = None;
        if let Some(path) = &self.token_path {
            let _ = save_json(
                path,
                &PersistedMusixmatchToken {
                    token: token.clone(),
                    created_at: now,
                },
            );
        }
        Ok((token, true))
    }

    async fn invalidate_token(&self) {
        let mut guard = self.token_state.lock().await;
        guard.token = None;
        guard.created_at = 0;
        if let Some(path) = &self.token_path {
            let _ = fs::remove_file(path);
        }
    }

    pub async fn lyrics(&self, track: &Audio) -> Result<Option<TrackLyrics>> {
        let (mut token, mut refreshed) = self.acquire_token(false).await?;
        for _ in 0..2 {
            let payload = self.fetch_macro_subtitles(track, &token).await?;
            match mxm_status_code(&payload) {
                Some(200) => {
                    let duration_ms = track.duration.max(0) * 1000;
                    return Ok(parse_musixmatch_macro(&payload, duration_ms));
                }
                Some(401) if !refreshed => {
                    self.invalidate_token().await;
                    let next = self.acquire_token(true).await?;
                    token = next.0;
                    refreshed = next.1;
                }
                Some(404) => return Ok(None),
                Some(code) => return Err(Error::Http(code)),
                None => return Ok(None),
            }
        }
        Ok(None)
    }

    async fn fetch_macro_subtitles(&self, track: &Audio, token: &str) -> Result<Value> {
        let artist = primary_artist_for_track(track);
        let duration_str = track.duration.to_string();
        let album_title = track
            .album
            .as_ref()
            .and_then(|a| a.title.as_deref())
            .map(str::trim)
            .filter(|t| !t.is_empty());

        let mut query = vec![
            ("app_id", MUSIXMATCH_APP_ID),
            ("format", "json"),
            ("namespace", "lyrics_richsynched"),
            ("subtitle_format", "mxm"),
            ("usertoken", token),
            ("q_track", track.title.as_str()),
            ("q_artist", artist),
        ];
        if track.duration > 0 {
            query.push(("q_duration", duration_str.as_str()));
        }
        if let Some(album) = album_title {
            query.push(("q_album", album));
        }
        self.get_json("/macro.subtitles.get", &query).await
    }
}

fn load_persisted_token(path: &Path) -> Option<PersistedMusixmatchToken> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn mxm_status_code(value: &Value) -> Option<u16> {
    value
        .pointer("/message/header/status_code")
        .and_then(Value::as_u64)
        .and_then(|code| u16::try_from(code).ok())
}

fn is_flag_true(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
        _ => false,
    }
}

pub fn parse_musixmatch_macro(payload: &Value, duration_ms: i64) -> Option<TrackLyrics> {
    let calls = payload.pointer("/message/body/macro_calls")?.as_object()?;

    if let Some(matcher) = calls.get("matcher.track.get")
        && mxm_status_code(matcher) == Some(200)
        && is_flag_true(matcher.pointer("/message/body/track/instrumental"))
    {
        return None;
    }

    if let Some(sub_call) = calls.get("track.subtitles.get")
        && mxm_status_code(sub_call) == Some(200)
        && let Some(body) = sub_call
            .pointer("/message/body/subtitle_list/0/subtitle/subtitle_body")
            .and_then(Value::as_str)
        && let Some(lines) = parse_musixmatch_subtitles(body, duration_ms)
    {
        return Some(TrackLyrics {
            content: LyricsContent::Synced(lines),
            source: LyricsSource::Musixmatch,
        });
    }

    if let Some(lyr_call) = calls.get("track.lyrics.get")
        && mxm_status_code(lyr_call) == Some(200)
        && !is_flag_true(lyr_call.pointer("/message/body/lyrics/instrumental"))
        && let Some(body) = lyr_call
            .pointer("/message/body/lyrics/lyrics_body")
            .and_then(Value::as_str)
    {
        return TrackLyrics::from_plain(body, LyricsSource::Musixmatch);
    }

    None
}

pub fn parse_musixmatch_subtitles(subtitle_body: &str, duration_ms: i64) -> Option<Vec<SyncedLine>> {
    let mut raw: Vec<(i64, String)> =
        if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(subtitle_body) {
            items
                .into_iter()
                .filter_map(|item| {
                    let total_secs = item.pointer("/time/total").and_then(Value::as_f64)?;
                    let begin = (total_secs * 1000.0).round() as i64;
                    let text = item
                        .get("text")
                        .and_then(Value::as_str)
                        .map(clean_line_whitespace)
                        .unwrap_or_default();
                    Some((begin.max(0), text))
                })
                .collect()
        } else {
            subtitle_body.lines().filter_map(parse_lrc_line).collect()
        };

    if raw.is_empty() {
        return None;
    }
    raw.sort_by_key(|(begin, _)| *begin);

    let last_begin = raw.last().map(|(b, _)| *b).unwrap_or(0);
    let fallback_end = duration_ms.max(last_begin + 5000);
    let mut lines: Vec<SyncedLine> = Vec::with_capacity(raw.len());

    for (idx, (begin, text)) in raw.iter().enumerate() {
        let end = raw
            .get(idx + 1)
            .map(|(next_begin, _)| *next_begin)
            .unwrap_or(fallback_end)
            .max(*begin);

        if text.is_empty() {
            if idx + 1 == raw.len() {
                continue;
            }
            if let Some(prev) = lines.last_mut()
                && prev.is_interlude()
            {
                prev.end = end;
                continue;
            }
            lines.push(SyncedLine {
                id: lines.len(),
                begin: *begin,
                end,
                text: None,
            });
        } else {
            lines.push(SyncedLine {
                id: lines.len(),
                begin: *begin,
                end,
                text: Some(text.clone()),
            });
        }
    }

    lines.iter().any(|l| !l.is_interlude()).then_some(lines)
}

fn parse_lrc_line(line: &str) -> Option<(i64, String)> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix('[')?;
    let (stamp, after) = rest.split_once(']')?;
    let (min_str, sec_part) = stamp.split_once(':')?;
    let mins: i64 = min_str.parse().ok()?;
    let (sec_str, frac_ms) = match sec_part.split_once('.') {
        Some((s, frac)) => {
            if frac.is_empty() || frac.len() > 3 || !frac.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let val: i64 = frac.parse().ok()?;
            let ms = match frac.len() {
                1 => val * 100,
                2 => val * 10,
                _ => val,
            };
            (s, ms)
        }
        None => (sec_part, 0),
    };
    if sec_str.len() != 2 {
        return None;
    }
    let secs: i64 = sec_str.parse().ok()?;
    if secs >= 60 {
        return None;
    }
    let begin = mins * 60_000 + secs * 1_000 + frac_ms;
    Some((begin, clean_line_whitespace(after)))
}

fn normalize(text: &str) -> String {
    text.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn overlaps(a: &str, b: &str) -> bool {
    !a.is_empty() && !b.is_empty() && (a.contains(b) || b.contains(a))
}

#[derive(Deserialize)]
struct SearchEnvelope {
    response: SearchData,
}
#[derive(Deserialize)]
struct SearchData {
    sections: Vec<SearchSection>,
}
#[derive(Deserialize)]
struct SearchSection {
    #[serde(rename = "type")]
    kind: String,
    hits: Vec<SearchHit>,
}
#[derive(Deserialize)]
struct SearchHit {
    #[serde(rename = "type")]
    kind: String,
    result: SongSummary,
}
#[derive(Deserialize)]
struct SongSummary {
    id: i64,
    title: Option<String>,
    artist_names: Option<String>,
    primary_artist_names: Option<String>,
    lyrics_state: Option<String>,
    instrumental: Option<bool>,
}
#[derive(Deserialize)]
struct SongEnvelope {
    response: SongData,
}
#[derive(Deserialize)]
struct SongData {
    song: Song,
}
#[derive(Deserialize)]
struct Song {
    instrumental: Option<bool>,
    lyrics: Option<PlainLyrics>,
}
#[derive(Deserialize)]
struct PlainLyrics {
    plain: Option<String>,
}

fn lyrics_priority(lyrics: &TrackLyrics) -> u8 {
    match (&lyrics.content, lyrics.source) {
        (LyricsContent::Synced(_), LyricsSource::Vk) => 5,
        (LyricsContent::Synced(_), LyricsSource::Musixmatch) => 4,
        (LyricsContent::Synced(_), LyricsSource::Genius) => 3,
        (LyricsContent::Plain(_), LyricsSource::Vk) => 2,
        (LyricsContent::Plain(_), LyricsSource::Genius) => 1,
        (LyricsContent::Plain(_), LyricsSource::Musixmatch) => 0,
    }
}

pub fn pick_best_lyrics(
    candidates: impl IntoIterator<Item = Option<TrackLyrics>>,
) -> Option<TrackLyrics> {
    candidates
        .into_iter()
        .flatten()
        .max_by_key(lyrics_priority)
}

pub struct LyricsProvider {
    service: AudioService,
    genius: GeniusService,
    musixmatch: MusixmatchService,
    cache: HashMap<String, Option<TrackLyrics>>,
}

impl LyricsProvider {
    pub fn new(service: AudioService) -> Result<Self> {
        Ok(Self {
            service,
            genius: GeniusService::new()?,
            musixmatch: MusixmatchService::new()?,
            cache: HashMap::new(),
        })
    }

    pub fn with_musixmatch_token_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.musixmatch = self.musixmatch.with_token_path(path);
        self
    }

    pub fn with_services(
        service: AudioService,
        genius: GeniusService,
        musixmatch: MusixmatchService,
    ) -> Self {
        Self {
            service,
            genius,
            musixmatch,
            cache: HashMap::new(),
        }
    }

    pub fn cached(&self, track: &Audio) -> Option<&Option<TrackLyrics>> {
        self.cache.get(&track.stable_id())
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }

    pub async fn load(
        &mut self,
        track: &Audio,
        token: Option<&str>,
        external_fallback: bool,
    ) -> Result<Option<TrackLyrics>> {
        let id = track.stable_id();
        if let Some(cached) = self.cache.get(&id) {
            return Ok(cached.clone());
        }

        let mut vk_lyrics = None;
        if track.has_lyrics == Some(true)
            && let Some(token) = token
            && let Ok(response) = self.service.lyrics(track, token).await
        {
            vk_lyrics = TrackLyrics::from_vk(response);
        }

        if vk_lyrics.as_ref().is_some_and(TrackLyrics::is_synced) {
            self.cache.insert(id, vk_lyrics.clone());
            return Ok(vk_lyrics);
        }

        if !external_fallback {
            self.cache.insert(id, vk_lyrics.clone());
            return Ok(vk_lyrics);
        }

        let (mxm_res, genius_res) = if vk_lyrics.is_some() {
            (self.musixmatch.lyrics(track).await, Ok(None))
        } else {
            tokio::join!(
                self.musixmatch.lyrics(track),
                self.genius.lyrics_for_track(track),
            )
        };

        let (mxm_lyrics, mxm_err) = match mxm_res {
            Ok(val) => (val, None),
            Err(err) => (None, Some(err)),
        };
        let (genius_lyrics, genius_err) = match genius_res {
            Ok(val) => (val, None),
            Err(err) => (None, Some(err)),
        };

        let best = pick_best_lyrics([vk_lyrics, mxm_lyrics, genius_lyrics]);
        if let Some(best) = best {
            self.cache.insert(id, Some(best.clone()));
            return Ok(Some(best));
        }

        if mxm_err.is_some()
            && let Some(err) = genius_err
        {
            return Err(err);
        }

        if mxm_err.is_none() && genius_err.is_none() {
            self.cache.insert(id, None);
        }
        Ok(None)
    }
}
