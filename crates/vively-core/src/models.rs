use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use serde_json::Value;
use url::Url;

pub(crate) fn tolerant<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).ok())
}

fn default_value<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    let value = Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

fn flexible_id<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<i64, D::Error> {
    let value = Value::deserialize(d)?;
    Ok(value
        .as_i64()
        .or_else(|| value.as_str()?.parse().ok())
        .unwrap_or(0))
}

fn lossy_items<'de, D, T>(d: D) -> std::result::Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = Value::deserialize(d)?;
    Ok(value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect()
        })
        .unwrap_or_default())
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
pub struct VkList<T> {
    #[serde(default, deserialize_with = "flexible_id")]
    pub count: i64,
    #[serde(
        default,
        deserialize_with = "lossy_items",
        bound(deserialize = "T: DeserializeOwned")
    )]
    pub items: Vec<T>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Audio {
    pub id: i64,
    pub owner_id: i64,
    pub artist: String,
    pub title: String,
    pub duration: i64,
    pub is_explicit: bool,
    pub access_key: Option<String>,
    pub is_licensed: Option<bool>,
    pub track_code: Option<String>,
    pub date: Option<i64>,
    pub genre_id: Option<i64>,
    pub has_lyrics: Option<bool>,
    pub release_audio_id: Option<String>,
    pub subtitle: Option<String>,
    pub album: Option<Album>,
    pub thumb: Option<Thumb>,
    pub main_artists: Option<Vec<Artist>>,
    pub featured_artists: Option<Vec<Artist>>,
    pub content_restricted: Option<i64>,
    pub url: Option<String>,
}

