use std::collections::HashMap;

use reqwest::header::{HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;
use serde_json::Value;
use url::Url;

use crate::{
    Error, Result, VkClient,
    client::{Parameters, params},
    config::{API_VERSION, AUDIO_API_VERSION},
    lyrics::VkLyricsResponse,
    models::*,
    playback::PlaybackEvent,
};

#[path = "vk_scripts.rs"]
mod scripts;

#[derive(Clone)]
pub struct AudioService {
    pub client: VkClient,
}

#[derive(Debug, Clone, Copy)]
pub struct Page {
    pub offset: usize,
    pub count: usize,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            offset: 0,
            count: 100,
        }
    }
}

impl Page {
    fn insert(self, p: &mut Parameters) {
        p.insert("offset".into(), self.offset.to_string());
        p.insert("count".into(), self.count.to_string());
    }
}

impl AudioService {
    pub fn new(client: VkClient) -> Self {
        Self { client }
    }

    async fn call<T: DeserializeOwned>(
        &self,
        method: &str,
        mut p: Parameters,
        token: &str,
    ) -> Result<T> {
        p.entry("v".into())
            .or_insert_with(|| AUDIO_API_VERSION.into());
        self.client.method(method, p, Some(token)).await
    }

    async fn catalog_call<T: DeserializeOwned>(
        &self,
        method: &str,
        mut p: Parameters,
        token: &str,
    ) -> Result<T> {
        p.entry("v".into())
            .or_insert_with(|| AUDIO_API_VERSION.into());
        let mut headers = HeaderMap::new();
        headers.insert(
            "user-agent",
            HeaderValue::from_str(&self.client.config().catalog_user_agent)
                .map_err(|_| Error::InvalidState("Invalid catalog user agent".into()))?,
        );
        self.client
            .method_with_headers(method, p, headers, Some(token))
            .await
    }

    async fn execute<T: DeserializeOwned>(
        &self,
        code: &str,
        mut p: Parameters,
        token: &str,
    ) -> Result<T> {
        p.insert("code".into(), code.into());
        self.call("execute", p, token).await
    }

    async fn catalog<T: DeserializeOwned>(
        &self,
        code: &str,
        mut p: Parameters,
        token: &str,
    ) -> Result<T> {
        p.insert("code".into(), code.into());
        self.catalog_call("execute", p, token).await
    }

    pub async fn profile_info(&self, token: &str) -> Result<VkUser> {
        self.call("account.getProfileInfo", Parameters::new(), token)
            .await
    }

    pub async fn library(&self, owner_id: i64, page: Page, token: &str) -> Result<VkList<Audio>> {
        let mut p = params([("owner_id", owner_id.to_string())]);
        page.insert(&mut p);
        self.call("audio.get", p, token).await
    }

    pub async fn friends(&self, page: Page, token: &str) -> Result<VkList<Friend>> {
        let mut p = params([("fields", "photo_base".into()), ("order", "name".into())]);
        page.insert(&mut p);
        self.call("friends.get", p, token).await
    }

    pub async fn all_friends(&self, token: &str) -> Result<VkList<Friend>> {
        let mut all = VkList {
            count: 0,
            items: Vec::new(),
        };
        loop {
            let page = self
                .friends(
                    Page {
                        offset: all.items.len(),
                        count: 1000,
                    },
                    token,
                )
                .await?;
            all.count = page.count;
            if page.items.is_empty() {
                break;
            }
            all.items.extend(page.items);
            if all.items.len() as i64 >= all.count {
                break;
            }
        }
        all.count = all.count.max(all.items.len() as i64);
        Ok(all)
    }

    pub async fn tracks_by_id(&self, ids: &[String], token: &str) -> Result<Vec<Audio>> {
        let mut tracks = Vec::with_capacity(ids.len());
        for chunk in ids.chunks(100) {
            let page: Vec<Audio> = self
                .call(
                    "audio.getById",
                    params([("audios", chunk.join(","))]),
                    token,
                )
                .await?;
            tracks.extend(page);
        }
        Ok(tracks)
    }

