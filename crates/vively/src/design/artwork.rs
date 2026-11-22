use crate::design::colors::ToHsla as _;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use gpui_kit::{
    AnyElement, Div, Hsla, IntoElement, ObjectFit, ParentElement as _, Styled as _,
    StyledImage as _, div, img, linear_color_stop, linear_gradient, px, rgb,
};
use image::{ImageReader, codecs::jpeg::JpegEncoder, imageops::FilterType};
use sha1::{Digest, Sha1};
use url::Url;

use super::{
    colors::{DEFAULT_TINT, Palette, RgbColor, mix_oklab},
    icons::{Icon, icon},
};

#[derive(Debug, Clone, Copy)]
pub struct SampledTint {
    pub color: RgbColor,
    pub is_neutral: bool,
}

impl Default for SampledTint {
    fn default() -> Self {
        Self {
            color: DEFAULT_TINT,
            is_neutral: true,
        }
    }
}

pub enum CoverTint {
    Pending,
    Ready(SampledTint),
    Unavailable,
}

pub fn sample_cover_bytes(bytes: &[u8]) -> Option<SampledTint> {
    let decoded = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let small = decoded.resize_exact(2, 2, FilterType::Triangle).to_rgb8();

    let mut best_sat = -1.0_f32;
    let mut best_rgb = DEFAULT_TINT;

    for pixel in small.pixels() {
        let r = pixel[0] as f32 / 255.0;
        let g = pixel[1] as f32 / 255.0;
        let b = pixel[2] as f32 / 255.0;
        let (h, mut s, mut v) = rgb_to_hsv(r, g, b);
        let raw_sat = s;
        if s > 0.08 {
            s = s.clamp(0.35, 0.85);
        }
        v = v.clamp(0.22, 0.68);
        if raw_sat > best_sat {
            best_sat = raw_sat;
            best_rgb = hsv_to_rgb(h, s, v);
        }
    }

    if best_sat <= 0.08 {
        Some(SampledTint::default())
    } else {
        Some(SampledTint {
            color: best_rgb,
            is_neutral: false,
        })
    }
}

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let s = if max <= 1e-6 { 0.0 } else { d / max };
    if d <= 1e-6 {
        return (0.0, s, max);
    }
    let mut h = if (max - r).abs() <= 1e-6 {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() <= 1e-6 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h /= 6.0;
    (h, s, max)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> RgbColor {
    if s <= 1e-6 {
        return RgbColor {
            r: v,
            g: v,
            b: v,
            a: 1.0,
        };
    }
    let h6 = (h.rem_euclid(1.0)) * 6.0;
    let i = h6.floor() as i32;
    let f = h6 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    RgbColor { r, g, b, a: 1.0 }
}

pub fn encode_cover_jpeg(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let img = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .decode()?;
    let (w, h) = (img.width(), img.height());
    let side = w.min(h);
    let cropped = img.crop_imm((w - side) / 2, (h - side) / 2, side, side);
    let target = if side > 1200 {
        cropped.resize_exact(1200, 1200, FilterType::Lanczos3)
    } else {
        cropped
    };
    let rgb_img = target.to_rgb8();
    let mut out = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut out, 88);
    encoder.encode_image(&rgb_img)?;
    Ok(out)
}

const DISPLAY_SHORT_SIDE: u32 = 400;

static DECODE_PERMITS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

struct CacheInner {
    files: HashMap<String, PathBuf>,
    display_files: HashMap<String, PathBuf>,
    tints: HashMap<String, Option<SampledTint>>,
    in_flight: HashSet<String>,
    tint_in_flight: HashSet<String>,
    failed: HashSet<String>,
}

#[derive(Clone)]
pub struct ArtworkCache {
    dir: PathBuf,
    client: reqwest::Client,
    inner: Arc<Mutex<CacheInner>>,
    dirty: Arc<std::sync::atomic::AtomicBool>,
}

impl ArtworkCache {
    pub fn new(dir: PathBuf, client: reqwest::Client) -> Self {
        let _ = fs::create_dir_all(&dir);
        Self {
            dir,
            client,
            inner: Arc::new(Mutex::new(CacheInner {
                files: HashMap::new(),
                display_files: HashMap::new(),
                tints: HashMap::new(),
                in_flight: HashSet::new(),
                tint_in_flight: HashSet::new(),
                failed: HashSet::new(),
            })),
            dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, std::sync::atomic::Ordering::AcqRel)
    }

    fn disk_path(&self, url: &Url) -> PathBuf {
        let digest = Sha1::digest(url.as_str().as_bytes());
        let name: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        self.dir.join(format!("{name}.img"))
    }

    fn display_path(&self, url: &Url) -> PathBuf {
        let mut path = self.disk_path(url);
        path.set_extension("display");
        path
    }