impl Audio {
    pub fn full_id(&self) -> String {
        format!("{}_{}", self.owner_id, self.id)
    }
    pub fn stable_id(&self) -> String {
        self.access_key
            .as_ref()
            .map(|k| format!("{}_{k}", self.full_id()))
            .unwrap_or_else(|| self.full_id())
    }
    pub fn stream_url(&self) -> Option<Url> {
        self.url
            .as_deref()
            .filter(|v| !v.is_empty())
            .and_then(|v| Url::parse(v).ok())
    }
    pub fn is_playable(&self) -> bool {
        self.stream_url().is_some() && self.content_restricted.unwrap_or(0) == 0
    }
    pub fn artwork_url(&self) -> Option<&Url> {
        self.album
            .as_ref()
            .and_then(|a| a.thumb.as_ref())
            .and_then(Thumb::best)
            .or_else(|| self.thumb.as_ref().and_then(Thumb::best))
    }
    pub fn thumbnail_url(&self) -> Option<&Url> {
        self.album
            .as_ref()
            .and_then(|a| a.thumb.as_ref())
            .and_then(Thumb::thumbnail)
            .or_else(|| self.thumb.as_ref().and_then(Thumb::thumbnail))
    }
    pub fn formatted_duration(&self) -> String {
        format!("{}:{:02}", self.duration / 60, self.duration % 60)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Album {
    pub id: Option<i64>,
    pub owner_id: Option<i64>,
    pub title: Option<String>,
    pub access_key: Option<String>,
    pub main_color: Option<String>,
    pub thumb: Option<Thumb>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub domain: Option<String>,
    pub is_followed: Option<bool>,
    pub can_follow: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Thumb {
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub photo_34: Option<Url>,
    pub photo_68: Option<Url>,
    pub photo_135: Option<Url>,
    pub photo_270: Option<Url>,
    pub photo_300: Option<Url>,
    pub photo_600: Option<Url>,
    pub photo_1200: Option<Url>,
}

impl Thumb {
    pub fn best(&self) -> Option<&Url> {
        self.photo_1200
            .as_ref()
            .or(self.photo_600.as_ref())
            .or(self.photo_300.as_ref())
            .or(self.photo_270.as_ref())
            .or(self.photo_135.as_ref())
            .or(self.photo_68.as_ref())
            .or(self.photo_34.as_ref())
    }
    pub fn thumbnail(&self) -> Option<&Url> {
        self.photo_135
            .as_ref()
            .or(self.photo_270.as_ref())
            .or(self.photo_68.as_ref())
            .or(self.photo_300.as_ref())
            .or_else(|| self.best())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlaylistKind {
    Playlist,
    Album,
    #[default]
    Unknown,
}

impl Serialize for PlaylistKind {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_i64(match self {
            Self::Playlist => 0,
            Self::Album => 1,
            Self::Unknown => -1,
        })
    }
}

impl<'de> Deserialize<'de> for PlaylistKind {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let value = Value::deserialize(d)?;
        Ok(match (value.as_i64(), value.as_str()) {
            (Some(0), _) => Self::Playlist,
            (Some(1), _) | (_, Some("album")) => Self::Album,
            (_, Some(kind)) if !kind.is_empty() => Self::Playlist,
            _ => Self::Unknown,
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: i64,
    #[serde(default, deserialize_with = "flexible_id")]
    pub owner_id: i64,
    #[serde(default, rename = "type")]
    pub kind: PlaylistKind,
    #[serde(default, deserialize_with = "default_value")]
    pub title: String,
    #[serde(default, deserialize_with = "tolerant")]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub subtitle: Option<String>,
    #[serde(default, deserialize_with = "flexible_id")]
    pub count: i64,
    #[serde(default, deserialize_with = "tolerant")]
    pub access_key: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub photo: Option<Thumb>,
    #[serde(default, deserialize_with = "tolerant")]
    pub thumbs: Option<Vec<Thumb>>,
    #[serde(default, deserialize_with = "tolerant")]
    pub main_color: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub album_type: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub followers: Option<i64>,
    #[serde(default, deserialize_with = "tolerant")]
    pub plays: Option<i64>,
    #[serde(default, deserialize_with = "tolerant")]
    pub create_time: Option<i64>,
    #[serde(default, deserialize_with = "tolerant")]
    pub update_time: Option<i64>,
    #[serde(default, deserialize_with = "tolerant")]
    pub is_following: Option<bool>,
    #[serde(default, deserialize_with = "tolerant")]
    pub genres: Option<Vec<Genre>>,
    #[serde(default, deserialize_with = "tolerant")]
    pub permissions: Option<PlaylistPermissions>,
    #[serde(default, deserialize_with = "tolerant")]
    pub no_discover: Option<bool>,
    #[serde(default, deserialize_with = "tolerant")]
    pub year: Option<i64>,
    #[serde(default, deserialize_with = "tolerant")]
    pub is_explicit: Option<bool>,
    #[serde(default, deserialize_with = "tolerant")]
    pub main_artists: Option<Vec<Artist>>,
    #[serde(default, deserialize_with = "tolerant")]
    pub original: Option<OriginalRef>,
    #[serde(default, rename = "original_owner", deserialize_with = "owner_name")]
    pub original_owner_name: Option<String>,
    #[serde(default, rename = "owner", deserialize_with = "owner_name")]
    pub owner_name: Option<String>,
}

fn owner_name<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Option<String>, D::Error> {
    let value = Value::deserialize(d)?;
    if let Some(name) = value.as_str() {
        return Ok(Some(name.into()));
    }
    if let Some(name) = value
        .get("name")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
    {
        return Ok(Some(name.into()));
    }
    let name = ["first_name", "last_name"]
        .iter()
        .filter_map(|k| value.get(k).and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join(" ");
    Ok((!name.is_empty()).then_some(name))
}

impl Playlist {
    pub fn new(id: i64, owner_id: i64, title: impl Into<String>) -> Self {
        Self {
            id,
            owner_id,
            title: title.into(),
            kind: PlaylistKind::Playlist,
            ..Self::default()
        }
    }
    pub fn from_album(album: &Album) -> Self {
        Self {
            id: album.id.unwrap_or(0),
            owner_id: album.owner_id.unwrap_or(0),
            title: album.title.clone().unwrap_or_default(),
            kind: PlaylistKind::Album,
            access_key: album.access_key.clone(),
            photo: album.thumb.clone(),
            main_color: album.main_color.clone(),
            ..Self::default()
        }
    }
    pub fn full_id(&self) -> String {
        format!("{}_{}", self.owner_id, self.id)
    }
    pub fn stable_id(&self) -> String {
        self.access_key
            .as_ref()
            .map(|k| format!("{}_{k}", self.full_id()))
            .unwrap_or_else(|| self.full_id())
    }
    pub fn is_album(&self) -> bool {
        self.kind == PlaylistKind::Album
    }
    pub fn is_editable(&self) -> bool {
        self.kind == PlaylistKind::Playlist
            && self
                .permissions
                .as_ref()
                .and_then(|p| p.edit)
                .unwrap_or(false)
    }
    pub fn artwork_url(&self) -> Option<&Url> {
        self.photo
            .as_ref()
            .and_then(Thumb::best)
            .or_else(|| self.thumbs.as_ref()?.first()?.best())
    }
    pub fn artist_line(&self) -> Option<String> {
        self.main_artists
            .as_ref()
            .filter(|a| !a.is_empty())
            .map(|a| {
                a.iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
    }
    pub fn author_name(&self) -> Option<&str> {
        self.owner_name
            .as_deref()
            .or(self.original_owner_name.as_deref())
    }
    pub fn byline(&self) -> String {
        if self.is_album() {
            let artist = self
                .artist_line()
                .or_else(|| self.author_name().map(str::to_owned))
                .unwrap_or_else(|| "Альбом".into());
            return self
                .year
                .map(|y| format!("{artist} · {y}"))
                .unwrap_or(artist);
        }
        self.subtitle
            .as_deref()
            .filter(|v| !v.is_empty())
            .or_else(|| self.author_name())
            .unwrap_or("Плейлист")
            .into()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Genre {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PlaylistPermissions {
    pub play: Option<bool>,
    pub share: Option<bool>,
    pub edit: Option<bool>,
    pub follow: Option<bool>,
    pub delete: Option<bool>,
    pub save_as_copy: Option<bool>,
    pub boom_download: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OriginalRef {
    pub playlist_id: Option<i64>,
    pub owner_id: Option<i64>,
    pub access_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VkUser {
    #[serde(default, deserialize_with = "flexible_id")]
    pub id: i64,
    #[serde(default, alias = "firstName")]
    pub first_name: String,
    #[serde(default, alias = "lastName")]
    pub last_name: String,
    #[serde(default, alias = "photo200", deserialize_with = "tolerant")]
    pub photo_200: Option<Url>,
    #[serde(default, alias = "screenName", deserialize_with = "tolerant")]
    pub screen_name: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub phone: Option<String>,
}

impl VkUser {
    pub fn full_name(&self) -> String {
        full_name(&self.first_name, &self.last_name)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Friend {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub photo_base: Option<Url>,
    pub is_closed: Option<bool>,
    pub can_access_closed: Option<bool>,
    pub deactivated: Option<String>,
}

impl Friend {
    pub fn full_name(&self) -> String {
        full_name(&self.first_name, &self.last_name)
    }
}

fn full_name(first: &str, last: &str) -> String {
    [first, last]
        .into_iter()
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VkImage {
    pub url: Option<Url>,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

pub fn best_image(images: &[VkImage]) -> Option<&Url> {
    images
        .iter()
        .max_by_key(|i| i.width.unwrap_or(0))?
        .url
        .as_ref()
}

pub fn medium_image(images: &[VkImage]) -> Option<&Url> {
    images
        .iter()
        .filter(|i| i.width.unwrap_or(0) >= 300)
        .min_by_key(|i| i.width.unwrap_or(0))
        .or_else(|| images.iter().max_by_key(|i| i.width.unwrap_or(0)))?
        .url
        .as_ref()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtistInfo {
    pub id: String,
    pub name: String,
    pub domain: Option<String>,
    pub photo: Option<Vec<VkImage>>,
    pub is_followed: Option<bool>,
}

impl ArtistInfo {
    pub fn best_photo(&self) -> Option<&Url> {
        best_image(self.photo.as_deref()?)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Catalog {
    pub sections: Vec<CatalogSection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogSection {
    pub title: Option<String>,
    pub blocks: Vec<CatalogBlock>,
    pub next_from: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CatalogBlock {
    #[serde(default, deserialize_with = "tolerant")]
    pub data_type: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub layout: Option<CatalogLayout>,
    #[serde(default, deserialize_with = "tolerant")]
    pub audios_ids: Option<Vec<String>>,
    #[serde(default, deserialize_with = "tolerant")]
    pub playlists_ids: Option<Vec<String>>,
    #[serde(default, deserialize_with = "tolerant")]
    pub audio_stream_mixes_ids: Option<Vec<String>>,
    #[serde(default, deserialize_with = "tolerant")]
    pub catalog_banner_ids: Option<Vec<i64>>,
    #[serde(default, deserialize_with = "tolerant")]
    pub links_ids: Option<Vec<String>>,
    #[serde(default, deserialize_with = "tolerant")]
    pub actions: Option<Vec<CatalogAction>>,
    #[serde(default, deserialize_with = "tolerant")]
    pub meta: Option<CatalogMeta>,
}

impl CatalogBlock {
    pub fn show_all_section_id(&self) -> Option<&str> {
        self.actions
            .as_ref()
            .and_then(|a| a.iter().find_map(|a| a.section_id.as_deref()))
            .or_else(|| {
                self.meta
                    .as_ref()?
                    .show_all_info
                    .as_ref()?
                    .section_id
                    .as_deref()
            })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CatalogLayout {
    #[serde(default, deserialize_with = "tolerant")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub subtitle: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub style: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogMeta {
    #[serde(default, deserialize_with = "tolerant")]
    pub show_all_info: Option<ShowAllInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShowAllInfo {
    pub title: Option<String>,
    pub section_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CatalogAction {
    #[serde(default, deserialize_with = "tolerant")]
    pub section_id: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub images: Option<Vec<VkImage>>,
    #[serde(default, deserialize_with = "tolerant")]
    pub foreground_images: Option<Vec<VkImage>>,
    #[serde(default, deserialize_with = "tolerant")]
    pub id: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub mix_id: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub mix_options: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub entity_id: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub action: Option<ActionTarget>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionTarget {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtistCardResponse {
    pub artists: Vec<ArtistInfo>,
    pub audios: Vec<Audio>,
    pub playlists: Vec<Playlist>,
    pub catalog: Catalog,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct RelatedArtists {
    #[serde(default, deserialize_with = "lossy_items")]
    pub artists: Vec<ArtistInfo>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileSearchResults {
    #[serde(default, deserialize_with = "default_value")]
    pub playlists: VkList<Playlist>,
    #[serde(default, deserialize_with = "lossy_items")]
    pub audios: Vec<Audio>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct CatalogPage {
    pub audios: Option<Vec<Audio>>,
    pub playlists: Option<Vec<Playlist>>,
    pub catalog: CatalogPageSections,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct CatalogPageSections {
    pub default_section: Option<String>,
    pub sections: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogSectionResponse {
    pub audios: Option<Vec<Audio>>,
    pub playlists: Option<Vec<Playlist>>,
    pub section: CatalogSection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResponse {
    pub audios: Option<Vec<Audio>>,
    pub playlists: Option<Vec<Playlist>>,
    pub links: Option<Vec<CatalogLink>>,
    pub catalog: Catalog,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamMix {
    pub id: String,
    #[serde(default, deserialize_with = "tolerant")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub background_animation_url: Option<Url>,
    #[serde(default, deserialize_with = "tolerant")]
    pub is_tunable: Option<bool>,
    #[serde(default, deserialize_with = "tolerant")]
    pub titles: Option<StreamMixTitles>,
    #[serde(default, deserialize_with = "tolerant")]
    pub stream_mix: Option<StreamMixTitle>,
}

impl StreamMix {
    pub fn display_title(&self) -> &str {
        [
            self.title.as_deref(),
            self.titles.as_ref().and_then(|t| t.common_state.as_deref()),
            self.stream_mix.as_ref().and_then(|t| t.title.as_deref()),
        ]
        .into_iter()
        .flatten()
        .find(|t| !t.is_empty())
        .unwrap_or("VK Микс")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamMixTitles {
    pub common_state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamMixTitle {
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogBanner {
    pub id: i64,
    pub title: Option<String>,
    pub text: Option<String>,
    pub images: Option<Vec<VkImage>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogLink {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub image: Option<Vec<VkImage>>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MusicDestination {
    Playlist(Box<Playlist>),
    CatalogUrl { url: String, title: String },
}

impl CatalogLink {
    pub fn artwork_url(&self) -> Option<&Url> {
        best_image(self.image.as_deref()?)
    }
    pub fn is_my_audios(&self) -> bool {
        self.url
            .as_ref()
            .is_some_and(|u| u.contains("catalog=my_audios"))
    }
    pub fn artist_domain(&self) -> Option<&str> {
        let url = self.url.as_deref()?;
        url.contains("/artist/")
            .then(|| url.trim_end_matches('/').rsplit('/').next())
            .flatten()
    }
    pub fn destination(&self) -> Option<MusicDestination> {
        let raw = self.url.as_deref()?;
        let url = Url::parse(raw).ok()?;
        let path = url.path();
        for (marker, kind) in [
            ("/music/playlist/", PlaylistKind::Playlist),
            ("/music/album/", PlaylistKind::Album),
        ] {
            if let Some((_, id)) = path.split_once(marker) {
                let mut parts = id.split('_');
                let owner = parts.next()?.parse().ok()?;
                let id = parts.next()?.parse().ok()?;
                let mut playlist = Playlist::new(id, owner, &self.title);
                playlist.kind = kind;
                playlist.access_key = parts.next().map(str::to_owned);
                return Some(MusicDestination::Playlist(Box::new(playlist)));
            }
        }
        path.starts_with("/audio")
            .then(|| MusicDestination::CatalogUrl {
                url: raw.into(),
                title: self.title.clone(),
            })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixTile {
    pub id: String,
    pub title: String,
    pub image: Option<Url>,
    pub mix_id: String,
    pub options_json: Option<String>,
}

impl MixTile {
    pub fn from_action(a: &CatalogAction) -> Option<Self> {
        if a.action.as_ref()?.kind.as_deref()? != "play_vk_mix" {
            return None;
        }
        let title = a.title.clone()?;
        Some(Self {
            id: a.id.clone().unwrap_or_else(|| title.clone()),
            title,
            image: a.images.as_deref().and_then(best_image).cloned(),
            mix_id: a.mix_id.clone()?,
            options_json: a.mix_options.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtistMixCard {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub photo: Option<Url>,
    pub background: Option<Url>,
    pub mix_id: String,
    pub entity_id: Option<String>,
}

impl ArtistMixCard {
    pub fn from_action(a: &CatalogAction) -> Option<Self> {
        if a.action.as_ref()?.kind.as_deref()? != "play_vk_mix" {
            return None;
        }
        let title = a.title.clone()?;
        Some(Self {
            id: a
                .id
                .clone()
                .or_else(|| a.entity_id.clone())
                .unwrap_or_else(|| title.clone()),
            title,
            description: a.description.clone(),
            photo: a.foreground_images.as_deref().and_then(best_image).cloned(),
            background: a.images.as_deref().and_then(best_image).cloned(),
            mix_id: a.mix_id.clone()?,
            entity_id: a.entity_id.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecommendedPlaylist {
    pub id: i64,
    #[serde(default, deserialize_with = "flexible_id")]
    pub owner_id: i64,
    #[serde(default, deserialize_with = "tolerant")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub percentage: Option<f64>,
    #[serde(default, deserialize_with = "tolerant")]
    pub percentage_title: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub cover: Option<Url>,
    #[serde(default, deserialize_with = "tolerant")]
    pub color: Option<String>,
    #[serde(default, deserialize_with = "tolerant")]
    pub photo: Option<Thumb>,
    #[serde(default, deserialize_with = "tolerant")]
    pub thumbs: Option<Vec<Thumb>>,
    #[serde(default, deserialize_with = "tolerant")]
    pub access_key: Option<String>,
}

impl RecommendedPlaylist {
    pub fn full_id(&self) -> String {
        format!("{}_{}", self.owner_id, self.id)
    }
    pub fn artwork_url(&self) -> Option<&Url> {
        self.photo
            .as_ref()
            .and_then(Thumb::best)
            .or_else(|| self.thumbs.as_ref()?.first()?.best())
            .or(self.cover.as_ref())
    }
    pub fn match_text(&self) -> Option<String> {
        self.percentage
            .map(|v| format!("{:.0}%", (v * 100.0).round()))
    }
    pub fn as_playlist(&self) -> Playlist {
        let photo = self.photo.clone().or_else(|| {
            self.cover.clone().map(|url| Thumb {
                photo_600: Some(url),
                ..Thumb::default()
            })
        });
        Playlist {
            photo,
            thumbs: self.thumbs.clone(),
            access_key: self.access_key.clone(),
            main_color: self.color.clone(),
            ..Playlist::new(
                self.id,
                self.owner_id,
                self.title.clone().unwrap_or_default(),
            )
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HomeResponse {
    pub audio_stream_mixes: Option<Vec<StreamMix>>,
    pub catalog_banners: Option<Vec<CatalogBanner>>,
    pub links: Option<Vec<CatalogLink>>,
    pub audios: Option<Vec<Audio>>,
    pub playlists: Option<Vec<Playlist>>,
    pub recommended_playlists: Option<Vec<RecommendedPlaylist>>,
    pub section: CatalogSection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixSettingsResponse {
    pub settings: MixSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixSettings {
    pub title: String,
    pub subtitle: Option<String>,
    pub mix_categories: Vec<MixCategory>,
    pub multi_select: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixCategory {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub options: Vec<MixOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixOption {
    pub id: String,
    pub title: String,
    pub selected: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaylistContents {
    pub playlist: Option<Playlist>,
    pub audios: VkList<Audio>,
    #[serde(default, deserialize_with = "tolerant")]
    pub owner: Option<Friend>,
    #[serde(default, deserialize_with = "default_value")]
    pub owner_audio_open: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverUploadServer {
    pub upload_url: Url,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedPlaylistCover {
    pub photo: Value,
    pub hash: String,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioUploadTicket {
    pub server: i64,
    pub audio: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FollowPlaylistResult {
    pub playlist_id: Option<i64>,
    pub owner_id: Option<i64>,
}
