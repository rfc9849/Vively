use vively_core::{
    Error,
    api::Page,
    catalog::catalog_sections,
    models::{Friend, Playlist},
};

use crate::{
    app::{AppCore, Dirty},
    models::{
        ArtistPageData, CatalogPageData, FriendMusicData, LoadState, PlaylistDetailData,
        SectionTarget,
    },
};

fn empty_playlist_detail(playlist: &Playlist, state: LoadState) -> PlaylistDetailData {
    PlaylistDetailData {
        playlist: playlist.clone(),
        tracks: Vec::new(),
        total_count: playlist.count,
        owner: None,
        owner_audio_open: false,
        loading_more: false,
        more_failed: false,
        state,
    }
}

fn empty_friend_page(friend: Friend, state: LoadState) -> FriendMusicData {
    FriendMusicData {
        friend,
        tracks: Vec::new(),
        total_tracks: 0,
        playlists: Vec::new(),
        total_playlists: 0,
        playlists_error: None,
        playlists_loading_more: false,
        loading_more: false,
        more_failed: false,
        state,
    }
}

impl AppCore {
    pub fn find_playlist_by_any_id(&self, id: &str) -> Option<Playlist> {
        if let Some(detail) = self.playlist_details.get(id) {
            return Some(detail.playlist.clone());
        }
        self.playlists
            .iter()
            .find(|p| p.stable_id() == id || p.full_id() == id)
            .cloned()
    }

    /// Seed an idle detail entry so the page can render the header instantly.
    pub fn ensure_playlist_entry(&mut self, playlist: &Playlist) {
        self.playlist_details
            .entry(playlist.stable_id())
            .or_insert_with(|| empty_playlist_detail(playlist, LoadState::Idle));
    }

    pub fn load_playlist_detail(&mut self, playlist: Playlist, force: bool) {
        let key = playlist.stable_id();
        if !force
            && self
                .playlist_details
                .get(&key)
                .is_some_and(|d| matches!(d.state, LoadState::Loading | LoadState::Loaded))
        {
            return;
        }
        let Some(token) = self.access_token() else {
            return;
        };
        self.playlist_details
            .entry(key.clone())
            .and_modify(|d| d.state = LoadState::Loading)
            .or_insert_with(|| empty_playlist_detail(&playlist, LoadState::Loading));
        self.mark(Dirty::PAGES);

        let account = self.account_generation;
        let generation = {
            let g = self
                .playlist_detail_generations
                .entry(key.clone())
                .or_default();
            *g += 1;
            *g
        };
        let service = self.service.clone();
        let request = playlist.clone();
        self.spawn(
            async move {
                service
                    .playlist_contents(
                        &request,
                        Page {
                            offset: 0,
                            count: 200,
                        },
                        &token,
                    )
                    .await
            },
            move |core, res| {
                if core.account_generation != account
                    || core.playlist_detail_generations.get(&key) != Some(&generation)
                {
                    return;
                }
                match res {
                    Ok(contents) => {
                        let resolved = contents.playlist.unwrap_or(playlist);
                        core.playlist_details.insert(
                            key,
                            PlaylistDetailData {
                                playlist: resolved,
                                tracks: contents.audios.items,
                                total_count: contents.audios.count,
                                owner: contents.owner,
                                owner_audio_open: contents.owner_audio_open,
                                loading_more: false,
                                more_failed: false,
                                state: LoadState::Loaded,
                            },
                        );
                    }
                    Err(err) => {
                        if let Some(d) = core.playlist_details.get_mut(&key) {
                            d.state = LoadState::Failed(err.to_string());
                        }
                    }
                }
                core.mark(Dirty::PAGES);
            },
        );
    }