    pub fn lookup_memory(&self, url: &Url) -> Option<PathBuf> {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.display_files.get(url.as_str()).cloned()
    }

    pub fn lookup_full(&self, url: &Url) -> Option<PathBuf> {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.files.get(url.as_str()).cloned()
    }

    pub fn cached_tint(&self, url: &Url) -> Option<SampledTint> {
        match self.tint_status(url) {
            CoverTint::Ready(tint) => Some(tint),
            _ => None,
        }
    }

    pub fn tint_status(&self, url: &Url) -> CoverTint {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let key = url.as_str();
        match guard.tints.get(key) {
            Some(Some(tint)) => CoverTint::Ready(*tint),
            Some(None) => CoverTint::Unavailable,
            None if guard.failed.contains(key) => CoverTint::Unavailable,
            None => CoverTint::Pending,
        }
    }

    pub fn begin_fetch(&self, url: &Url) -> bool {
        let key = url.as_str();
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if guard.files.contains_key(key)
            || guard.in_flight.contains(key)
            || guard.failed.contains(key)
        {
            return false;
        }
        guard.in_flight.insert(key.to_owned());
        true
    }

    pub fn begin_tint_sample(&self, url: &Url) -> Option<PathBuf> {
        let key = url.as_str();
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if guard.tints.contains_key(key) || guard.tint_in_flight.contains(key) {
            return None;
        }
        let path = guard.display_files.get(key)?.clone();
        guard.tint_in_flight.insert(key.to_owned());
        Some(path)
    }

    pub fn sample_file_off_thread(&self, url: Url, path: PathBuf) -> Option<SampledTint> {
        let key = url.to_string();
        let tint = fs::read(&path).ok().and_then(|b| sample_cover_bytes(&b));
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.tint_in_flight.remove(&key);
        guard.tints.insert(key, tint);
        self.dirty.store(true, std::sync::atomic::Ordering::Release);
        tint
    }

    pub async fn fetch_or_load(
        &self,
        url: Url,
        offline_candidate: Option<PathBuf>,
        compute_tint: bool,
    ) -> Option<PathBuf> {
        let key = url.to_string();
        let Some((original, bytes)) = self.load_original(&url, offline_candidate).await else {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            guard.in_flight.remove(&key);
            guard.failed.insert(key);
            self.dirty.store(true, std::sync::atomic::Ordering::Release);
            return None;
        };

        let display_dest = self.display_path(&url);
        let display = if display_dest.exists() {
            display_dest
        } else if needs_display_variant(&bytes) {
            let _permit = DECODE_PERMITS.acquire().await.ok()?;
            match tokio::task::block_in_place(|| write_display_variant(&bytes, &display_dest)) {
                Ok(true) => display_dest,
                _ => original.clone(),
            }
        } else {
            original.clone()
        };
        drop(bytes);

        let tint = if compute_tint {
            let _permit = DECODE_PERMITS.acquire().await.ok()?;
            let display_bytes = tokio::fs::read(&display).await.ok();
            tokio::task::block_in_place(|| display_bytes.and_then(|b| sample_cover_bytes(&b)))
        } else {
            None
        };

        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.in_flight.remove(&key);
        guard.files.insert(key.clone(), original);
        guard.display_files.insert(key.clone(), display.clone());
        if compute_tint {
            guard.tints.insert(key, tint);
        }
        self.dirty.store(true, std::sync::atomic::Ordering::Release);
        Some(display)
    }

    async fn load_original(
        &self,
        url: &Url,
        offline_candidate: Option<PathBuf>,
    ) -> Option<(PathBuf, Vec<u8>)> {
        if let Some(off) = offline_candidate
            && let Ok(bytes) = tokio::fs::read(&off).await
        {
            return Some((off, bytes));
        }

        let disk = self.disk_path(url);
        if let Ok(bytes) = tokio::fs::read(&disk).await {
            return Some((disk, bytes));
        }

        let resp = self.client.get(url.clone()).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let bytes = resp.bytes().await.ok()?;
        if bytes.is_empty() {
            return None;
        }
        let _ = tokio::fs::create_dir_all(&self.dir).await;
        tokio::fs::write(&disk, &bytes).await.ok()?;
        Some((disk, bytes.to_vec()))
    }
}

fn needs_display_variant(bytes: &[u8]) -> bool {
    ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.into_dimensions().ok())
        .is_some_and(|(w, h)| w.min(h) > DISPLAY_SHORT_SIDE)
}

