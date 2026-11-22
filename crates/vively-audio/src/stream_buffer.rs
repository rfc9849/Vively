use std::{
    collections::HashMap,
    io::{self, Read, Seek, SeekFrom},
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use futures_util::StreamExt;
use url::Url;
use vively_core::{
    Error, Result,
    config::USER_AGENT,
    media::{TsDemuxState, decrypt_aes128_cbc, extract_audio_with_state, is_adts_aac, parse_playlist, strip_leading_id3},
};

const WAIT_STEP: Duration = Duration::from_millis(250);
const INITIAL_BUFFER_TARGET: usize = 24 * 1024;

struct BufferInner {
    bytes: Vec<u8>,
    eof: bool,
    cancelled: bool,
    error: Option<String>,
}

pub struct StreamingAudioBuffer {
    inner: Mutex<BufferInner>,
    ready: Condvar,
}

impl StreamingAudioBuffer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(BufferInner {
                bytes: Vec::with_capacity(256 * 1024),
                eof: false,
                cancelled: false,
                error: None,
            }),
            ready: Condvar::new(),
        })
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(BufferInner {
                bytes,
                eof: true,
                cancelled: false,
                error: None,
            }),
            ready: Condvar::new(),
        })
    }

    pub fn push(&self, chunk: &[u8]) -> bool {
        if chunk.is_empty() {
            return !self.is_cancelled();
        }
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if guard.cancelled {
            return false;
        }
        guard.bytes.extend_from_slice(chunk);
        self.ready.notify_all();
        true
    }

    pub fn finish(&self) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.eof = true;
        self.ready.notify_all();
    }

    pub fn fail(&self, message: impl Into<String>) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.error = Some(message.into());
        guard.eof = true;
        self.ready.notify_all();
    }

    pub fn cancel(&self) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.cancelled = true;
        self.ready.notify_all();
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .cancelled
    }

    pub fn buffered_len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .bytes
            .len()
    }

    pub fn complete_len(&self) -> Option<u64> {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        (guard.eof && guard.error.is_none() && !guard.bytes.is_empty())
            .then_some(guard.bytes.len() as u64)
    }

    pub fn format_hint(&self) -> Option<&'static str> {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let head = strip_leading_id3(&guard.bytes);
        if is_adts_aac(head) {
            Some("aac")
        } else if head.len() >= 2 && head[0] == 0xff && (head[1] & 0xe0) == 0xe0 {
            Some("mp3")
        } else {
            None
        }
    }

    pub fn wait_until_playable(&self, min_bytes: usize, timeout: Duration) -> Result<()> {
        let deadline = std::time::Instant::now() + timeout;
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if guard.cancelled {
                return Err(Error::Media("Stream cancelled".into()));
            }
            if guard.bytes.len() >= min_bytes || (guard.eof && !guard.bytes.is_empty()) {
                return Ok(());
            }
            if let Some(err) = &guard.error {
                return Err(Error::Media(err.clone()));
            }
            if guard.eof {
                return Err(Error::Media("Audio stream produced no bytes".into()));
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return Err(Error::Media("Timed out waiting for audio stream".into()));
            }
            let wait = (deadline - now).min(WAIT_STEP);
            let (next_guard, _) = self
                .ready
                .wait_timeout(guard, wait)
                .unwrap_or_else(|e| e.into_inner());
            guard = next_guard;
        }
    }

    pub fn reader(self: &Arc<Self>) -> StreamingAudioReader {
        StreamingAudioReader {
            shared: Arc::clone(self),
            pos: 0,
        }
    }
}

pub struct StreamingAudioReader {
    shared: Arc<StreamingAudioBuffer>,
    pos: u64,
}

impl Read for StreamingAudioReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        let mut guard = self.shared.inner.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if guard.cancelled {
                return Ok(0);
            }
            let pos = self.pos as usize;
            if pos < guard.bytes.len() {
                let available = &guard.bytes[pos..];
                let count = available.len().min(out.len());
                out[..count].copy_from_slice(&available[..count]);
                self.pos += count as u64;
                return Ok(count);
            }
            if let Some(err) = &guard.error {
                return Err(io::Error::other(err.clone()));
            }
            if guard.eof {
                return Ok(0);
            }
            let (next_guard, _) = self
                .shared
                .ready
                .wait_timeout(guard, WAIT_STEP)
                .unwrap_or_else(|e| e.into_inner());
            guard = next_guard;
        }
    }
}