    pub fn load_more_playlist_tracks(&mut self, key: &str) {
        let Some(token) = self.access_token() else {
            return;
        };
        let Some(detail) = self.playlist_details.get_mut(key) else {
            return;
        };
        if !matches!(detail.state, LoadState::Loaded)
            || detail.loading_more
            || detail.tracks.len() as i64 >= detail.total_count
        {
            return;
        }
        detail.loading_more = true;
        detail.more_failed = false;
        let offset = detail.tracks.len();
        let playlist = detail.playlist.clone();
        self.mark(Dirty::PAGES);

        let account = self.account_generation;
        let generation = self.playlist_detail_generations.get(key).copied();
        let key = key.to_owned();
        let service = self.service.clone();
        self.spawn(
            async move {
                service
                    .playlist_tracks_page(&playlist, Page { offset, count: 200 }, &token)
                    .await
            },
            move |core, res| {
                if core.account_generation != account
                    || core.playlist_detail_generations.get(&key).copied() != generation
                {
                    return;
                }
                let Some(detail) = core.playlist_details.get_mut(&key) else {
                    return;
                };
                if !detail.loading_more || detail.tracks.len() != offset {
                    return;
                }
                detail.loading_more = false;
                match res {
                    Ok(page) if page.items.is_empty() => {
                        detail.total_count = detail.tracks.len() as i64;
                    }
                    Ok(page) => {
                        detail.tracks.extend(page.items);
                        detail.total_count = page.count.max(detail.tracks.len() as i64);
                    }
                    Err(_) => detail.more_failed = true,
                }
                core.mark(Dirty::PAGES);
            },
        );
    }

