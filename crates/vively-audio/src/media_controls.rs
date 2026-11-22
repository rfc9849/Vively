use std::time::Duration;

use playwire::{Capabilities, Event, MediaControls, PlaybackState, PlayerConfig, Repeat, Track};
use tokio::sync::mpsc;
use vively_core::{models::Audio, playback::RepeatMode};

#[derive(Debug, Clone, PartialEq)]
pub enum MediaCommand {
    Play,
    Pause,
    TogglePlayPause,
    Stop,
    Next,
    Previous,
    SeekTo(f64),
    SeekBy(f64),
    SetVolume(f32),
    SetShuffle(bool),
    SetRepeat(RepeatMode),
}

pub struct OsMediaControls {
    controls: Option<MediaControls>,
}

impl OsMediaControls {
    pub fn disabled() -> (Self, mpsc::UnboundedReceiver<MediaCommand>) {
        let (_tx, rx) = mpsc::unbounded_channel();
        (Self { controls: None }, rx)
    }

    pub fn new() -> (Self, mpsc::UnboundedReceiver<MediaCommand>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let config = PlayerConfig::new("Vively")
            .desktop_entry("eu.r3pr3ss10n.vively")
            .track_id_prefix("/eu/r3pr3ss10n/vively/track");

        let controls = MediaControls::new(config, move |event| {
            let cmd = match event {
                Event::Play => Some(MediaCommand::Play),
                Event::Pause => Some(MediaCommand::Pause),
                Event::PlayPause => Some(MediaCommand::TogglePlayPause),
                Event::Stop => Some(MediaCommand::Stop),
                Event::Next => Some(MediaCommand::Next),
                Event::Previous => Some(MediaCommand::Previous),
                Event::SeekTo(pos) => Some(MediaCommand::SeekTo(pos.as_secs_f64())),
                Event::SeekBy(delta) => Some(MediaCommand::SeekBy(delta)),
                Event::SetVolume(vol) => Some(MediaCommand::SetVolume(vol as f32)),
                Event::SetShuffle(shuf) => Some(MediaCommand::SetShuffle(shuf)),
                Event::SetRepeat(rep) => Some(MediaCommand::SetRepeat(match rep {
                    Repeat::Off => RepeatMode::Off,
                    Repeat::All => RepeatMode::All,
                    Repeat::One => RepeatMode::One,
                })),
                _ => None,
            };
            if let Some(cmd) = cmd {
                let _ = tx.send(cmd);
            }
        })
        .ok();

        (Self { controls }, rx)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn publish(
        &mut self,
        track: Option<&Audio>,
        playing: bool,
        position_secs: f64,
        duration_secs: f64,
        volume: f32,
        repeat: RepeatMode,
        shuffle: bool,
        can_go_next: bool,
        can_go_previous: bool,
    ) {
        let Some(controls) = &mut self.controls else {
            return;
        };
        let pw_track = track.map(|t| Track {
            id: t.stable_id(),
            title: t.title.clone(),
            artists: vec![t.artist.clone()],
            album: t
                .album
                .as_ref()
                .and_then(|a| a.title.clone())
                .unwrap_or_default(),
            artwork_url: t.artwork_url().map(|u| u.to_string()).unwrap_or_default(),
            url: String::new(),
        });

        let state = PlaybackState {
            track: pw_track,
            playing,
            position: Duration::from_secs_f64(position_secs.max(0.0)),
            duration: (duration_secs > 0.0).then(|| Duration::from_secs_f64(duration_secs)),
            volume: volume.clamp(0.0, 1.0) as f64,
            repeat: match repeat {
                RepeatMode::Off => Repeat::Off,
                RepeatMode::All => Repeat::All,
                RepeatMode::One => Repeat::One,
            },
            shuffle,
            capabilities: Capabilities {
                can_go_next,
                can_go_previous,
                can_seek: track.is_some(),
            },
        };
        let _ = controls.set_state(&state);
    }
}
