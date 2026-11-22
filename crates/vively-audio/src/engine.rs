use std::{
    fs::File,
    io::BufReader,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use rodio::{
    Decoder, DeviceSinkBuilder, MixerDeviceSink, Player,
    cpal::{self, BufferSize, traits::HostTrait as _},
};
use url::Url;
use vively_core::{Error, Result};

use crate::stream_buffer::{
    StreamingAudioBuffer, default_initial_target, make_stream_http_client, stream_url_into_buffer,
};

#[derive(Debug, Clone)]
pub enum TrackSource {
    LocalFile(PathBuf),
    RemoteUrl(Url),
}

pub struct PreparedTrack {
    pub track_id: String,
    pub duration_hint: f64,
    buffer: Option<Arc<StreamingAudioBuffer>>,
    kind: PreparedKind,
}

enum PreparedKind {
    File(PathBuf),
    Stream(Arc<StreamingAudioBuffer>),
}

impl Drop for PreparedTrack {
    fn drop(&mut self) {
        if let Some(buf) = &self.buffer {
            buf.cancel();
        }
    }
}

pub async fn prepare_track(
    rt_handle: &tokio::runtime::Handle,
    client: &reqwest::Client,
    track_id: String,
    duration_hint: f64,
    source: TrackSource,
) -> Result<PreparedTrack> {
    match source {
        TrackSource::LocalFile(path) => {
            if !path.exists() {
                return Err(Error::Media(format!(
                    "Audio file not found: {}",
                    path.display()
                )));
            }
            Ok(PreparedTrack {
                track_id,
                duration_hint,
                buffer: None,
                kind: PreparedKind::File(path),
            })
        }
        TrackSource::RemoteUrl(url) => {
            let buffer = StreamingAudioBuffer::new();
            let writer = Arc::clone(&buffer);
            let client = client.clone();
            rt_handle.spawn(async move {
                stream_url_into_buffer(client, url, writer).await;
            });

            let waiter = Arc::clone(&buffer);
            tokio::task::spawn_blocking(move || {
                waiter.wait_until_playable(default_initial_target(), Duration::from_secs(25))
            })
            .await
            .map_err(|e| Error::Media(format!("Stream join error: {e}")))??;

            Ok(PreparedTrack {
                track_id,
                duration_hint,
                buffer: Some(Arc::clone(&buffer)),
                kind: PreparedKind::Stream(buffer),
            })
        }
    }
}

pub fn effective_crossfade_secs(configured: f64, track_duration: f64) -> f64 {
    if !configured.is_finite() || configured <= 0.0 || track_duration <= 4.0 {
        return 0.0;
    }
    configured.min(12.0).min((track_duration * 0.4).max(0.0))
}

pub fn linear_crossfade_gains(progress: f64, master_volume: f32) -> (f32, f32) {
    let t = progress.clamp(0.0, 1.0) as f32;
    let vol = master_volume.clamp(0.0, 1.0);
    (vol * (1.0 - t), vol * t)
}

struct PlaybackClock {
    position: f64,
    anchored_at: Instant,
    rate: f64,
    ceiling: f64,
}

impl PlaybackClock {
    fn new(position: f64, now: Instant, playing: bool) -> Self {
        Self {
            position,
            anchored_at: now,
            rate: if playing { 1.0 } else { 0.0 },
            ceiling: position + 0.35,
        }
    }

    fn position_at(&self, now: Instant) -> f64 {
        let elapsed = now
            .saturating_duration_since(self.anchored_at)
            .as_secs_f64();
        (self.position + elapsed * self.rate).min(self.ceiling)
    }

    fn synchronize(&mut self, audio_position: f64, now: Instant) {
        self.position = self.position_at(now);
        self.anchored_at = now;
        self.rate = 1.0 + ((audio_position - self.position) * 2.0).clamp(-0.12, 0.12);
        self.ceiling = (audio_position + 0.35).max(self.position);
    }
}

struct DeckSlot {
    track_id: String,
    index: Option<usize>,
    duration: f64,
    player: Option<Player>,
    buffer: Option<Arc<StreamingAudioBuffer>>,
    sim_pos: f64,
    sim_last_tick: Option<Instant>,
    playback_clock: PlaybackClock,
}

impl DeckSlot {
    fn cancel_buffer(&mut self) {
        if let Some(buf) = self.buffer.take() {
            buf.cancel();
        }
    }

    fn position(&self) -> f64 {
        if let Some(player) = &self.player {
            player.get_pos().as_secs_f64()
        } else {
            let elapsed = self
                .sim_last_tick
                .map_or(0.0, |start| start.elapsed().as_secs_f64());
            let pos = self.sim_pos + elapsed;
            if self.duration > 0.0 {
                pos.min(self.duration)
            } else {
                pos
            }
        }
    }

    fn is_finished(&self) -> bool {
        if let Some(player) = &self.player {
            player.empty()
        } else {
            self.duration > 0.0 && self.position() >= self.duration
        }
    }

    fn set_volume(&self, vol: f32) {
        if let Some(player) = &self.player {
            player.set_volume(vol);
        }
    }

    fn play(&mut self) {
        let now = Instant::now();
        if let Some(player) = &self.player {
            player.play();
        } else if self.sim_last_tick.is_none() {
            self.sim_last_tick = Some(now);
        }
        self.playback_clock = PlaybackClock::new(self.playback_clock.position_at(now), now, true);
    }

    fn pause(&mut self) {
        let now = Instant::now();
        if let Some(player) = &self.player {
            player.pause();
        } else {
            self.sim_pos = self.position();
            self.sim_last_tick = None;
        }
        self.playback_clock = PlaybackClock::new(self.playback_clock.position_at(now), now, false);
    }

    fn seek(&mut self, secs: f64) -> bool {
        let clamped = if self.duration > 0.0 {
            secs.clamp(0.0, self.duration)
        } else {
            secs.max(0.0)
        };
        let playing = self.playback_clock.rate > 0.0;
        if let Some(player) = &self.player {
            if player.try_seek(Duration::from_secs_f64(clamped)).is_err() {
                return false;
            }
        } else {
            self.sim_pos = clamped;
            if self.sim_last_tick.is_some() {
                self.sim_last_tick = Some(Instant::now());
            }
        }
        self.playback_clock = PlaybackClock::new(clamped, Instant::now(), playing);
        true
    }
}

impl Drop for DeckSlot {
    fn drop(&mut self) {
        self.cancel_buffer();
        if let Some(player) = &self.player {
            player.stop();
        }
    }
}

struct ActiveCrossfade {
    started_at: Instant,
    duration: f64,
    outgoing: DeckSlot,
}

/// The device went away or the stream can't continue. Underruns (`Xrun`)
/// and automatic reroutes (`DeviceChanged`) keep the stream usable.
fn is_fatal_stream_error(err: &cpal::Error) -> bool {
    matches!(
        err.kind(),
        cpal::ErrorKind::DeviceNotAvailable | cpal::ErrorKind::StreamInvalidated
    )
}

fn open_hardware_sink(stream_failed: Arc<AtomicBool>) -> Option<MixerDeviceSink> {
    stream_failed.store(false, Ordering::Release);

    if let Ok(builder) = DeviceSinkBuilder::from_default_device() {
        let flag = Arc::clone(&stream_failed);
        if let Ok(mut sink) = builder
            .with_buffer_size(BufferSize::Default)
            .with_error_callback(move |err: cpal::Error| {
                if is_fatal_stream_error(&err) {
                    flag.store(true, Ordering::Release);
                }
            })
            .open_stream()
        {
            sink.log_on_drop(false);
            return Some(sink);
        }
    }

    if let Ok(mut sink) = DeviceSinkBuilder::open_default_sink() {
        sink.log_on_drop(false);
        return Some(sink);
    }

    if let Ok(devices) = cpal::default_host().output_devices() {
        for dev in devices {
            if let Ok(builder) = DeviceSinkBuilder::from_device(dev) {
                let flag = Arc::clone(&stream_failed);
                if let Ok(mut sink) = builder
                    .with_buffer_size(BufferSize::Default)
                    .with_error_callback(move |err: cpal::Error| {
                        if is_fatal_stream_error(&err) {
                            flag.store(true, Ordering::Release);
                        }
                    })
                    .open_stream()
                {
                    sink.log_on_drop(false);
                    return Some(sink);
                }
            }
        }
    }

    None
}

pub struct DualAudioEngine {
    sink: Option<MixerDeviceSink>,
    stream_failed: Arc<AtomicBool>,
    simulated: bool,
    http_client: reqwest::Client,
    active: Option<DeckSlot>,
    standby: Option<DeckSlot>,
    crossfade: Option<ActiveCrossfade>,
    playing: bool,
    volume: f32,
    muted: bool,
}

impl DualAudioEngine {
    pub fn new() -> Result<Self> {
        let stream_failed = Arc::new(AtomicBool::new(false));
        let sink = open_hardware_sink(Arc::clone(&stream_failed));
        let http_client = make_stream_http_client()?;
        Ok(Self {
            sink,
            stream_failed,
            simulated: false,
            http_client,
            active: None,
            standby: None,
            crossfade: None,
            playing: false,
            volume: 1.0,
            muted: false,
        })
    }

    pub fn new_simulated() -> Result<Self> {
        let http_client = make_stream_http_client()?;
        Ok(Self {
            sink: None,
            stream_failed: Arc::new(AtomicBool::new(false)),
            simulated: true,
            http_client,
            active: None,
            standby: None,
            crossfade: None,
            playing: false,
            volume: 1.0,
            muted: false,
        })
    }

    fn ensure_hardware_sink(&mut self) -> Result<()> {
        if self.simulated {
            return Ok(());
        }
        if self.sink.is_none() || self.stream_failed.load(Ordering::Acquire) {
            self.standby = None;
            self.crossfade = None;
            self.sink = open_hardware_sink(Arc::clone(&self.stream_failed));
        }
        if self.sink.is_none() {
            return Err(Error::Media(
                "Не удалось открыть аудиоустройство вывода".into(),
            ));
        }
        Ok(())
    }

    pub fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }

    pub fn has_hardware_output(&self) -> bool {
        self.sink.is_some()
    }

    pub fn effective_volume(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.volume.clamp(0.0, 1.0)
        }
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn is_crossfading(&self) -> bool {
        self.crossfade.is_some()
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = if volume.is_finite() {
            volume.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if self.volume > 0.0 && self.muted {
            self.muted = false;
        }
        self.sync_volumes();
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        self.sync_volumes();
    }

    pub fn toggle_mute(&mut self) -> bool {
        self.muted = !self.muted;
        self.sync_volumes();
        self.muted
    }

    fn sync_volumes(&mut self) {
        let master = self.effective_volume();
        if let Some(xfade) = &self.crossfade {
            let progress = if xfade.duration > 0.0 {
                xfade.started_at.elapsed().as_secs_f64() / xfade.duration
            } else {
                1.0
            };
            let (out_gain, in_gain) = linear_crossfade_gains(progress, master);
            xfade.outgoing.set_volume(out_gain);
            if let Some(active) = &self.active {
                active.set_volume(in_gain);
            }
        } else if let Some(active) = &self.active {
            active.set_volume(master);
        }
    }

    fn build_slot(
        &self,
        mut prepared: PreparedTrack,
        queue_index: Option<usize>,
        initial_volume: f32,
        paused: bool,
        start_secs: f64,
    ) -> Result<DeckSlot> {
        let buffer = prepared.buffer.take();
        let mut initial_position = start_secs.max(0.0);
        let player = if let Some(sink) = &self.sink {
            let p = Player::connect_new(sink.mixer());
            p.set_volume(initial_volume);
            if paused {
                p.pause();
            }
            match &prepared.kind {
                PreparedKind::File(path) => {
                    let file = File::open(path)?;
                    let decoder = Decoder::try_from(file)
                        .map_err(|e| Error::Media(format!("Audio decode error: {e}")))?;
                    p.append(decoder);
                }
                PreparedKind::Stream(stream_buf) => {
                    let reader = BufReader::new(stream_buf.reader());
                    let mut builder = Decoder::builder()
                        .with_data(reader)
                        .with_gapless(true)
                        .with_seekable(true);
                    if let Some(ext) = stream_buf.format_hint() {
                        builder = builder.with_hint(ext);
                    }
                    if let Some(len) = stream_buf.complete_len() {
                        builder = builder.with_byte_len(len);
                    }
                    let decoder = builder
                        .build()
                        .map_err(|e| Error::Media(format!("Stream decode error: {e}")))?;
                    p.append(decoder);
                }
            }
            if start_secs <= 0.05 || p.try_seek(Duration::from_secs_f64(start_secs)).is_err() {
                initial_position = 0.0;
            }
            if !paused {
                p.play();
            }
            Some(p)
        } else {
            None
        };

        let now = Instant::now();
        Ok(DeckSlot {
            track_id: prepared.track_id.clone(),
            index: queue_index,
            duration: prepared.duration_hint.max(0.0),
            player,
            buffer,
            sim_pos: initial_position,
            sim_last_tick: (!paused).then_some(now),
            playback_clock: PlaybackClock::new(initial_position, now, !paused),
        })
    }

    pub fn load_active(
        &mut self,
        prepared: PreparedTrack,
        queue_index: Option<usize>,
        autoplay: bool,
        start_secs: f64,
    ) -> Result<()> {
        self.ensure_hardware_sink()?;
        self.crossfade = None;
        self.standby = None;
        self.active = None;

        let vol = self.effective_volume();
        let slot = self.build_slot(prepared, queue_index, vol, !autoplay, start_secs)?;
        self.active = Some(slot);
        self.playing = autoplay;
        Ok(())
    }

    pub fn preload_standby(&mut self, prepared: PreparedTrack, queue_index: usize) -> Result<()> {
        if self.crossfade.is_some() {
            return Ok(());
        }
        let slot = self.build_slot(prepared, Some(queue_index), 0.0, true, 0.0)?;
        self.standby = Some(slot);
        Ok(())
    }

    pub fn has_valid_standby(&self, index: usize, stable_id: &str) -> bool {
        self.standby
            .as_ref()
            .is_some_and(|s| s.index == Some(index) && s.track_id == stable_id)
    }

    pub fn clear_standby(&mut self) {
        self.standby = None;
    }

    pub fn promote_standby(&mut self, crossfade_secs: f64) -> Option<usize> {
        let mut next = self.standby.take()?;
        let promoted_index = next.index;
        let fade = effective_crossfade_secs(
            crossfade_secs,
            self.active.as_ref().map(|a| a.duration).unwrap_or(0.0),
        );

        if fade > 0.0
            && self.playing
            && let Some(outgoing) = self.active.take()
        {
            next.set_volume(0.0);
            next.play();
            self.active = Some(next);
            self.crossfade = Some(ActiveCrossfade {
                started_at: Instant::now(),
                duration: fade,
                outgoing,
            });
        } else {
            self.crossfade = None;
            next.set_volume(self.effective_volume());
            if self.playing {
                next.play();
            }
            self.active = Some(next);
        }
        promoted_index
    }

    pub fn play(&mut self) {
        self.playing = true;
        if let Some(active) = &mut self.active {
            active.play();
        }
        if let Some(xfade) = &mut self.crossfade {
            xfade.outgoing.play();
        }
    }

    pub fn pause(&mut self) {
        self.playing = false;
        if let Some(active) = &mut self.active {
            active.pause();
        }
        if let Some(xfade) = &mut self.crossfade {
            xfade.outgoing.pause();
        }
    }

    pub fn stop_and_clear(&mut self) {
        self.playing = false;
        self.crossfade = None;
        self.standby = None;
        self.active = None;
    }

    pub fn seek(&mut self, secs: f64) -> bool {
        if self.crossfade.is_some() {
            self.crossfade = None;
            self.sync_volumes();
        }
        self.active.as_mut().is_some_and(|active| active.seek(secs))
    }

    pub fn position(&self) -> f64 {
        self.active.as_ref().map_or(0.0, DeckSlot::position)
    }

    pub fn progress_position(&self) -> f64 {
        self.active
            .as_ref()
            .map_or(0.0, |deck| deck.playback_clock.position_at(Instant::now()))
    }

    pub fn active_track_id(&self) -> Option<&str> {
        self.active.as_ref().map(|a| a.track_id.as_str())
    }

    pub fn has_active(&self) -> bool {
        self.active.is_some()
    }

    /// Test hook: behave as if the output device was lost.
    #[doc(hidden)]
    pub fn inject_output_failure(&self) {
        self.stream_failed.store(true, Ordering::Release);
    }

    pub fn tick(&mut self, configured_crossfade: f64) -> EngineTickOutcome {
        if self.active.is_some() && self.stream_failed.load(Ordering::Acquire) {
            let position = self.position();
            let was_playing = self.playing;
            self.crossfade = None;
            self.standby = None;
            self.active = None;
            self.sink = None;
            self.playing = false;
            self.stream_failed.store(false, Ordering::Release);
            return EngineTickOutcome::OutputLost {
                position,
                was_playing,
            };
        }
        if let Some(xfade) = &self.crossfade {
            let elapsed = xfade.started_at.elapsed().as_secs_f64();
            if elapsed >= xfade.duration {
                self.crossfade = None;
                self.sync_volumes();
            } else {
                self.sync_volumes();
            }
        }

        if !self.playing {
            return EngineTickOutcome::Idle;
        }

        if self.crossfade.is_none()
            && configured_crossfade > 0.0
            && self.standby.is_some()
            && let Some(active) = &self.active
            && active.duration > 4.0
        {
            let pos = active.position();
            let fade = effective_crossfade_secs(configured_crossfade, active.duration);
            let remaining = (active.duration - pos).max(0.0);
            if fade > 0.0
                && remaining <= fade
                && let Some(index) = self.promote_standby(configured_crossfade)
            {
                return EngineTickOutcome::PromotedStandby { queue_index: index };
            }
        }

        if let Some(active) = &self.active
            && active.is_finished()
        {
            if self.standby.is_some()
                && let Some(index) = self.promote_standby(0.0)
            {
                return EngineTickOutcome::PromotedStandby { queue_index: index };
            }
            self.pause();
            return EngineTickOutcome::TrackEnded;
        }

        if let Some(active) = &mut self.active {
            active
                .playback_clock
                .synchronize(active.position(), Instant::now());
        }

        EngineTickOutcome::Playing
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EngineTickOutcome {
    Idle,
    Playing,
    PromotedStandby { queue_index: usize },
    TrackEnded,
    /// The output stream died (device unplugged, Bluetooth dropped). Decks
    /// were cleared; reload the current track at `position`.
    OutputLost { position: f64, was_playing: bool },
}
