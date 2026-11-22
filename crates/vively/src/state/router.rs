use std::{path::PathBuf, time::Instant};

use gpui_kit::{Pixels, Point};
pub use vively_app::models::{RecentQuickItem, RecentTargetKind, SectionTarget};
use vively_core::models::{Audio, Playlist};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Home,
    Search,
    Library,
    Friends,
    FriendMusic(i64),
    Downloads,
    Collection(String),
    DownloadedCol(String),
    Artist(String),
    Section(SectionTarget),
}

impl Route {
    pub fn nav_highlight(&self) -> Option<&'static str> {
        match self {
            Self::Home => Some("home"),
            Self::Search => Some("search"),
            Self::Library => Some("library"),
            Self::Friends | Self::FriendMusic(_) => Some("friends"),
            Self::Downloads | Self::DownloadedCol(_) => Some("downloads"),
            Self::Collection(_) | Self::Artist(_) | Self::Section(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelTab {
    Lyrics,
    Queue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Downloads,
    Account,
    About,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorTrackSource {
    Likes,
    Search,
    Playlists,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportState {
    Exporting,
    Ready(PathBuf),
    Failed(String),
}

#[derive(Debug, Clone)]
pub enum ActiveModal {
    MixTune { mix_id: String },
    BulkDownloadLibrary,
    PlaylistEditor { editing: Option<Playlist> },
    AddTracksToPlaylist { playlist: Playlist },
    ConfirmDeletePlaylist { playlist: Playlist },
    AddToPlaylist { track: Audio },
    AudioUpload,
    ShareFile { track: Audio, state: ExportState },
    Settings { tab: SettingsTab },
    Auth,
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ContextMenuKind {
    Track {
        track: Audio,
        position: Point<Pixels>,
        editable_playlist: Option<Playlist>,
    },
    Playlist {
        playlist: Playlist,
        position: Point<Pixels>,
    },
}

#[derive(Debug, Clone)]
pub struct ToastItem {
    pub id: u64,
    pub message: String,
    pub expires_at: Instant,
}

pub struct RouterState {
    history: Vec<Route>,
    index: usize,
    pub panel_open: bool,
    pub panel_tab: PanelTab,
    pub stage_open: bool,
    pub stage_view: Option<PanelTab>,
    pub modal: Option<ActiveModal>,
    pub context_menu: Option<ContextMenuKind>,
}

impl Default for RouterState {
    fn default() -> Self {
        Self {
            history: vec![Route::Home],
            index: 0,
            panel_open: true,
            panel_tab: PanelTab::Lyrics,
            stage_open: false,
            stage_view: Some(PanelTab::Lyrics),
            modal: None,
            context_menu: None,
        }
    }
}

impl RouterState {
    pub fn current(&self) -> &Route {
        self.history.get(self.index).unwrap_or(&Route::Home)
    }

    pub fn navigate(&mut self, route: Route) -> bool {
        self.stage_open = false;
        self.context_menu = None;
        if self.current() == &route {
            return false;
        }
        self.history.truncate(self.index + 1);
        self.history.push(route);
        self.index = self.history.len() - 1;
        true
    }

    pub fn can_go_back(&self) -> bool {
        self.index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.index + 1 < self.history.len()
    }

    pub fn go_back(&mut self) -> Option<Route> {
        if !self.can_go_back() {
            return None;
        }
        self.index -= 1;
        self.stage_open = false;
        self.context_menu = None;
        Some(self.current().clone())
    }

    pub fn go_forward(&mut self) -> Option<Route> {
        if !self.can_go_forward() {
            return None;
        }
        self.index += 1;
        self.stage_open = false;
        self.context_menu = None;
        Some(self.current().clone())
    }

    pub fn toggle_panel(&mut self, tab: PanelTab) {
        if self.panel_open && self.panel_tab == tab {
            self.panel_open = false;
        } else {
            self.panel_open = true;
            self.panel_tab = tab;
        }
    }

    pub fn toggle_stage_tab(&mut self, tab: PanelTab) {
        if self.stage_view == Some(tab) {
            self.stage_view = None;
        } else {
            self.stage_view = Some(tab);
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