    pub async fn playlists(
        &self,
        owner_id: i64,
        page: Page,
        token: &str,
    ) -> Result<VkList<Playlist>> {
        let mut p = params([("owner_id", owner_id.to_string())]);
        page.insert(&mut p);
        let mut list: VkList<Playlist> = self.call("audio.getPlaylists", p, token).await?;
        let creators: Vec<Option<i64>> = list
            .items
            .iter()
            .map(|pl| {
                let has_artists = pl.main_artists.as_ref().is_some_and(|a| !a.is_empty());
                (!has_artists).then(|| {
                    pl.original
                        .as_ref()
                        .and_then(|o| o.owner_id)
                        .filter(|id| *id != 0)
                        .unwrap_or(pl.owner_id)
                })
            })
            .collect();
        let names = self
            .owner_names(creators.iter().flatten().copied(), token)
            .await
            .unwrap_or_default();
        for (pl, creator) in list.items.iter_mut().zip(creators) {
            pl.owner_id = owner_id;
            if let Some(name) = creator.and_then(|id| names.get(&id)) {
                pl.original_owner_name = Some(name.clone());
            }
        }
        Ok(list)
    }

    pub async fn all_playlists(&self, owner_id: i64, token: &str) -> Result<VkList<Playlist>> {
        let mut all = VkList {
            count: 0,
            items: Vec::new(),
        };
        loop {
            let page = self
                .playlists(
                    owner_id,
                    Page {
                        offset: all.items.len(),
                        count: 100,
                    },
                    token,
                )
                .await?;
            all.count = page.count;
            if page.items.is_empty() {
                break;
            }
            all.items.extend(page.items);
            if all.items.len() as i64 >= all.count {
                break;
            }
        }
        all.count = all.count.max(all.items.len() as i64);
        Ok(all)
    }

    async fn owner_names(
        &self,
        ids: impl Iterator<Item = i64>,
        token: &str,
    ) -> Result<HashMap<i64, String>> {
        let mut user_ids = Vec::new();
        let mut group_ids = Vec::new();
        for id in ids {
            let (list, value) = if id < 0 {
                (&mut group_ids, -id)
            } else {
                (&mut user_ids, id)
            };
            if id != 0 && !list.contains(&value) {
                list.push(value);
            }
        }
        let join = |ids: &[i64]| ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        let mut names = HashMap::new();
        if !user_ids.is_empty() {
            let users: Vec<VkUser> = self
                .call("users.get", params([("user_ids", join(&user_ids))]), token)
                .await?;
            for user in users {
                names.insert(user.id, user.full_name());
            }
        }
        if !group_ids.is_empty() {
            let value: Value = self
                .call(
                    "groups.getById",
                    params([("group_ids", join(&group_ids))]),
                    token,
                )
                .await?;
            let groups = value.get("groups").unwrap_or(&value);
            for group in groups.as_array().into_iter().flatten() {
                if let (Some(id), Some(name)) = (
                    group.get("id").and_then(Value::as_i64),
                    group.get("name").and_then(Value::as_str),
                ) {
                    names.insert(-id, name.to_owned());
                }
            }
        }
        Ok(names)
    }

    pub async fn playlist_contents(
        &self,
        playlist: &Playlist,
        page: Page,
        token: &str,
    ) -> Result<PlaylistContents> {
        let mut p = params([
            ("owner_id", playlist.owner_id.to_string()),
            ("playlist_id", playlist.id.to_string()),
            (
                "access_key",
                playlist.access_key.as_deref().unwrap_or(token).into(),
            ),
        ]);
        page.insert(&mut p);
        self.execute(scripts::PLAYLIST_CONTENTS, p, token).await
    }

    pub async fn artist_card(&self, artist_id: &str, token: &str) -> Result<ArtistCardResponse> {
        self.call(
            "catalog.getAudioArtist",
            params([("artist_id", artist_id.into()), ("need_blocks", "1".into())]),
            token,
        )
        .await
    }

