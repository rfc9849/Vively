use std::time::{Duration, Instant};

use gpui_kit::{Bounds, Entity, Pixels, ScrollHandle, component::input::InputState, point, px};
pub use vively_app::models::{
    ArtistPageData, AuthStep, CatalogPageData, FriendMusicData, LoadState, LyricsLoadState,
    PlaylistDetailData, ProfileSearchData, QrStage, SearchResultsData, UploadItemStatus,
    active_synced_lyric_index, default_mix_settings,
};

pub const LYRICS_FOCAL_RATIO: f32 = 0.36;
const LYRICS_SMOOTH_TAU_SECS: f32 = 0.14;
const LYRICS_SETTLE_EPS_PX: f32 = 0.5;
const LYRICS_MANUAL_DETECT_EPS_PX: f32 = 1.5;
pub const LYRICS_MANUAL_PAUSE: Duration = Duration::from_secs(4);

#[derive(Debug, Default)]
pub struct LyricsScrollState {
    pub track_id: Option<String>,
    pub active_line: Option<usize>,
    pub target_y: Option<f32>,
    pub last_offset_y: Option<f32>,
    pub last_viewport_h: f32,
    pub snap_next: bool,
    pub anim_last_frame: Option<Instant>,
    pub manual_pause_until: Option<Instant>,
    pub manual_paused: bool,
    pub was_visible: bool,
    pub frame_requested: bool,
}

impl LyricsScrollState {
    pub fn mark_hidden(&mut self) {
        self.was_visible = false;
        self.anim_last_frame = None;
        self.frame_requested = false;
    }

    pub fn reset(&mut self) {
        *self = Self {
            snap_next: true,
            ..Self::default()
        };
    }

    pub fn resume_auto_scroll(&mut self) {
        self.manual_paused = false;
        self.manual_pause_until = None;
        self.anim_last_frame = None;
    }

    pub fn mark_manual_scroll(&mut self, now: Instant, offset_y: f32) {
        self.manual_paused = true;
        self.manual_pause_until = Some(now + LYRICS_MANUAL_PAUSE);
        self.anim_last_frame = None;
        self.last_offset_y = Some(offset_y);
    }

    pub fn prepare_render(
        &mut self,
        handle: &ScrollHandle,
        track_id: &str,
        active_line: Option<usize>,
        is_playing: bool,
        reduce_motion: bool,
        now: Instant,
    ) -> bool {
        self.frame_requested = false;

        let reopened = !self.was_visible;
        let track_changed = self.track_id.as_deref() != Some(track_id);
        if reopened || track_changed {
            self.was_visible = true;
            self.track_id = Some(track_id.to_owned());
            self.active_line = active_line;
            self.target_y = None;
            self.snap_next = true;
            self.anim_last_frame = None;
            self.manual_paused = false;
            self.manual_pause_until = None;
            if track_changed {
                handle.set_offset(point(px(0.0), px(0.0)));
                self.last_offset_y = Some(0.0);
            } else {
                self.last_offset_y = Some(f32::from(handle.offset().y));
            }
            self.last_viewport_h = f32::from(handle.bounds().size.height);
            return false;
        }

        let actual_y = f32::from(handle.offset().y);
        let max_y = f32::from(handle.max_offset().y).max(0.0);
        let viewport_h = f32::from(handle.bounds().size.height);
        let viewport_changed = (viewport_h - self.last_viewport_h).abs() > 1.0;

        if let Some(last_y) = self.last_offset_y {
            let expected_y = if viewport_h > 1.0 {
                last_y.clamp(-max_y, 0.0)
            } else {
                last_y
            };
            if !viewport_changed && (actual_y - expected_y).abs() > LYRICS_MANUAL_DETECT_EPS_PX {
                self.mark_manual_scroll(now, actual_y);
            } else {
                self.last_offset_y = Some(actual_y);
            }
        } else {
            self.last_offset_y = Some(actual_y);
        }
        self.last_viewport_h = viewport_h;

        if self.manual_paused {
            if is_playing && self.manual_pause_until.is_some_and(|until| now >= until) {
                self.manual_paused = false;
                self.manual_pause_until = None;
                self.anim_last_frame = None;
            } else {
                self.active_line = active_line;
                return false;
            }
        }

        if self.active_line != active_line {
            self.active_line = active_line;
            self.target_y = None;
        }

        if active_line.is_none() {
            self.target_y = Some(0.0);
        }

        let Some(raw_target_y) = self.target_y else {
            return false;
        };
        let target_y = if viewport_h > 1.0 {
            raw_target_y.clamp(-max_y, 0.0)
        } else {
            raw_target_y
        };

        if self.snap_next || reduce_motion {
            self.snap_next = false;
            self.anim_last_frame = None;
            if (actual_y - target_y).abs() > LYRICS_SETTLE_EPS_PX {
                handle.set_offset(point(px(0.0), px(target_y)));
                self.last_offset_y = Some(target_y);
            }
            return false;
        }

        let diff = target_y - actual_y;
        if diff.abs() <= LYRICS_SETTLE_EPS_PX {
            self.anim_last_frame = None;
            if diff != 0.0 {
                handle.set_offset(point(px(0.0), px(target_y)));
                self.last_offset_y = Some(target_y);
            }
            return false;
        }

        let dt = self
            .anim_last_frame
            .map_or(1.0 / 60.0, |prev| {
                now.saturating_duration_since(prev).as_secs_f32()
            })
            .clamp(0.001, 0.05);
        let alpha = 1.0 - (-dt / LYRICS_SMOOTH_TAU_SECS).exp();
        let mut next_y = actual_y + diff * alpha;
        let settled = (target_y - next_y).abs() <= LYRICS_SETTLE_EPS_PX;
        if settled {
            next_y = target_y;
            self.anim_last_frame = None;
        } else {
            self.anim_last_frame = Some(now);
            self.frame_requested = true;
        }
        handle.set_offset(point(px(0.0), px(next_y)));
        self.last_offset_y = Some(next_y);
        !settled
    }

    pub fn record_active_line_bounds(
        &mut self,
        handle: &ScrollHandle,
        line_bounds: Bounds<Pixels>,
        reduce_motion: bool,
    ) -> bool {
        let viewport = handle.bounds();
        let viewport_h = f32::from(viewport.size.height);
        if viewport_h <= 1.0 {
            return false;
        }
        let cur_y = f32::from(handle.offset().y);
        let max_y = f32::from(handle.max_offset().y).max(0.0);
        let content_center_y = f32::from(line_bounds.center().y - viewport.top()) - cur_y;
        let target_y = (viewport_h * LYRICS_FOCAL_RATIO - content_center_y).clamp(-max_y, 0.0);

        self.target_y = Some(target_y);
        self.last_viewport_h = viewport_h;

        if self.manual_paused {
            return false;
        }

        let diff = (target_y - cur_y).abs();
        if self.snap_next || reduce_motion {
            self.snap_next = false;
            self.anim_last_frame = None;
            if diff > LYRICS_SETTLE_EPS_PX {
                handle.set_offset(point(px(0.0), px(target_y)));
                self.last_offset_y = Some(target_y);
                if !self.frame_requested {
                    self.frame_requested = true;
                    return true;
                }
            } else {
                self.last_offset_y = Some(cur_y);
            }
            return false;
        }

        if diff > LYRICS_SETTLE_EPS_PX && !self.frame_requested {
            self.frame_requested = true;
            return true;
        }

        false
    }
}

/// gpui inputs for one upload row; the row data lives in `core.upload_items`.
pub struct UploadInputs {
    pub artist: Entity<InputState>,
    pub title: Entity<InputState>,
}
