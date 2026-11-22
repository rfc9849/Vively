use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};

use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
use futures_util::{StreamExt, TryStreamExt, stream};
use url::Url;

use crate::{Error, Result, config::USER_AGENT, session::atomic_write};

#[derive(Debug, Clone, PartialEq)]
pub struct HlsSegment {
    pub index: usize,
    pub url: Url,
    pub key_url: Option<Url>,
    pub iv: [u8; 16],
}

pub fn sequence_iv(sequence: u64) -> [u8; 16] {
    let mut iv = [0; 16];
    iv[8..].copy_from_slice(&sequence.to_be_bytes());
    iv
}

pub fn parse_playlist(text: &str, base: &Url) -> Result<Vec<HlsSegment>> {
    if !text
        .trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with("#EXTM3U")
    {
        return Err(Error::Media("Invalid HLS playlist".into()));
    }
    let mut segments = Vec::new();
    let mut key_url = None;
    let mut explicit_iv = None;
    let mut sequence = 0u64;
    for line in text.lines().map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(value) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            sequence = value
                .parse()
                .map_err(|_| Error::Media("Invalid HLS media sequence".into()))?;
        } else if let Some(value) = line.strip_prefix("#EXT-X-KEY:") {
            let attrs = attributes(value)?;
            match attrs.get("METHOD").map(String::as_str) {
                Some("NONE") => {
                    key_url = None;
                    explicit_iv = None;
                }
                Some("AES-128") => {
                    let uri = attrs
                        .get("URI")
                        .ok_or_else(|| Error::Media("HLS key URI is missing".into()))?;
                    key_url = Some(base.join(uri)?);
                    explicit_iv = attrs.get("IV").map(|v| parse_iv(v)).transpose()?;
                }
                _ => return Err(Error::Media("Unsupported HLS encryption method".into())),
            }
        } else if line.starts_with("#EXT-X-STREAM-INF:")
            || line.starts_with("#EXT-X-MAP:")
            || line.starts_with("#EXT-X-BYTERANGE:")
        {
            return Err(Error::Media(
                "Expected an MPEG-TS HLS media playlist".into(),
            ));
        } else if !line.starts_with('#') {
            let index = segments.len();
            let n = sequence
                .checked_add(index as u64)
                .ok_or_else(|| Error::Media("HLS media sequence overflow".into()))?;
            segments.push(HlsSegment {
                index,
                url: base.join(line)?,
                key_url: key_url.clone(),
                iv: explicit_iv.unwrap_or_else(|| sequence_iv(n)),
            });
        }
    }
    if segments.is_empty() {
        return Err(Error::Media("HLS playlist has no segments".into()));
    }
    Ok(segments)
}

fn attributes(text: &str) -> Result<HashMap<String, String>> {
    let mut result = HashMap::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let (key, value) = rest
            .split_once('=')
            .ok_or_else(|| Error::Media("Invalid HLS key attributes".into()))?;
        if let Some(quoted) = value.strip_prefix('"') {
            let (value, remaining) = quoted
                .split_once('"')
                .ok_or_else(|| Error::Media("Invalid HLS key URI".into()))?;
            result.insert(key.trim().into(), value.into());
            rest = remaining.trim_start_matches(',').trim();
        } else {
            let (value, remaining) = value.split_once(',').unwrap_or((value, ""));
            result.insert(key.trim().into(), value.trim().into());
            rest = remaining.trim();
        }
    }
    Ok(result)
}

fn parse_iv(value: &str) -> Result<[u8; 16]> {
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    if value.is_empty() || value.len() > 32 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Media("Invalid HLS initialization vector".into()));
    }
    let padded = format!("{value:0>32}");
    let mut result = [0; 16];
    for (i, byte) in result.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&padded[i * 2..i * 2 + 2], 16)
            .map_err(|_| Error::Media("Invalid HLS initialization vector".into()))?;
    }
    Ok(result)
}

pub fn decrypt_aes128_cbc(data: &[u8], key: &[u8], iv: &[u8; 16]) -> Result<Vec<u8>> {
    let cipher = cbc::Decryptor::<aes::Aes128>::new_from_slices(key, iv)
        .map_err(|_| Error::Media("Invalid HLS encryption key".into()))?;
    cipher
        .decrypt_padded_vec_mut::<Pkcs7>(data)
        .map_err(|_| Error::Media("Could not decrypt HLS segment".into()))
}

