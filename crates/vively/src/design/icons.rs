use std::{borrow::Cow, time::Duration};

use gpui_kit::{
    Animation, AnimationExt as _, AnyElement, AssetSource, ElementId, Hsla, IntoElement as _,
    Result, SharedString, Styled as _, Svg, Transformation, assets::Assets, percentage, px, svg,
};

pub struct VivelyAssets;

impl AssetSource for VivelyAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = Icon::bytes_for_path(path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Assets.list(path)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Logo,
    Home,
    Search,
    Heart,
    HeartFill,
    HeartSlash,
    Infinity,
    Users,
    Mic,
    Album,
    Trash,
    Folder,
    Download,
    Downloaded,
    Upload,
    Play,
    Pause,
    Prev,
    Next,
    Shuffle,
    Repeat,
    Wave,
    Lyrics,
    Queue,
    Plus,
    Check,
    Close,
    More,
    Sliders,
    Volume,
    Mute,
    Expand,
    ChevLeft,
    ChevRight,
    ChevUp,
    ChevDown,
    Moon,
    Sun,
    Settings,
    Share,
    Edit,
    Lock,
    Loader,
    QrCode,
}

impl Icon {
    pub const fn asset_path(self) -> SharedString {
        match self {
            Self::Logo => SharedString::new_static("vively/logo.svg"),
            Self::Home => SharedString::new_static("vively/home.svg"),
            Self::Search => SharedString::new_static("vively/search.svg"),
            Self::Heart => SharedString::new_static("vively/heart.svg"),
            Self::HeartFill => SharedString::new_static("vively/heart_fill.svg"),
            Self::HeartSlash => SharedString::new_static("vively/heart_slash.svg"),
            Self::Infinity => SharedString::new_static("vively/infinity.svg"),
            Self::Users => SharedString::new_static("vively/users.svg"),
            Self::Mic => SharedString::new_static("vively/mic.svg"),
            Self::Album => SharedString::new_static("vively/album.svg"),
            Self::Trash => SharedString::new_static("vively/trash.svg"),
            Self::Folder => SharedString::new_static("vively/folder.svg"),
            Self::Download => SharedString::new_static("vively/download.svg"),
            Self::Downloaded => SharedString::new_static("vively/downloaded.svg"),
            Self::Upload => SharedString::new_static("vively/upload.svg"),
            Self::Play => SharedString::new_static("vively/play.svg"),
            Self::Pause => SharedString::new_static("vively/pause.svg"),
            Self::Prev => SharedString::new_static("vively/prev.svg"),
            Self::Next => SharedString::new_static("vively/next.svg"),
            Self::Shuffle => SharedString::new_static("vively/shuffle.svg"),
            Self::Repeat => SharedString::new_static("vively/repeat.svg"),
            Self::Wave => SharedString::new_static("vively/wave.svg"),
            Self::Lyrics => SharedString::new_static("vively/lyrics.svg"),
            Self::Queue => SharedString::new_static("vively/queue.svg"),
            Self::Plus => SharedString::new_static("vively/plus.svg"),
            Self::Check => SharedString::new_static("vively/check.svg"),
            Self::Close => SharedString::new_static("vively/close.svg"),
            Self::More => SharedString::new_static("vively/more.svg"),
            Self::Sliders => SharedString::new_static("vively/sliders.svg"),
            Self::Volume => SharedString::new_static("vively/volume.svg"),
            Self::Mute => SharedString::new_static("vively/mute.svg"),
            Self::Expand => SharedString::new_static("vively/expand.svg"),
            Self::ChevLeft => SharedString::new_static("vively/chev_left.svg"),
            Self::ChevRight => SharedString::new_static("vively/chev_right.svg"),
            Self::ChevUp => SharedString::new_static("vively/chev_up.svg"),
            Self::ChevDown => SharedString::new_static("vively/chev_down.svg"),
            Self::Moon => SharedString::new_static("vively/moon.svg"),
            Self::Sun => SharedString::new_static("vively/sun.svg"),
            Self::Settings => SharedString::new_static("vively/settings.svg"),
            Self::Share => SharedString::new_static("vively/share.svg"),
            Self::Edit => SharedString::new_static("vively/edit.svg"),
            Self::Lock => SharedString::new_static("vively/lock.svg"),
            Self::Loader => SharedString::new_static("vively/loader.svg"),
            Self::QrCode => SharedString::new_static("vively/qr_code.svg"),
        }
    }