    pub async fn audio_home(&self, token: &str) -> Result<HomeResponse> {
        self.catalog(scripts::AUDIO_HOME, Parameters::new(), token)
            .await
    }

    pub async fn search_audio(&self, query: &str, token: &str) -> Result<SearchResponse> {
        self.call(
            "catalog.getAudioSearch",
            params([
                ("query", query.into()),
                ("show_suggests", "0".into()),
                ("need_blocks", "1".into()),
            ]),
            token,
        )
        .await
    }

    pub async fn report_playback_events(
        &self,
        events: &[PlaybackEvent],
        token: &str,
    ) -> Result<()> {
        let json =
            serde_json::to_string(events).map_err(|e| Error::decoding("playback events", e))?;
        let mut headers = HeaderMap::new();
        headers.insert("x-screen", HeaderValue::from_static("nowhere"));
        let _: i64 = self
            .client
            .method_with_headers(
                "stats.trackEvents",
                params([("events", json), ("v", AUDIO_API_VERSION.into())]),
                headers,
                Some(token),
            )
            .await?;
        Ok(())
    }

    pub async fn stream_mix_settings(&self, mix_id: &str, token: &str) -> Result<MixSettings> {
        let response: MixSettingsResponse = self
            .call(
                "audio.getStreamMixSettings",
                params([("mix_id", mix_id.into())]),
                token,
            )
            .await?;
        Ok(response.settings)
    }

    pub async fn stream_mix_audios(
        &self,
        mix_id: &str,
        entity_id: Option<&str>,
        count: usize,
        options_json: Option<&str>,
        token: &str,
    ) -> Result<Vec<Audio>> {
        let mut p = params([
            ("mix_id", mix_id.into()),
            ("append", "1".into()),
            ("count", count.to_string()),
        ]);
        if let Some(v) = entity_id {
            p.insert("entity_id".into(), v.into());
        }
        if let Some(v) = options_json {
            p.insert("options".into(), v.into());
        }
        self.call("audio.getStreamMixAudios", p, token).await
    }

    pub async fn catalog_page(&self, url: &str, token: &str) -> Result<CatalogSectionResponse> {
        let page: CatalogPage = self
            .catalog_call(
                "catalog.getAudio",
                params([("url", url.into()), ("need_blocks", "1".into())]),
                token,
            )
            .await?;
        let sections = page.catalog.sections;
        let default = page.catalog.default_section.as_deref();
        let raw = sections
            .iter()
            .find(|s| default.is_some() && s.get("id").and_then(Value::as_str) == default)
            .or_else(|| sections.iter().find(|s| s.get("blocks").is_some()))
            .ok_or_else(|| Error::InvalidState("Catalog page has no sections".into()))?;
        let section: CatalogSection = serde_json::from_value(raw.clone())
            .map_err(|e| Error::decoding("catalog section", e))?;
        Ok(CatalogSectionResponse {
            audios: page.audios,
            playlists: page.playlists,
            section,
        })
    }

    pub async fn catalog_section(
        &self,
        section_id: &str,
        start_from: &str,
        count: usize,
        token: &str,
    ) -> Result<CatalogSectionResponse> {
        self.catalog_call(
            "catalog.getSection",
            params([
                ("section_id", section_id.into()),
                ("start_from", start_from.into()),
                ("count", count.to_string()),
            ]),
            token,
        )
        .await
    }

    pub async fn playlist_tracks_page(
        &self,
        playlist: &Playlist,
        page: Page,
        token: &str,
    ) -> Result<VkList<Audio>> {
        let mut p = params([
            ("owner_id", playlist.owner_id.to_string()),
            ("playlist_id", playlist.id.to_string()),
            (
                "access_key",
                playlist.access_key.as_deref().unwrap_or(token).into(),
            ),
        ]);
        page.insert(&mut p);
        self.call("audio.get", p, token).await
    }