#[derive(Clone)]
pub struct Mp3StreamDownloader {
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct Mp3Tag {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub artwork: Option<Vec<u8>>,
}

impl Mp3StreamDownloader {
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .connect_timeout(Duration::from_secs(30))
                .timeout(Duration::from_secs(300))
                .build()?,
        })
    }

    pub async fn export_track(
        &self,
        track: &crate::models::Audio,
        directory: &Path,
    ) -> Result<PathBuf> {
        let url = track
            .stream_url()
            .ok_or_else(|| Error::Media("Track has no stream URL".into()))?;
        let artwork = match track.artwork_url() {
            Some(url) => self.fetch_data(url.clone()).await.ok(),
            None => None,
        };
        let name: String = format!("{} - {}", track.artist, track.title)
            .chars()
            .map(|c| {
                if c.is_control() || "/\\\\:*?\\\"<>|".contains(c) {
                    '_'
                } else {
                    c
                }
            })
            .take(180)
            .collect();
        let raw = self.mp3_data(url, &|_| {}).await?;
        if is_adts_aac(&raw) {
            let path = directory.join(format!("{name}.aac"));
            atomic_write(&path, &raw)?;
            Ok(path)
        } else {
            let mut tagged = build_id3(
                &track.title,
                &track.artist,
                track.album.as_ref().and_then(|a| a.title.as_deref()),
                artwork.as_deref(),
            )?;
            tagged.extend_from_slice(&raw);
            let path = directory.join(format!("{name}.mp3"));
            atomic_write(&path, &tagged)?;
            Ok(path)
        }
    }

    pub async fn fetch_data(&self, url: Url) -> Result<Vec<u8>> {
        let response = self.client.get(url).send().await?;
        if !response.status().is_success() {
            return Err(Error::Http(response.status().as_u16()));
        }
        Ok(response.bytes().await?.to_vec())
    }

    pub async fn mp3_data(
        &self,
        url: Url,
        progress: &(dyn Fn(f64) + Send + Sync),
    ) -> Result<Vec<u8>> {
        let bytes = self.fetch_data(url.clone()).await?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| Error::Media("Invalid HLS playlist encoding".into()))?;
        let segments = parse_playlist(text, &url)?;
        let mut keys = HashMap::new();
        for segment in &segments {
            if let Some(url) = &segment.key_url
                && !keys.contains_key(url)
            {
                let key = self.fetch_data(url.clone()).await?;
                if key.len() != 16 {
                    return Err(Error::Media("Invalid HLS encryption key".into()));
                }
                keys.insert(url.clone(), key);
            }
        }
        let count = segments.len();
        let mut chunks: Vec<Option<Vec<u8>>> = vec![None; count];
        let mut tasks = stream::iter(segments.into_iter().map(|segment| {
            let keys = &keys;
            async move {
                let data = self.fetch_data(segment.url).await?;
                let data = if let Some(url) = segment.key_url {
                    decrypt_aes128_cbc(&data, &keys[&url], &segment.iv)?
                } else {
                    data
                };
                Ok::<_, Error>((segment.index, data))
            }
        }))
        .buffer_unordered(6);
        let mut completed = 0;
        while let Some((index, data)) = tasks.try_next().await? {
            chunks[index] = Some(data);
            completed += 1;
            progress(completed as f64 / count as f64);
        }
        let ts: Vec<u8> = chunks.into_iter().flatten().flatten().collect();
        Ok(strip_leading_id3(&extract_mp3(&ts)?).to_vec())
    }

    pub async fn download(
        &self,
        url: Url,
        destination: &Path,
        tag: &Mp3Tag,
        progress: &(dyn Fn(f64) + Send + Sync),
    ) -> Result<u64> {
        let mp3 = self.mp3_data(url, progress).await?;
        let mut bytes = build_id3(
            &tag.title,
            &tag.artist,
            tag.album.as_deref(),
            tag.artwork.as_deref(),
        )?;
        bytes.extend(mp3);
        atomic_write(destination, &bytes)?;
        Ok(bytes.len() as u64)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TsDemuxState {
    pub pmt_pid: Option<u16>,
    pub audio_pid: Option<u16>,
}

pub fn is_adts_aac(data: &[u8]) -> bool {
    let stripped = strip_leading_id3(data);
    stripped.len() >= 2 && stripped[0] == 0xff && (stripped[1] & 0xf6) == 0xf0
}

pub fn extract_mp3(ts: &[u8]) -> Result<Vec<u8>> {
    extract_audio_with_state(ts, &mut TsDemuxState::default())
}

pub fn extract_audio_with_state(ts: &[u8], state: &mut TsDemuxState) -> Result<Vec<u8>> {
    let start = (0..ts.len().min(188))
        .find(|i| ts[*i] == 0x47 && (*i + 188 >= ts.len() || ts[*i + 188] == 0x47))
        .ok_or_else(|| Error::Media("MPEG-TS sync byte was not found".into()))?;
    let mut output = Vec::new();
    for packet in ts[start..].as_chunks::<188>().0 {
        if packet[0] != 0x47 || packet[1] & 0x80 != 0 {
            continue;
        }
        let unit_start = packet[1] & 0x40 != 0;
        let pid = ((packet[1] as u16 & 0x1f) << 8) | packet[2] as u16;
        let adaptation = (packet[3] >> 4) & 3;
        let payload = match adaptation {
            1 => 4,
            3 => 5 + packet[4] as usize,
            _ => continue,
        };
        let Some(data) = packet.get(payload..).filter(|p| !p.is_empty()) else {
            continue;
        };
        if pid == 0 {
            if let Some(pmt) = psi_section(data, unit_start).and_then(parse_pat) {
                state.pmt_pid = Some(pmt);
            }
        } else if Some(pid) == state.pmt_pid {
            if let Some(aud) = psi_section(data, unit_start).and_then(parse_pmt) {
                state.audio_pid = Some(aud);
            }
        } else if Some(pid) == state.audio_pid {
            let start = if unit_start && data.len() >= 9 && data[..3] == [0, 0, 1] {
                9 + data[8] as usize
            } else {
                0
            };
            if let Some(bytes) = data.get(start..) {
                output.extend_from_slice(bytes);
            }
        }
    }
    if output.is_empty() {
        return Err(Error::Media("Audio stream was not found in MPEG-TS".into()));
    }
    Ok(output)
}

fn psi_section(data: &[u8], unit_start: bool) -> Option<&[u8]> {
    let data = if unit_start {
        data.get(1 + *data.first()? as usize..)?
    } else {
        data
    };
    let length = (((*data.get(1)? as usize) & 15) << 8) | *data.get(2)? as usize;
    let end = (3usize.checked_add(length)?.checked_sub(4)?).min(data.len());
    data.get(..end)
}

fn parse_pat(data: &[u8]) -> Option<u16> {
    for entry in data.get(8..)?.as_chunks::<4>().0 {
        if entry[0] != 0 || entry[1] != 0 {
            return Some(((entry[2] as u16 & 31) << 8) | entry[3] as u16);
        }
    }
    None
}

fn parse_pmt(data: &[u8]) -> Option<u16> {
    let info_len = (((*data.get(10)? as usize) & 15) << 8) | *data.get(11)? as usize;
    let mut offset = 12 + info_len;
    while let Some(entry) = data.get(offset..offset.checked_add(5)?) {
        let pid = ((entry[1] as u16 & 31) << 8) | entry[2] as u16;
        if matches!(entry[0], 0x03 | 0x04 | 0x0f | 0x11) {
            return Some(pid);
        }
        offset += 5 + (((entry[3] as usize) & 15) << 8) + entry[4] as usize;
    }
    None
}

fn synchsafe(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 4 || bytes[..4].iter().any(|b| b & 0x80 != 0) {
        return None;
    }
    Some(bytes[..4].iter().fold(0, |v, b| (v << 7) | *b as usize))
}

fn encode_synchsafe(value: usize) -> Result<[u8; 4]> {
    if value >= 1 << 28 {
        return Err(Error::Media("ID3 tag is too large".into()));
    }
    Ok([
        ((value >> 21) & 127) as u8,
        ((value >> 14) & 127) as u8,
        ((value >> 7) & 127) as u8,
        (value & 127) as u8,
    ])
}

pub fn strip_leading_id3(data: &[u8]) -> &[u8] {
    if data.len() < 10 || &data[..3] != b"ID3" {
        return data;
    }
    let Some(size) = synchsafe(&data[6..10]) else {
        return data;
    };
    let footer = if data[3] == 4 && data[5] & 16 != 0 {
        10
    } else {
        0
    };
    data.get(10 + size + footer..).unwrap_or(data)
}

pub fn build_id3(
    title: &str,
    artist: &str,
    album: Option<&str>,
    artwork: Option<&[u8]>,
) -> Result<Vec<u8>> {
    fn frame(id: &[u8; 4], body: &[u8], output: &mut Vec<u8>) -> Result<()> {
        output.extend_from_slice(id);
        output.extend_from_slice(&encode_synchsafe(body.len())?);
        output.extend_from_slice(&[0, 0]);
        output.extend_from_slice(body);
        Ok(())
    }
    let mut frames = Vec::new();
    for (id, text) in [
        (b"TIT2", Some(title)),
        (b"TPE1", Some(artist)),
        (b"TALB", album.filter(|a| !a.is_empty())),
    ] {
        if let Some(text) = text {
            let mut body = vec![3];
            body.extend_from_slice(text.as_bytes());
            frame(id, &body, &mut frames)?;
        }
    }
    if let Some(artwork) = artwork.filter(|a| !a.is_empty()) {
        let mut body = b"\x03image/jpeg\0\x03\0".to_vec();
        body.extend_from_slice(artwork);
        frame(b"APIC", &body, &mut frames)?;
    }
    let mut tag = b"ID3\x04\0\0".to_vec();
    tag.extend_from_slice(&encode_synchsafe(frames.len())?);
    tag.extend(frames);
    Ok(tag)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Id3Tags {
    pub artist: Option<String>,
    pub title: Option<String>,
}

pub fn read_id3(data: &[u8]) -> Id3Tags {
    let mut tags = Id3Tags::default();
    if data.len() < 10 || &data[..3] != b"ID3" || !(2..=4).contains(&data[3]) || data[5] & 128 != 0
    {
        return tags;
    }
    let version = data[3];
    let Some(tag_size) = synchsafe(&data[6..10]) else {
        return tags;
    };
    let end = data.len().min(10 + tag_size);
    let mut offset = 10;
    if version >= 3 && data[5] & 64 != 0 {
        let Some(size) = data.get(offset..offset + 4) else {
            return tags;
        };
        let length = if version == 4 {
            synchsafe(size).unwrap_or(0)
        } else {
            plain_int(size) + 4
        };
        if length < 4 {
            return tags;
        }
        offset += length;
    }
    let (id_len, header_len) = if version == 2 { (3, 6) } else { (4, 10) };
    while offset + header_len <= end && (tags.artist.is_none() || tags.title.is_none()) {
        let id = &data[offset..offset + id_len];
        if id[0] == 0 {
            break;
        }
        let size = match version {
            2 => plain_int(&data[offset + 3..offset + 6]),
            3 => plain_int(&data[offset + 4..offset + 8]),
            _ => match synchsafe(&data[offset + 4..offset + 8]) {
                Some(size) => size,
                None => break,
            },
        };
        let body_start = offset + header_len;
        if size == 0 || size > end - body_start {
            break;
        }
        let body = &data[body_start..body_start + size];
        if id == b"TIT2" || id == b"TT2" {
            tags.title = decode_text(body);
        }
        if id == b"TPE1" || id == b"TP1" {
            tags.artist = decode_text(body);
        }
        offset = body_start + size;
    }
    tags
}

fn plain_int(data: &[u8]) -> usize {
    data.iter().fold(0, |v, b| (v << 8) | *b as usize)
}

fn decode_text(body: &[u8]) -> Option<String> {
    let (&encoding, text) = body.split_first()?;
    let decoded = match encoding {
        0 => text.iter().map(|b| *b as char).collect(),
        3 => std::str::from_utf8(text).ok()?.into(),
        1 | 2 => {
            let (little, text) = if encoding == 1 && text.starts_with(&[255, 254]) {
                (true, &text[2..])
            } else if encoding == 1 && text.starts_with(&[254, 255]) {
                (false, &text[2..])
            } else {
                (false, text)
            };
            let words: Vec<_> = text
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| {
                    if little {
                        u16::from_le_bytes([b[0], b[1]])
                    } else {
                        u16::from_be_bytes([b[0], b[1]])
                    }
                })
                .collect();
            String::from_utf16(&words).ok()?
        }
        _ => return None,
    };
    let text = decoded.trim_matches('\0').trim();
    (!text.is_empty()).then(|| text.to_owned())
}