impl Seek for StreamingAudioReader {
    fn seek(&mut self, style: SeekFrom) -> io::Result<u64> {
        let mut guard = self.shared.inner.lock().unwrap_or_else(|e| e.into_inner());
        let target: i128 = match style {
            SeekFrom::Start(offset) => offset as i128,
            SeekFrom::Current(delta) => self.pos as i128 + delta as i128,
            SeekFrom::End(delta) => {
                if !guard.eof {
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "Cannot seek from end before stream completes",
                    ));
                }
                guard.bytes.len() as i128 + delta as i128
            }
        };
        if target < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Negative seek position",
            ));
        }
        let target = target as u64;
        while (guard.bytes.len() as u64) < target && !guard.eof && !guard.cancelled {
            let (next_guard, _) = self
                .shared
                .ready
                .wait_timeout(guard, WAIT_STEP)
                .unwrap_or_else(|e| e.into_inner());
            guard = next_guard;
        }
        if guard.cancelled {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "Stream cancelled"));
        }
        let clamped = if guard.eof {
            target.min(guard.bytes.len() as u64)
        } else {
            target
        };
        self.pos = clamped;
        Ok(clamped)
    }
}

pub fn make_stream_http_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(180))
        .build()?)
}

pub async fn stream_url_into_buffer(
    client: reqwest::Client,
    url: Url,
    buffer: Arc<StreamingAudioBuffer>,
) {
    if let Err(err) = stream_url_inner(&client, url, &buffer).await {
        if !buffer.is_cancelled() {
            buffer.fail(err.to_string());
        }
    } else {
        buffer.finish();
    }
}

async fn stream_url_inner(
    client: &reqwest::Client,
    url: Url,
    buffer: &Arc<StreamingAudioBuffer>,
) -> Result<()> {
    let response = client.get(url.clone()).send().await?;
    if !response.status().is_success() {
        return Err(Error::Http(response.status().as_u16()));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let looks_hls = url.path().to_ascii_lowercase().ends_with(".m3u8")
        || content_type.contains("mpegurl")
        || content_type.contains("vnd.apple");

    if looks_hls {
        let playlist_bytes = response.bytes().await?;
        let text = std::str::from_utf8(&playlist_bytes)
            .map_err(|_| Error::Media("Invalid HLS playlist encoding".into()))?;
        return stream_hls_segments(client, text, &url, buffer).await;
    }

    let mut byte_stream = response.bytes_stream();
    let mut first_chunk = true;
    while let Some(chunk) = byte_stream.next().await {
        if buffer.is_cancelled() {
            return Ok(());
        }
        let chunk = chunk?;
        if first_chunk {
            first_chunk = false;
            if let Ok(text) = std::str::from_utf8(&chunk)
                && text
                    .trim_start_matches('\u{feff}')
                    .trim_start()
                    .starts_with("#EXTM3U")
            {
                let mut full = chunk.to_vec();
                while let Some(rest) = byte_stream.next().await {
                    full.extend_from_slice(&rest?);
                }
                let full_text = std::str::from_utf8(&full)
                    .map_err(|_| Error::Media("Invalid HLS playlist encoding".into()))?;
                return stream_hls_segments(client, full_text, &url, buffer).await;
            }
            let stripped = strip_leading_id3(&chunk);
            if !buffer.push(stripped) {
                return Ok(());
            }
        } else if !buffer.push(&chunk) {
            return Ok(());
        }
    }
    Ok(())
}

async fn fetch_bytes(client: &reqwest::Client, url: Url) -> Result<Vec<u8>> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(Error::Http(resp.status().as_u16()));
    }
    Ok(resp.bytes().await?.to_vec())
}

async fn stream_hls_segments(
    client: &reqwest::Client,
    playlist_text: &str,
    base_url: &Url,
    buffer: &Arc<StreamingAudioBuffer>,
) -> Result<()> {
    let segments = parse_playlist(playlist_text, base_url)?;
    let mut keys: HashMap<Url, Vec<u8>> = HashMap::new();
    for seg in &segments {
        if let Some(key_url) = &seg.key_url
            && !keys.contains_key(key_url)
        {
            if buffer.is_cancelled() {
                return Ok(());
            }
            let key = fetch_bytes(client, key_url.clone()).await?;
            if key.len() != 16 {
                return Err(Error::Media("Invalid HLS encryption key".into()));
            }
            keys.insert(key_url.clone(), key);
        }
    }

    let keys = Arc::new(keys);
    let mut tasks = futures_util::stream::iter(segments.into_iter().map(|seg| {
        let client = client.clone();
        let keys = Arc::clone(&keys);
        async move {
            let raw = fetch_bytes(&client, seg.url).await?;
            let ts = if let Some(key_url) = seg.key_url {
                decrypt_aes128_cbc(&raw, &keys[&key_url], &seg.iv)?
            } else {
                raw
            };
            Ok::<(usize, Vec<u8>), Error>((seg.index, ts))
        }
    }))
    .buffered(4);

    let mut demux_state = TsDemuxState::default();
    while let Some(result) = tasks.next().await {
        if buffer.is_cancelled() {
            return Ok(());
        }
        let (index, ts) = result?;
        let elem = extract_audio_with_state(&ts, &mut demux_state)?;
        let slice = if index == 0 {
            strip_leading_id3(&elem)
        } else {
            elem.as_slice()
        };
        if !buffer.push(slice) {
            return Ok(());
        }
    }
    Ok(())
}

pub fn default_initial_target() -> usize {
    INITIAL_BUFFER_TARGET
}