    pub async fn add_to_library(&self, track: &Audio, token: &str) -> Result<i64> {
        self.execute("return API.audio.add({ audio_id: Args.audio_id, owner_id: Args.owner_id, access_key: Args.access_key });", legacy(track_parameters(track, token)), token).await
    }

    pub async fn restore_to_library(&self, track: &Audio, token: &str) -> Result<i64> {
        self.execute("return API.audio.restore({ owner_id: Args.owner_id, audio_id: Args.audio_id, access_key: Args.access_key });", legacy(track_parameters(track, token)), token).await
    }

    pub async fn remove_from_library(
        &self,
        owner_id: i64,
        audio_id: i64,
        access_key: Option<&str>,
        token: &str,
    ) -> Result<i64> {
        self.execute("return API.audio.delete({ owner_id: Args.owner_id, audio_id: Args.audio_id, access_key: Args.access_key });", legacy(params([("owner_id", owner_id.to_string()), ("audio_id", audio_id.to_string()), ("access_key", access_key.unwrap_or(token).into())])), token).await
    }

    pub async fn add_to_playlist(
        &self,
        track: &Audio,
        playlist: &Playlist,
        token: &str,
    ) -> Result<()> {
        let _: Value = self.execute("return API.audio.addToPlaylist({ owner_id: Args.owner_id, audio_ids: Args.audio_ids, playlist_id: Args.playlist_id, access_key: Args.access_key });", legacy(params([("owner_id", playlist.owner_id.to_string()), ("audio_ids", track.full_id()), ("playlist_id", playlist.id.to_string()), ("access_key", token.into())])), token).await?;
        Ok(())
    }

    pub async fn add_tracks_to_playlist(
        &self,
        tracks: &[Audio],
        playlist: &Playlist,
        token: &str,
    ) -> Result<()> {
        for chunk in tracks.chunks(100) {
            let ids = chunk
                .iter()
                .map(Audio::full_id)
                .collect::<Vec<_>>()
                .join(",");
            if ids.is_empty() {
                continue;
            }
            let _: Value = self
                .execute(
                    "return API.audio.addToPlaylist({ owner_id: Args.owner_id, audio_ids: Args.audio_ids, playlist_id: Args.playlist_id, access_key: Args.access_key });",
                    legacy(params([
                        ("owner_id", playlist.owner_id.to_string()),
                        ("audio_ids", ids),
                        ("playlist_id", playlist.id.to_string()),
                        ("access_key", token.into()),
                    ])),
                    token,
                )
                .await?;
        }
        Ok(())
    }

    pub async fn remove_from_playlist(
        &self,
        track: &Audio,
        playlist: &Playlist,
        token: &str,
    ) -> Result<i64> {
        self.call(
            "audio.removeFromPlaylist",
            legacy(params([
                ("owner_id", playlist.owner_id.to_string()),
                ("playlist_id", playlist.id.to_string()),
                ("audio_ids", track.full_id()),
            ])),
            token,
        )
        .await
    }

    pub async fn follow_playlist(
        &self,
        playlist: &Playlist,
        token: &str,
    ) -> Result<FollowPlaylistResult> {
        let mut p = playlist_parameters(playlist);
        if let Some(v) = &playlist.access_key {
            p.insert("access_key".into(), v.clone());
        }
        self.call("audio.followPlaylist", legacy(p), token).await
    }

    pub async fn delete_playlist(&self, playlist: &Playlist, token: &str) -> Result<i64> {
        self.call(
            "audio.deletePlaylist",
            legacy(playlist_parameters(playlist)),
            token,
        )
        .await
    }

    pub async fn create_playlist(
        &self,
        owner_id: i64,
        title: &str,
        description: &str,
        track_ids: &[String],
        hidden: bool,
        token: &str,
    ) -> Result<Playlist> {
        let mut p = params([("owner_id", owner_id.to_string()), ("title", title.into())]);
        if hidden {
            p.insert("no_discover".into(), "1".into());
        }
        if !description.is_empty() {
            p.insert("description".into(), description.into());
        }
        if !track_ids.is_empty() {
            p.insert("audio_ids".into(), track_ids.join(","));
        }
        self.call("audio.createPlaylist", legacy(p), token).await
    }