    pub fn bytes_for_path(path: &str) -> Option<&'static [u8]> {
        let icon = match path {
            "vively/logo.svg" => Self::Logo,
            "vively/home.svg" => Self::Home,
            "vively/search.svg" => Self::Search,
            "vively/heart.svg" => Self::Heart,
            "vively/heart_fill.svg" => Self::HeartFill,
            "vively/heart_slash.svg" => Self::HeartSlash,
            "vively/infinity.svg" => Self::Infinity,
            "vively/users.svg" => Self::Users,
            "vively/mic.svg" => Self::Mic,
            "vively/album.svg" => Self::Album,
            "vively/trash.svg" => Self::Trash,
            "vively/folder.svg" => Self::Folder,
            "vively/download.svg" => Self::Download,
            "vively/downloaded.svg" => Self::Downloaded,
            "vively/upload.svg" => Self::Upload,
            "vively/play.svg" => Self::Play,
            "vively/pause.svg" => Self::Pause,
            "vively/prev.svg" => Self::Prev,
            "vively/next.svg" => Self::Next,
            "vively/shuffle.svg" => Self::Shuffle,
            "vively/repeat.svg" => Self::Repeat,
            "vively/wave.svg" => Self::Wave,
            "vively/lyrics.svg" => Self::Lyrics,
            "vively/queue.svg" => Self::Queue,
            "vively/plus.svg" => Self::Plus,
            "vively/check.svg" => Self::Check,
            "vively/close.svg" => Self::Close,
            "vively/more.svg" => Self::More,
            "vively/sliders.svg" => Self::Sliders,
            "vively/volume.svg" => Self::Volume,
            "vively/mute.svg" => Self::Mute,
            "vively/expand.svg" => Self::Expand,
            "vively/chev_left.svg" => Self::ChevLeft,
            "vively/chev_right.svg" => Self::ChevRight,
            "vively/chev_up.svg" => Self::ChevUp,
            "vively/chev_down.svg" => Self::ChevDown,
            "vively/moon.svg" => Self::Moon,
            "vively/sun.svg" => Self::Sun,
            "vively/settings.svg" => Self::Settings,
            "vively/share.svg" => Self::Share,
            "vively/edit.svg" => Self::Edit,
            "vively/lock.svg" => Self::Lock,
            "vively/loader.svg" => Self::Loader,
            "vively/qr_code.svg" => Self::QrCode,
            _ => return None,
        };
        Some(icon.svg_bytes())
    }

    pub const fn svg_bytes(self) -> &'static [u8] {
        match self {
            Self::Logo => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M12 3H18V7H14V17A4 4 0 1 1 12 13.536Z" fill="black"/></svg>"#,
            Self::Home => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4 10.5L12 4l8 6.5V20a1 1 0 0 1-1 1h-4.5v-5.5h-5V21H5a1 1 0 0 1-1-1v-9.5z" fill="none" stroke="black" stroke-width="1.8" stroke-linejoin="round"/></svg>"#,
            Self::Search => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="10.8" cy="10.8" r="6.3" fill="none" stroke="black" stroke-width="1.8"/><path d="M15.5 15.5L20 20" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round"/></svg>"#,
            Self::Heart => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M12 20.2l-7.2-7a4.8 4.8 0 0 1 6.8-6.8l.4.4.4-.4a4.8 4.8 0 0 1 6.8 6.8l-7.2 7z" fill="none" stroke="black" stroke-width="1.8" stroke-linejoin="round"/></svg>"#,
            Self::HeartFill => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M12 20.2l-7.2-7a4.8 4.8 0 0 1 6.8-6.8l.4.4.4-.4a4.8 4.8 0 0 1 6.8 6.8l-7.2 7z" fill="black"/></svg>"#,
            Self::HeartSlash => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M12 20.2l-7.2-7a4.8 4.8 0 0 1 6.8-6.8l.4.4.4-.4a4.8 4.8 0 0 1 6.8 6.8l-7.2 7z" fill="none" stroke="black" stroke-width="1.8" stroke-linejoin="round"/><path d="M4.5 4.5l15 15" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round"/></svg>"#,
            Self::Infinity => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M18.2 8.5a4 4 0 0 1 0 7c-2.2 0-4-2-6.2-3.5C9.8 10.5 8 8.5 5.8 8.5a4 4 0 0 0 0 7c2.2 0 4-2 6.2-3.5 2.2-1.5 4-3.5 6.2-3.5z" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round"/></svg>"#,
            Self::Users => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="9" cy="8.5" r="3.2" fill="none" stroke="black" stroke-width="1.8"/><path d="M3.5 19c0-3 2.5-5.2 5.5-5.2s5.5 2.2 5.5 5.2" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round"/><circle cx="16.5" cy="9.2" r="2.5" fill="none" stroke="black" stroke-width="1.8"/><path d="M16 14.2c2.4.3 4.5 2 4.5 4.8" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round"/></svg>"#,
            Self::Mic => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect x="9" y="3.5" width="6" height="10.5" rx="3" fill="none" stroke="black" stroke-width="1.8"/><path d="M6 11a6 6 0 0 0 12 0M12 17v3.5M9 20.5h6" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round"/></svg>"#,
            Self::Album => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" rx="3" fill="none" stroke="black" stroke-width="1.8"/><circle cx="12" cy="12" r="4" fill="none" stroke="black" stroke-width="1.6"/><circle cx="12" cy="12" r="1.2" fill="black"/></svg>"#,
            Self::Trash => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4.5 7h15M9.5 7V5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v2M6.8 7l.9 12a1.5 1.5 0 0 0 1.5 1.4h5.6a1.5 1.5 0 0 0 1.5-1.4l.9-12" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>"#,
            Self::Folder => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M3.5 7.5a2 2 0 0 1 2-2h4l2 2.2h7a2 2 0 0 1 2 2v7.8a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2V7.5z" fill="none" stroke="black" stroke-width="1.8" stroke-linejoin="round"/></svg>"#,
            Self::Download => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M12 4v11m0 0l-4-4m4 4l4-4M5 19.5h14" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>"#,
            Self::Downloaded => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="9" fill="black" opacity="0.2"/><path d="M12 7.5v7m0 0l-3-3m3 3l3-3" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>"#,
            Self::Upload => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M12 16V5m0 0L8 9m4-4l4 4M5 19.5h14" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>"#,
            Self::Play => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M7.5 5.2l11.2 6.8-11.2 6.8V5.2z" fill="black"/></svg>"#,
            Self::Pause => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect x="6.5" y="5" width="4" height="14" rx="1.2" fill="black"/><rect x="13.5" y="5" width="4" height="14" rx="1.2" fill="black"/></svg>"#,
            Self::Prev => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M6 6v12M18 6l-9.5 6L18 18V6z" fill="black" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>"#,
            Self::Next => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M18 6v12M6 6l9.5 6L6 18V6z" fill="black" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>"#,
            Self::Shuffle => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4 7h3.8l8.4 10H20m0 0l-2.6-2.6M20 17l-2.6 2.6M4 17h3.8l2.6-3.1M14.2 9.4L16.2 7H20m0 0l-2.6-2.6M20 7l-2.6 2.6" fill="none" stroke="black" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>"#,
            Self::Repeat => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M5 10V7.5A1.5 1.5 0 0 1 6.5 6H19m0 0l-2.8-2.8M19 6l-2.8 2.8M19 14v2.5a1.5 1.5 0 0 1-1.5 1.5H5m0 0l2.8 2.8M5 18l2.8-2.8" fill="none" stroke="black" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>"#,
            Self::Wave => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4 12v2m4-6v8m4-11v14m4-9v6m4-4v2" fill="none" stroke="black" stroke-width="2" stroke-linecap="round"/></svg>"#,
            Self::Lyrics => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M5 7h14M5 12h10M5 17h12" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round"/></svg>"#,
            Self::Queue => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4 7h16M4 12h16M4 17h10" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round"/><path d="M17 15l4 2.5-4 2.5v-5z" fill="black"/></svg>"#,
            Self::Plus => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round"/></svg>"#,
            Self::Check => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M5 12.5l4.5 4.5L19 7.5" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>"#,
            Self::Close => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round"/></svg>"#,
            Self::More => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="6" cy="12" r="1.6" fill="black"/><circle cx="12" cy="12" r="1.6" fill="black"/><circle cx="18" cy="12" r="1.6" fill="black"/></svg>"#,
            Self::Sliders => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4 7h2.8M11.2 7H20M4 12h8.8M17.2 12H20M4 17h3.8M12.2 17H20" fill="none" stroke="black" stroke-width="1.7" stroke-linecap="round"/><circle cx="9" cy="7" r="2.2" fill="none" stroke="black" stroke-width="1.7"/><circle cx="15" cy="12" r="2.2" fill="none" stroke="black" stroke-width="1.7"/><circle cx="10" cy="17" r="2.2" fill="none" stroke="black" stroke-width="1.7"/></svg>"#,
            Self::Volume => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4.5 9.5h3.2L12 5.8v12.4l-4.3-3.7H4.5v-5z" fill="black"/><path d="M15.5 8.8a4.5 4.5 0 0 1 0 6.4M18.2 6.2a8 8 0 0 1 0 11.6" fill="none" stroke="black" stroke-width="1.7" stroke-linecap="round"/></svg>"#,
            Self::Mute => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4.5 9.5h3.2L12 5.8v12.4l-4.3-3.7H4.5v-5z" fill="black"/><path d="M15.5 9.5l5 5m0-5l-5 5" fill="none" stroke="black" stroke-width="1.7" stroke-linecap="round"/></svg>"#,
            Self::Expand => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M14 4h6v6M20 4l-6.5 6.5M10 20H4v-6M4 20l6.5-6.5" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>"#,
            Self::ChevLeft => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M14.5 6l-6 6 6 6" fill="none" stroke="black" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"/></svg>"#,
            Self::ChevRight => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M9.5 6l6 6-6 6" fill="none" stroke="black" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"/></svg>"#,
            Self::ChevUp => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M6 14.5l6-6 6 6" fill="none" stroke="black" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"/></svg>"#,
            Self::ChevDown => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M6 9.5l6 6 6-6" fill="none" stroke="black" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"/></svg>"#,
            Self::Moon => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M20.2 14.8A8.2 8.2 0 0 1 9.2 3.8a8.2 8.2 0 1 0 11 11z" fill="none" stroke="black" stroke-width="1.8" stroke-linejoin="round"/></svg>"#,
            Self::Sun => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="4" fill="none" stroke="black" stroke-width="1.8"/><path d="M12 2.8v2.2M12 19v2.2M2.8 12H5M19 12h2.2M5.5 5.5l1.6 1.6M16.9 16.9l1.6 1.6M18.5 5.5l-1.6 1.6M7.1 16.9l-1.6 1.6" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round"/></svg>"#,
            Self::Settings => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M10.3 3.5h3.4l.5 2.4 1.6.9 2.3-.8 1.7 2.9-1.8 1.6v1.9l1.8 1.6-1.7 2.9-2.3-.8-1.6.9-.5 2.4h-3.4l-.5-2.4-1.6-.9-2.3.8-1.7-2.9 1.8-1.6v-1.9L4.2 8.9l1.7-2.9 2.3.8 1.6-.9.5-2.4z" fill="none" stroke="black" stroke-width="1.7" stroke-linejoin="round"/><circle cx="12" cy="12" r="2.6" fill="none" stroke="black" stroke-width="1.7"/></svg>"#,
            Self::Share => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M12 15V4m0 0L8 8m4-4l4 4M5 14v4.5A1.5 1.5 0 0 0 6.5 20h11a1.5 1.5 0 0 0 1.5-1.5V14" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>"#,
            Self::Edit => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M15.8 4.5l3.7 3.7L8.7 19H5v-3.7L15.8 4.5z" fill="none" stroke="black" stroke-width="1.8" stroke-linejoin="round"/></svg>"#,
            Self::Lock => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect x="5.5" y="10.5" width="13" height="9" rx="2" fill="none" stroke="black" stroke-width="1.8"/><path d="M8.5 10.5V8a3.5 3.5 0 0 1 7 0v2.5" fill="none" stroke="black" stroke-width="1.8"/></svg>"#,
            Self::Loader => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="8.5" fill="none" stroke="black" stroke-width="2.2" opacity="0.22"/><path d="M12 3.5A8.5 8.5 0 0 1 19.7 15.59" fill="none" stroke="black" stroke-width="2.2" stroke-linecap="round"/></svg>"#,
            Self::QrCode => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect x="3.5" y="3.5" width="6.5" height="6.5" rx="1.2" fill="none" stroke="black" stroke-width="1.8"/><rect x="5.8" y="5.8" width="1.9" height="1.9" fill="black"/><rect x="14" y="3.5" width="6.5" height="6.5" rx="1.2" fill="none" stroke="black" stroke-width="1.8"/><rect x="16.3" y="5.8" width="1.9" height="1.9" fill="black"/><rect x="3.5" y="14" width="6.5" height="6.5" rx="1.2" fill="none" stroke="black" stroke-width="1.8"/><rect x="5.8" y="16.3" width="1.9" height="1.9" fill="black"/><path d="M14 14h2.5v2.5H14zM18 14h2.5v2.5H18zM16 16.5h2.5V19H16zM14 18.5h2V20.5h-2zM18.5 18.5h2V20.5h-2z" fill="black"/></svg>"#,
        }
    }
}

pub fn icon(kind: Icon, size_px: f32, color: Hsla) -> Svg {
    svg()
        .path(kind.asset_path())
        .size(px(size_px))
        .flex_shrink_0()
        .text_color(color)
}

pub fn spinner(id: impl Into<ElementId>, size_px: f32, color: Hsla) -> AnyElement {
    icon(Icon::Loader, size_px, color)
        .with_animation(
            id,
            Animation::new(Duration::from_millis(850)).repeat(),
            |el, delta| el.with_transformation(Transformation::rotate(percentage(delta))),
        )
        .into_any_element()
}