pub fn write_display_variant(bytes: &[u8], dest: &Path) -> anyhow::Result<bool> {
    let (w, h) = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .into_dimensions()?;
    let short = w.min(h);
    if short <= DISPLAY_SHORT_SIDE {
        return Ok(false);
    }
    let img = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .decode()?;
    let scale = DISPLAY_SHORT_SIDE as f64 / short as f64;
    let nw = ((w as f64 * scale).round() as u32).max(1);
    let nh = ((h as f64 * scale).round() as u32).max(1);
    let resized = img.resize_exact(nw, nh, FilterType::CatmullRom);

    let mut out = Vec::new();
    if resized.color().has_alpha() {
        resized.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)?;
    } else {
        JpegEncoder::new_with_quality(&mut out, 90).encode_image(&resized.to_rgb8())?;
    }
    fs::write(dest, out)?;
    Ok(true)
}

fn placeholder_surface(pal: &Palette) -> (Hsla, Hsla) {
    if pal.is_dark {
        (
            mix_oklab(pal.tint, 10.0, RgbColor::hex(0x2B2C30)).to_hsla(),
            mix_oklab(pal.tint, 8.0, RgbColor::hex(0x1B1C1F)).to_hsla(),
        )
    } else {
        (
            mix_oklab(pal.tint, 5.0, RgbColor::hex(0xEAEBED)).to_hsla(),
            mix_oklab(pal.tint, 8.0, RgbColor::hex(0xD5D7DD)).to_hsla(),
        )
    }
}

pub fn render_cover(local_path: Option<PathBuf>, size_px: f32, round: bool, pal: &Palette) -> Div {
    let radius = if round { px(9999.0) } else { px(3.0) };
    let inner: AnyElement = match local_path {
        Some(path) => img(path)
            .size_full()
            .object_fit(ObjectFit::Cover)
            .rounded(radius)
            .into_any_element(),
        None => {
            let (top, bottom) = placeholder_surface(pal);
            let emblem = if round { Icon::Mic } else { Icon::Logo };
            let emblem_px = (size_px * 0.48).min(size_px * 0.36 + 10.0);
            let emblem_color = pal.ink.opacity(if size_px < 64.0 { 0.45 } else { 0.42 });
            div()
                .size_full()
                .rounded(radius)
                .bg(linear_gradient(
                    145.0,
                    linear_color_stop(top, 0.0),
                    linear_color_stop(bottom, 1.0),
                ))
                .flex()
                .items_center()
                .justify_center()
                .child(icon(emblem, emblem_px, emblem_color))
                .into_any_element()
        }
    };

    div()
        .relative()
        .size(px(size_px))
        .rounded(radius)
        .overflow_hidden()
        .flex_shrink_0()
        .border_1()
        .border_color(pal.art_ring)
        .child(inner)
}

pub fn render_liked_cover(size_px: f32, ring: gpui_kit::Hsla) -> Div {
    div()
        .relative()
        .size(px(size_px))
        .rounded(px(3.0))
        .overflow_hidden()
        .flex_shrink_0()
        .bg(rgb(0x2A2B2E))
        .border_1()
        .border_color(ring)
        .flex()
        .items_center()
        .justify_center()
        .child(icon(Icon::HeartFill, size_px * 0.48, rgb(0xECEAE6).into()))
}

pub fn render_avatar(
    local_path: Option<PathBuf>,
    seed_id: i64,
    initials: &str,
    size_px: f32,
    ring: gpui_kit::Hsla,
) -> Div {
    const BGS: [u32; 6] = [0x2A2A2A, 0x1F5F4A, 0x6E2E35, 0x354F52, 0x524638, 0x3B3B3B];
    let bg = BGS[(seed_id.unsigned_abs() as usize) % BGS.len()];

    let mut base = div()
        .relative()
        .size(px(size_px))
        .rounded_full()
        .overflow_hidden()
        .flex_shrink_0()
        .border_1()
        .border_color(ring)
        .bg(rgb(bg))
        .flex()
        .items_center()
        .justify_center();

    if let Some(path) = local_path {
        base = base.child(
            img(path)
                .size_full()
                .object_fit(ObjectFit::Cover)
                .rounded_full(),
        );
    } else {
        base = base.child(
            div()
                .text_size(px((size_px * 0.36).max(11.0)))
                .font_weight(gpui_kit::FontWeight::BOLD)
                .text_color(rgb(0xF5F5F5))
                .child(initials.to_owned()),
        );
    }
    base
}

pub fn make_initials(first: &str, last: &str) -> String {
    let mut out = String::new();
    if let Some(c) = first.chars().next() {
        for u in c.to_uppercase() {
            out.push(u);
        }
    }
    if let Some(c) = last.chars().next() {
        for u in c.to_uppercase() {
            out.push(u);
        }
    }
    if out.is_empty() { "VK".into() } else { out }
}

pub fn validate_local_image(path: &Path) -> bool {
    path.exists()
}