    pub fn load_artist(&mut self, artist_id: &str, force: bool) {
        if !force
            && self
                .artist_pages
                .get(artist_id)
                .is_some_and(|a| matches!(a.state, LoadState::Loading | LoadState::Loaded))
        {
            return;
        }
        let Some(token) = self.access_token() else {
            return;
        };
        let key = artist_id.to_owned();
        self.artist_pages
            .entry(key.clone())
            .and_modify(|a| a.state = LoadState::Loading)
            .or_insert_with(|| ArtistPageData {
                info: None,
                tracks: Vec::new(),
                playlists: Vec::new(),
                sections: Vec::new(),
                related: Vec::new(),
                state: LoadState::Loading,
            });
        self.mark(Dirty::PAGES);

        let account = self.account_generation;
        let service = self.service.clone();
        let req_id = key.clone();
        self.spawn(
            async move {
                let resp = service.artist_card(&req_id, &token).await?;
                let blocks: Vec<_> = resp
                    .catalog
                    .sections
                    .iter()
                    .flat_map(|s| s.blocks.clone())
                    .collect();
                let sections = catalog_sections(&resp.audios, &resp.playlists, &blocks);
                let related = service
                    .related_artists(&req_id, &token)
                    .await
                    .unwrap_or_default();
                Ok::<_, Error>((resp, sections, related))
            },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                match res {
                    Ok((resp, sections, related)) => {
                        let info = resp.artists.into_iter().next();
                        if let Some(ar) = &info {
                            core.record_recent_artist(&ar.id, &ar.name, ar.best_photo().cloned());
                        }
                        core.artist_pages.insert(
                            key,
                            ArtistPageData {
                                info,
                                tracks: resp.audios,
                                playlists: resp.playlists,
                                sections,
                                related,
                                state: LoadState::Loaded,
                            },
                        );
                    }
                    Err(err) => {
                        if let Some(p) = core.artist_pages.get_mut(&key) {
                            p.state = LoadState::Failed(err.to_string());
                        }
                    }
                }
                core.mark(Dirty::PAGES);
            },
        );
    }

    pub fn load_section(&mut self, target: SectionTarget, force: bool) {
        let key = target.key();
        if !force
            && self
                .catalog_pages
                .get(&key)
                .is_some_and(|p| matches!(p.state, LoadState::Loading | LoadState::Loaded))
        {
            return;
        }
        let Some(token) = self.access_token() else {
            return;
        };
        let fallback_title = target.fallback_title().to_owned();
        self.catalog_pages
            .entry(key.clone())
            .and_modify(|p| p.state = LoadState::Loading)
            .or_insert_with(|| CatalogPageData {
                title: fallback_title.clone(),
                sections: Vec::new(),
                next_from: None,
                loading_more: false,
                more_failed: false,
                state: LoadState::Loading,
            });
        self.mark(Dirty::PAGES);

        let account = self.account_generation;
        let service = self.service.clone();
        self.spawn(
            async move {
                let resp = match &target {
                    SectionTarget::Id { section_id, .. } => {
                        service.catalog_section(section_id, "", 50, &token).await?
                    }
                    SectionTarget::Url { url, .. } => service.catalog_page(url, &token).await?,
                };
                let sections = catalog_sections(
                    resp.audios.as_deref().unwrap_or_default(),
                    resp.playlists.as_deref().unwrap_or_default(),
                    &resp.section.blocks,
                );
                Ok::<_, Error>((resp.section.title, resp.section.next_from, sections))
            },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                match res {
                    Ok((title, next_from, sections)) => {
                        let title = title
                            .filter(|s| !s.trim().is_empty())
                            .unwrap_or(fallback_title);
                        core.catalog_pages.insert(
                            key,
                            CatalogPageData {
                                title,
                                sections,
                                next_from,
                                loading_more: false,
                                more_failed: false,
                                state: LoadState::Loaded,
                            },
                        );
                    }
                    Err(err) => {
                        if let Some(p) = core.catalog_pages.get_mut(&key) {
                            p.state = LoadState::Failed(err.to_string());
                        }
                    }
                }
                core.mark(Dirty::PAGES);
            },
        );
    }

    /// Next page of a catalog section. The caller checks the section is the
    /// one on screen.
    pub fn load_more_section(&mut self, target: SectionTarget) {
        let key = target.key();
        let SectionTarget::Id { section_id, .. } = &target else {
            return;
        };
        let Some((next_from, loading)) = self
            .catalog_pages
            .get(&key)
            .filter(|p| matches!(p.state, LoadState::Loaded))
            .map(|p| (p.next_from.clone(), p.loading_more))
        else {
            return;
        };
        let Some(cursor) = next_from.filter(|c| !c.is_empty()) else {
            return;
        };
        if loading {
            return;
        }
        let Some(token) = self.access_token() else {
            return;
        };
        if let Some(p) = self.catalog_pages.get_mut(&key) {
            p.loading_more = true;
            p.more_failed = false;
        }
        self.mark(Dirty::PAGES);

        let account = self.account_generation;
        let service = self.service.clone();
        let sid = section_id.clone();
        self.spawn(
            async move {
                let resp = service.catalog_section(&sid, &cursor, 50, &token).await?;
                let sections = catalog_sections(
                    resp.audios.as_deref().unwrap_or_default(),
                    resp.playlists.as_deref().unwrap_or_default(),
                    &resp.section.blocks,
                );
                let next_from = resp.section.next_from.filter(|next| next != &cursor);
                Ok::<_, Error>((next_from, sections))
            },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                if let Some(page) = core.catalog_pages.get_mut(&key) {
                    page.loading_more = false;
                    match res {
                        Ok((next_from, more)) => {
                            page.next_from = next_from;
                            page.sections.extend(more);
                        }
                        Err(_) => page.more_failed = true,
                    }
                }
                core.mark(Dirty::PAGES);
            },
        );
    }

    pub fn load_friends(&mut self, force: bool) {
        if !force && matches!(self.friends_state, LoadState::Loading | LoadState::Loaded) {
            return;
        }
        let Some(token) = self.access_token() else {
            return;
        };
        self.friends_state = LoadState::Loading;
        self.mark(Dirty::FRIENDS);

        let account = self.account_generation;
        let service = self.service.clone();
        self.spawn(
            async move { service.all_friends(&token).await },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                match res {
                    Ok(list) => {
                        core.friends_total = list.count;
                        core.friends = list.items;
                        core.friends_state = LoadState::Loaded;
                    }
                    Err(err) => core.friends_state = LoadState::Failed(err.to_string()),
                }
                core.mark(Dirty::FRIENDS);
            },
        );
    }

    pub fn friend_profile(&self, id: i64) -> Option<Friend> {
        self.friends
            .iter()
            .find(|f| f.id == id)
            .or_else(|| self.friend_pages.get(&id).map(|p| &p.friend))
            .cloned()
    }

    pub fn load_friend_music(&mut self, friend: Friend, force: bool) {
        let fid = friend.id;
        if !force
            && self
                .friend_pages
                .get(&fid)
                .is_some_and(|p| matches!(p.state, LoadState::Loading | LoadState::Loaded))
        {
            return;
        }
        if friend.deactivated.is_some()
            || (friend.is_closed == Some(true) && friend.can_access_closed != Some(true))
        {
            self.friend_pages
                .insert(fid, empty_friend_page(friend, LoadState::Loaded));
            self.mark(Dirty::FRIENDS);
            return;
        }
        let Some(token) = self.access_token() else {
            return;
        };
        self.friend_pages
            .entry(fid)
            .and_modify(|p| p.state = LoadState::Loading)
            .or_insert_with(|| empty_friend_page(friend.clone(), LoadState::Loading));
        self.mark(Dirty::FRIENDS);

        let account = self.account_generation;
        let service = self.service.clone();
        self.spawn(
            async move {
                let tracks = service
                    .library(
                        fid,
                        Page {
                            offset: 0,
                            count: 100,
                        },
                        &token,
                    )
                    .await?;
                let pls = service
                    .playlists(
                        fid,
                        Page {
                            offset: 0,
                            count: 100,
                        },
                        &token,
                    )
                    .await
                    .map_err(|err| err.to_string());
                Ok::<_, Error>((tracks, pls))
            },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                match res {
                    Ok((tracks, pls)) => {
                        let (playlists, total_playlists, playlists_error) = match pls {
                            Ok(list) => (list.items, list.count, None),
                            Err(err) => (Vec::new(), 0, Some(err)),
                        };
                        let total_tracks = tracks.count.max(tracks.items.len() as i64);
                        core.friend_pages.insert(
                            fid,
                            FriendMusicData {
                                friend,
                                tracks: tracks.items,
                                total_tracks,
                                playlists,
                                total_playlists,
                                playlists_error,
                                playlists_loading_more: false,
                                loading_more: false,
                                more_failed: false,
                                state: LoadState::Loaded,
                            },
                        );
                    }
                    Err(err) => {
                        if let Some(p) = core.friend_pages.get_mut(&fid) {
                            p.state = LoadState::Failed(err.to_string());
                        }
                    }
                }
                core.mark(Dirty::FRIENDS);
            },
        );
    }

    pub fn load_more_friend_playlists(&mut self, fid: i64) {
        let Some(token) = self.access_token() else {
            return;
        };
        let Some(data) = self.friend_pages.get_mut(&fid) else {
            return;
        };
        if data.playlists_loading_more || data.playlists.len() as i64 >= data.total_playlists {
            return;
        }
        data.playlists_loading_more = true;
        let offset = data.playlists.len();
        self.mark(Dirty::FRIENDS);

        let account = self.account_generation;
        let service = self.service.clone();
        self.spawn(
            async move {
                service
                    .playlists(fid, Page { offset, count: 100 }, &token)
                    .await
            },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                let Some(data) = core.friend_pages.get_mut(&fid) else {
                    return;
                };
                if !data.playlists_loading_more || data.playlists.len() != offset {
                    return;
                }
                data.playlists_loading_more = false;
                match res {
                    Ok(page) if page.items.is_empty() => {
                        data.total_playlists = data.playlists.len() as i64;
                    }
                    Ok(page) => {
                        data.playlists.extend(page.items);
                        data.total_playlists = page.count.max(data.playlists.len() as i64);
                    }
                    Err(err) => core.toast(format!("Ошибка: {err}")),
                }
                core.mark(Dirty::FRIENDS);
            },
        );
    }

    pub fn load_more_friend_tracks(&mut self, fid: i64) {
        let Some(token) = self.access_token() else {
            return;
        };
        let Some(data) = self.friend_pages.get_mut(&fid) else {
            return;
        };
        if !matches!(data.state, LoadState::Loaded)
            || data.loading_more
            || data.tracks.len() as i64 >= data.total_tracks
        {
            return;
        }
        data.loading_more = true;
        data.more_failed = false;
        let offset = data.tracks.len();
        self.mark(Dirty::FRIENDS);

        let account = self.account_generation;
        let service = self.service.clone();
        self.spawn(
            async move {
                service
                    .library(fid, Page { offset, count: 100 }, &token)
                    .await
            },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                let Some(data) = core.friend_pages.get_mut(&fid) else {
                    return;
                };
                if !data.loading_more || data.tracks.len() != offset {
                    return;
                }
                data.loading_more = false;
                match res {
                    Ok(page) if page.items.is_empty() => {
                        data.total_tracks = data.tracks.len() as i64;
                    }
                    Ok(page) => {
                        data.tracks.extend(page.items);
                        data.total_tracks = page.count.max(data.tracks.len() as i64);
                    }
                    Err(_) => data.more_failed = true,
                }
                core.mark(Dirty::FRIENDS);
            },
        );
    }
}