    pub async fn edit_playlist(
        &self,
        playlist: &Playlist,
        title: &str,
        description: &str,
        hidden: bool,
        token: &str,
    ) -> Result<i64> {
        let mut p = playlist_parameters(playlist);
        p.extend(params([
            ("title", title.into()),
            ("description", description.into()),
            ("no_discover", if hidden { "1" } else { "0" }.into()),
        ]));
        self.call("audio.editPlaylist", legacy(p), token).await
    }

    pub async fn cover_upload_url(&self, playlist: &Playlist, token: &str) -> Result<Url> {
        let mut p = playlist_parameters(playlist);
        p.insert("v".into(), AUDIO_API_VERSION.into());
        let server: CoverUploadServer = self
            .call("photos.getAudioPlaylistCoverUploadServer", p, token)
            .await?;
        Ok(server.upload_url)
    }

    pub async fn upload_cover(&self, jpeg: Vec<u8>, url: Url) -> Result<String> {
        self.client
            .upload(url, jpeg, "cover.jpg", "image/jpeg")
            .await
    }

    pub async fn save_cover(
        &self,
        owner_id: i64,
        upload_response: &str,
        token: &str,
    ) -> Result<SavedPlaylistCover> {
        self.call(
            "photos.saveAudioPlaylistCover",
            params([
                ("owner_id", owner_id.to_string()),
                ("response_json", upload_response.into()),
                ("v", AUDIO_API_VERSION.into()),
            ]),
            token,
        )
        .await
    }

    pub async fn set_playlist_cover(
        &self,
        playlist: &Playlist,
        cover: &SavedPlaylistCover,
        token: &str,
    ) -> Result<Thumb> {
        self.call(
            "audio.setPlaylistCoverPhoto",
            params([
                ("playlist_id", playlist.id.to_string()),
                ("playlist_owner_id", playlist.owner_id.to_string()),
                ("photo", cover.photo.to_string()),
                ("hash", cover.hash.clone()),
                ("v", AUDIO_API_VERSION.into()),
            ]),
            token,
        )
        .await
    }

    pub async fn audio_upload_url(&self, token: &str) -> Result<Url> {
        let server: CoverUploadServer = self
            .call(
                "audio.getUploadServer",
                params([("v", AUDIO_API_VERSION.into())]),
                token,
            )
            .await?;
        Ok(server.upload_url)
    }

    pub async fn upload_audio(&self, mp3: Vec<u8>, url: Url) -> Result<AudioUploadTicket> {
        let body = self
            .client
            .upload(url, mp3, "VKXAudioUpload.mp3", "audio/mpeg")
            .await?;
        serde_json::from_str(&body).map_err(|e| Error::decoding("audio upload server", e))
    }

    pub async fn save_audio(
        &self,
        ticket: &AudioUploadTicket,
        artist: &str,
        title: &str,
        token: &str,
    ) -> Result<Audio> {
        self.call(
            "audio.save",
            params([
                ("server", ticket.server.to_string()),
                ("audio", ticket.audio.clone()),
                ("hash", ticket.hash.clone()),
                ("artist", artist.into()),
                ("title", title.into()),
                ("v", AUDIO_API_VERSION.into()),
            ]),
            token,
        )
        .await
    }

    pub async fn lyrics(&self, track: &Audio, token: &str) -> Result<VkLyricsResponse> {
        self.call(
            "audio.getLyrics",
            params([
                ("audio_id", track.full_id()),
                ("v", AUDIO_API_VERSION.into()),
            ]),
            token,
        )
        .await
    }

    pub async fn set_broadcast(&self, track: Option<&Audio>, token: &str) -> Result<()> {
        let mut p = Parameters::new();
        if let Some(track) = track {
            p.insert("audio".into(), track.full_id());
        }
        let _: Value = self.call("audio.setBroadcast", p, token).await?;
        Ok(())
    }

