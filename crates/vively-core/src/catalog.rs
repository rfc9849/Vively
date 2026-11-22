use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::models::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MusicSection {
    pub id: usize,
    pub title: String,
    pub subtitle: Option<String>,
    pub show_all_section_id: Option<String>,
    pub content: MusicContent,
    pub playlist_style: PlaylistStyle,
    pub track_style: TrackStyle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MusicContent {
    Tracks(Vec<Audio>),
    Playlists(Vec<Playlist>),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub enum PlaylistStyle {
    #[default]
    Standard,
    Tile,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub enum TrackStyle {
    #[default]
    List,
    Columns,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtistEntry {
    pub id: String,
    pub name: String,
    pub avatar: Option<Url>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SearchItem {
    Music(MusicSection),
    Artists {
        id: usize,
        title: String,
        artists: Vec<ArtistEntry>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HomeBlock {
    pub id: usize,
    pub kind: HomeBlockKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HomeBlockKind {
    StreamMixes(Vec<StreamMix>),
    Links(Vec<CatalogLink>),
    MixTiles {
        title: String,
        tiles: Vec<MixTile>,
    },
    ArtistMixes {
        title: String,
        cards: Vec<ArtistMixCard>,
    },
    Recommended {
        title: String,
        show_all_section_id: Option<String>,
        playlists: Vec<RecommendedPlaylist>,
    },
    Music(MusicSection),
}

impl HomeBlock {
    fn rank(&self) -> u8 {
        match self.kind {
            HomeBlockKind::StreamMixes(_) => 0,
            HomeBlockKind::Recommended { .. } => 1,
            _ => 2,
        }
    }
}

#[derive(Default)]
struct Pending {
    title: String,
    subtitle: Option<String>,
    show_all: Option<String>,
}

impl Pending {
    fn accept(&mut self, block: &CatalogBlock) {
        if let Some(layout) = &block.layout
            && let Some(title) = layout.title.as_ref().filter(|s| !s.is_empty())
        {
            self.title = title.clone();
            self.subtitle = layout.subtitle.clone();
            self.show_all = block.show_all_section_id().map(str::to_owned);
        }
    }

    fn section(&mut self, id: usize, content: MusicContent) -> MusicSection {
        let pending = std::mem::take(self);
        MusicSection {
            id,
            title: pending.title,
            subtitle: pending.subtitle,
            show_all_section_id: pending.show_all,
            content,
            playlist_style: PlaylistStyle::Standard,
            track_style: TrackStyle::List,
        }
    }
}

fn index<T>(items: &[T], key: impl Fn(&T) -> String) -> HashMap<String, &T> {
    let mut map = HashMap::new();
    for item in items {
        map.entry(key(item)).or_insert(item);
    }
    map
}

fn resolve<T: Clone>(ids: &Option<Vec<String>>, map: &HashMap<String, &T>) -> Vec<T> {
    ids.iter()
        .flatten()
        .filter_map(|id| map.get(id).map(|v| (*v).clone()))
        .collect()
}

pub fn catalog_sections(
    audios: &[Audio],
    playlists: &[Playlist],
    blocks: &[CatalogBlock],
) -> Vec<MusicSection> {
    let audios = index(audios, Audio::full_id);
    let playlists = index(playlists, Playlist::full_id);
    let mut result = Vec::new();
    let mut pending = Pending::default();
    for block in blocks {
        pending.accept(block);
        match block.data_type.as_deref() {
            Some("music_audios") => {
                let tracks = resolve(&block.audios_ids, &audios);
                if !tracks.is_empty() {
                    let mut section = pending.section(result.len(), MusicContent::Tracks(tracks));
                    if block
                        .layout
                        .as_ref()
                        .and_then(|l| l.name.as_deref())
                        .is_some_and(|n| n.contains("slider"))
                    {
                        section.track_style = TrackStyle::Columns;
                    }
                    result.push(section);
                }
            }
            Some("music_playlists") => {
                let lists = resolve(&block.playlists_ids, &playlists);
                if !lists.is_empty() {
                    result.push(pending.section(result.len(), MusicContent::Playlists(lists)));
                }
            }
            _ => {}
        }
    }
    result
}

pub fn search_items(response: &SearchResponse) -> Vec<SearchItem> {
    let audios = index(
        response.audios.as_deref().unwrap_or_default(),
        Audio::full_id,
    );
    let playlists = index(
        response.playlists.as_deref().unwrap_or_default(),
        Playlist::full_id,
    );
    let links = index(response.links.as_deref().unwrap_or_default(), |l| {
        l.id.clone()
    });
    let mut result = Vec::new();
    let mut pending = Pending::default();
    for block in response.catalog.sections.iter().flat_map(|s| &s.blocks) {
        pending.accept(block);
        match block.data_type.as_deref() {
            Some("music_audios") => {
                let tracks = resolve(&block.audios_ids, &audios);
                if !tracks.is_empty() {
                    let mut section = pending.section(result.len(), MusicContent::Tracks(tracks));
                    section.subtitle = None;
                    if block.layout.as_ref().and_then(|l| l.name.as_deref()) != Some("list") {
                        section.track_style = TrackStyle::Columns;
                    }
                    result.push(SearchItem::Music(section));
                }
            }
            Some("music_playlists") => {
                let lists = resolve(&block.playlists_ids, &playlists);
                if !lists.is_empty() {
                    let mut section = pending.section(result.len(), MusicContent::Playlists(lists));
                    section.subtitle = None;
                    result.push(SearchItem::Music(section));
                }
            }
            Some("links") => {
                let artists: Vec<_> = resolve(&block.links_ids, &links)
                    .iter()
                    .filter_map(|l| {
                        Some(ArtistEntry {
                            id: l.artist_domain()?.into(),
                            name: l.title.clone(),
                            avatar: l.image.as_deref().and_then(medium_image).cloned(),
                        })
                    })
                    .collect();
                if !artists.is_empty() {
                    result.push(SearchItem::Artists {
                        id: result.len(),
                        title: std::mem::take(&mut pending).title,
                        artists,
                    });
                }
            }
            _ => {}
        }
    }
    result
}

pub fn home_blocks(response: &HomeResponse) -> Vec<HomeBlock> {
    let audios = index(
        response.audios.as_deref().unwrap_or_default(),
        Audio::full_id,
    );
    let playlists = index(
        response.playlists.as_deref().unwrap_or_default(),
        Playlist::full_id,
    );
    let mixes = index(
        response.audio_stream_mixes.as_deref().unwrap_or_default(),
        |m| m.id.clone(),
    );
    let recommended = index(
        response
            .recommended_playlists
            .as_deref()
            .unwrap_or_default(),
        RecommendedPlaylist::full_id,
    );
    let links = index(response.links.as_deref().unwrap_or_default(), |l| {
        l.id.clone()
    });
    let mut result: Vec<HomeBlock> = Vec::new();
    let mut pending = Pending::default();
    for block in &response.section.blocks {
        pending.accept(block);
        let kind = match block.data_type.as_deref() {
            Some("audio_stream_mixes") => {
                let items: Vec<_> = resolve(&block.audio_stream_mixes_ids, &mixes)
                    .into_iter()
                    .filter(|m| m.id != "my_music")
                    .collect();
                (!items.is_empty()).then_some(HomeBlockKind::StreamMixes(items))
            }
            Some("links") => {
                let items: Vec<_> = resolve(&block.links_ids, &links)
                    .into_iter()
                    .filter(|l| !l.is_my_audios() && l.destination().is_some())
                    .collect();
                (!items.is_empty()).then_some(HomeBlockKind::Links(items))
            }
            Some("music_playlists") => {
                let items = resolve(&block.playlists_ids, &playlists);
                if items.is_empty() {
                    None
                } else {
                    let mut section = pending.section(result.len(), MusicContent::Playlists(items));
                    if block.layout.as_ref().and_then(|l| l.name.as_deref()) == Some("crop_slider")
                    {
                        section.playlist_style = PlaylistStyle::Tile;
                    }
                    Some(HomeBlockKind::Music(section))
                }
            }
            Some("music_recommended_playlists") => {
                let mut items = resolve(&block.playlists_ids, &recommended);
                for rec in &mut items {
                    if let Some(pl) = playlists.get(&rec.full_id()) {
                        if rec.photo.is_none() {
                            rec.photo = pl.photo.clone();
                        }
                        if rec.thumbs.is_none() {
                            rec.thumbs = pl.thumbs.clone();
                        }
                        if rec.access_key.is_none() {
                            rec.access_key = pl.access_key.clone();
                        }
                        if rec.title.as_deref().is_none_or(str::is_empty) && !pl.title.is_empty() {
                            rec.title = Some(pl.title.clone());
                        }
                    }
                }
                if items.is_empty() {
                    None
                } else {
                    let header = std::mem::take(&mut pending);
                    Some(HomeBlockKind::Recommended {
                        title: header.title,
                        show_all_section_id: header.show_all,
                        playlists: items,
                    })
                }
            }
            Some("music_audios") => {
                let items = resolve(&block.audios_ids, &audios);
                if items.is_empty() {
                    None
                } else {
                    let mut section = pending.section(result.len(), MusicContent::Tracks(items));
                    section.show_all_section_id = section
                        .show_all_section_id
                        .or_else(|| block.show_all_section_id().map(str::to_owned));
                    Some(HomeBlockKind::Music(section))
                }
            }
            Some("action") => {
                let actions = block.actions.as_deref().unwrap_or_default();
                if block.layout.as_ref().and_then(|l| l.style.as_deref()) == Some("artist_mix") {
                    let cards: Vec<_> = actions
                        .iter()
                        .filter_map(ArtistMixCard::from_action)
                        .collect();
                    if cards.is_empty() {
                        None
                    } else {
                        Some(HomeBlockKind::ArtistMixes {
                            title: std::mem::take(&mut pending).title,
                            cards,
                        })
                    }
                } else if let Some(id) = actions.iter().find_map(|a| a.section_id.as_ref()) {
                    if let Some(last) = result.last_mut() {
                        match &mut last.kind {
                            HomeBlockKind::Recommended {
                                show_all_section_id,
                                ..
                            } if show_all_section_id.is_none() => {
                                *show_all_section_id = Some(id.clone());
                            }
                            HomeBlockKind::Music(section)
                                if section.show_all_section_id.is_none() =>
                            {
                                section.show_all_section_id = Some(id.clone());
                            }
                            _ => {}
                        }
                    }
                    None
                } else {
                    let tiles: Vec<_> = actions.iter().filter_map(MixTile::from_action).collect();
                    if tiles.is_empty() {
                        None
                    } else {
                        Some(HomeBlockKind::MixTiles {
                            title: std::mem::take(&mut pending).title,
                            tiles,
                        })
                    }
                }
            }
            _ => None,
        };
        if let Some(kind) = kind {
            result.push(HomeBlock {
                id: result.len(),
                kind,
            });
        }
    }
    result.sort_by_key(|b| (b.rank(), b.id));
    result
}