    pub async fn add_dislike(&self, track: &Audio, token: &str) -> Result<()> {
        let _: Value = self
            .call(
                "audio.addDislike",
                params([("audio_ids", track.full_id())]),
                token,
            )
            .await?;
        Ok(())
    }

    pub async fn remove_dislike(&self, track: &Audio, token: &str) -> Result<()> {
        let _: Value = self
            .call(
                "audio.removeDislike",
                params([("audio_ids", track.full_id())]),
                token,
            )
            .await?;
        Ok(())
    }

    pub async fn related_artists(&self, artist_id: &str, token: &str) -> Result<Vec<ArtistInfo>> {
        let response: RelatedArtists = self
            .call(
                "audio.getRelatedArtistsById",
                params([
                    ("artist_id", artist_id.into()),
                    ("offset", "0".into()),
                    ("count", "10".into()),
                ]),
                token,
            )
            .await?;
        Ok(response.artists)
    }

    pub async fn set_artist_followed(
        &self,
        artist_id: &str,
        user_id: i64,
        followed: bool,
        token: &str,
    ) -> Result<()> {
        let method = if followed {
            "audio.followArtist"
        } else {
            "audio.unfollowArtist"
        };
        let _: Value = self
            .call(
                method,
                params([
                    ("artist_id", artist_id.into()),
                    ("user_id", user_id.to_string()),
                    ("ref", "banner".into()),
                ]),
                token,
            )
            .await?;
        Ok(())
    }

    pub async fn set_owner_followed(
        &self,
        owner_id: i64,
        followed: bool,
        token: &str,
    ) -> Result<()> {
        let method = if followed {
            "audio.followOwner"
        } else {
            "audio.unfollowOwner"
        };
        let _: Value = self
            .call(method, params([("owner_id", owner_id.to_string())]), token)
            .await?;
        Ok(())
    }

    pub async fn move_in_playlist(
        &self,
        playlist: &Playlist,
        track: &Audio,
        new_index: usize,
        token: &str,
    ) -> Result<()> {
        let actions = serde_json::json!([[track.owner_id, track.id, new_index]]).to_string();
        let mut p = playlist_parameters(playlist);
        p.insert("actions".into(), actions);
        let _: Value = self.call("audio.reorderInPlaylist", p, token).await?;
        Ok(())
    }

    pub async fn save_playlist_copy(&self, playlist: &Playlist, token: &str) -> Result<Playlist> {
        self.call(
            "audio.savePlaylistAsCopy",
            playlist_parameters(playlist),
            token,
        )
        .await
    }

    pub async fn search_in_profile(
        &self,
        owner_id: i64,
        query: &str,
        token: &str,
    ) -> Result<ProfileSearchResults> {
        self.execute(
            scripts::SEARCH_IN_PROFILE,
            params([
                ("owner_id", owner_id.to_string()),
                ("query", query.into()),
                ("p_count", "10".into()),
                ("a_count", "100".into()),
            ]),
            token,
        )
        .await
    }

    pub async fn recommendations(
        &self,
        target_audio: &str,
        user_id: i64,
        page: Page,
        token: &str,
    ) -> Result<VkList<Audio>> {
        let mut p = params([
            ("target_audio", target_audio.into()),
            ("user_id", user_id.to_string()),
        ]);
        page.insert(&mut p);
        self.call("audio.getRecommendations", p, token).await
    }
}

fn legacy(mut p: Parameters) -> Parameters {
    p.insert("v".into(), API_VERSION.into());
    p
}

fn track_parameters(track: &Audio, token: &str) -> Parameters {
    params([
        ("audio_id", track.id.to_string()),
        ("owner_id", track.owner_id.to_string()),
        (
            "access_key",
            track.access_key.as_deref().unwrap_or(token).into(),
        ),
    ])
}

fn playlist_parameters(playlist: &Playlist) -> Parameters {
    params([
        ("playlist_id", playlist.id.to_string()),
        ("owner_id", playlist.owner_id.to_string()),
    ])
}
