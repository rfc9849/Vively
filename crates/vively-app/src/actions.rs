use vively_core::{
    Error,
    models::{Audio, Playlist},
};

use crate::{
    app::{AppCore, CoreEvent, Dirty},
    models::{LoadState, ProfileSearchData},
};

impl AppCore {
    pub fn toggle_follow_playlist(&mut self, playlist: Playlist) {
        let Some(token) = self.access_token() else {
            return;
        };
        let key = playlist.stable_id();
        let currently_following = playlist.is_following.unwrap_or(false)
            || self
                .playlists
                .iter()
                .any(|p| p.full_id() == playlist.full_id());
        let account = self.account_generation;
        let service = self.service.clone();
        self.spawn(
            async move {
                if currently_following {
                    service.delete_playlist(&playlist, &token).await?;
                    Ok::<_, Error>(false)
                } else {
                    service.follow_playlist(&playlist, &token).await?;
                    Ok(true)
                }
            },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                match res {
                    Ok(now_following) => {
                        if let Some(d) = core.playlist_details.get_mut(&key) {
                            d.playlist.is_following = Some(now_following);
                        }
                        core.load_playlists(true);
                        core.toast(if now_following {
                            "Добавлено в медиатеку"
                        } else {
                            "Удалено из медиатеки"
                        });
                        core.mark(Dirty::PAGES | Dirty::PLAYLISTS);
                    }
                    Err(err) => core.toast(format!("Ошибка: {err}")),
                }
            },
        );
    }

    pub fn delete_own_playlist(&mut self, playlist: Playlist) {
        let Some(token) = self.access_token() else {
            return;
        };
        let key = playlist.stable_id();
        let full = playlist.full_id();
        let account = self.account_generation;
        let service = self.service.clone();
        self.spawn(
            async move { service.delete_playlist(&playlist, &token).await },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                match res {
                    Ok(_) => {
                        core.playlists.retain(|p| p.full_id() != full);
                        core.playlist_details.remove(&key);
                        core.emit(CoreEvent::PlaylistDeleted { key });
                        core.toast("Плейлист удалён");
                        core.mark(Dirty::PLAYLISTS | Dirty::PAGES);
                    }
                    Err(err) => core.toast(format!("Ошибка удаления: {err}")),
                }
            },
        );
    }

    /// "Add to playlist" picker. Busy/added sets are reset by the embedder
    /// when it opens the picker.
    pub fn add_track_to_playlist(&mut self, track: Audio, playlist: Playlist) {
        let Some(token) = self.access_token() else {
            return;
        };
        let pl_fid = playlist.full_id();
        if self.add_pl_busy.contains(&pl_fid) || self.add_pl_added.contains(&pl_fid) {
            return;
        }
        self.add_pl_busy.insert(pl_fid.clone());
        self.mark(Dirty::PLAYLISTS);

        let account = self.account_generation;
        let service = self.service.clone();
        let request = playlist.clone();
        self.spawn(
            async move { service.add_to_playlist(&track, &request, &token).await },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                core.add_pl_busy.remove(&pl_fid);
                match res {
                    Ok(_) => {
                        core.add_pl_added.insert(pl_fid);
                        let title = playlist.title.clone();
                        core.load_playlist_detail(playlist, true);
                        core.load_playlists(true);
                        core.toast(format!("Добавлено в «{title}»"));
                    }
                    Err(err) => core.toast(format!("Ошибка: {err}")),
                }
                core.mark(Dirty::PLAYLISTS);
            },
        );
    }

    pub fn remove_track_from_playlist(&mut self, track: Audio, playlist: Playlist) {
        let Some(token) = self.access_token() else {
            return;
        };
        let target = playlist.clone();
        let account = self.account_generation;
        let service = self.service.clone();
        self.spawn(
            async move {
                service
                    .remove_from_playlist(&track, &playlist, &token)
                    .await
            },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                match res {
                    Ok(result) if result > 0 => {
                        core.load_playlist_detail(target, true);
                        core.load_playlists(true);
                        core.toast("Удалено из плейлиста");
                    }
                    Ok(_) => core.toast("Не удалось удалить трек из плейлиста"),
                    Err(err) => core.toast(format!("Ошибка: {err}")),
                }
            },
        );
    }

    pub fn is_disliked(&self, track: &Audio) -> bool {
        self.disliked_tracks.contains(&(track.owner_id, track.id))
    }

    /// Optimistic; reverted if the request fails. Disliking the playing
    /// track skips it.
    pub fn toggle_dislike_track(&mut self, track: Audio) {
        let Some(token) = self.access_token() else {
            return;
        };
        let id = (track.owner_id, track.id);
        let dislike = !self.disliked_tracks.contains(&id);
        if dislike {
            self.disliked_tracks.insert(id);
            if self
                .queue
                .current()
                .is_some_and(|t| (t.owner_id, t.id) == id)
            {
                self.next_track();
            }
        } else {
            self.disliked_tracks.remove(&id);
        }
        self.mark(Dirty::LIBRARY);

        let account = self.account_generation;
        let service = self.service.clone();
        self.spawn(
            async move {
                if dislike {
                    service.add_dislike(&track, &token).await
                } else {
                    service.remove_dislike(&track, &token).await
                }
            },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                match res {
                    Ok(_) => core.toast(if dislike {
                        "Трек будет реже попадаться в рекомендациях"
                    } else {
                        "Отметка «Не нравится» снята"
                    }),
                    Err(err) => {
                        if dislike {
                            core.disliked_tracks.remove(&id);
                        } else {
                            core.disliked_tracks.insert(id);
                        }
                        core.toast(format!("Ошибка: {err}"));
                        core.mark(Dirty::LIBRARY);
                    }
                }
            },
        );
    }

    /// Optimistic swap with the neighbour; reloads the playlist on failure.
    pub fn move_track_in_playlist(&mut self, track: Audio, playlist: Playlist, up: bool) {
        let Some(token) = self.access_token() else {
            return;
        };
        let key = playlist.stable_id();
        let Some(detail) = self.playlist_details.get_mut(&key) else {
            return;
        };
        let Some(index) = detail
            .tracks
            .iter()
            .position(|t| (t.owner_id, t.id) == (track.owner_id, track.id))
        else {
            return;
        };
        let new_index = if up {
            match index.checked_sub(1) {
                Some(i) => i,
                None => return,
            }
        } else if index + 1 < detail.tracks.len() {
            index + 1
        } else {
            return;
        };
        detail.tracks.swap(index, new_index);
        self.mark(Dirty::PAGES);

        let account = self.account_generation;
        let service = self.service.clone();
        let target = playlist.clone();
        self.spawn(
            async move {
                service
                    .move_in_playlist(&playlist, &track, new_index, &token)
                    .await
            },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                if let Err(err) = res {
                    core.load_playlist_detail(target, true);
                    core.toast(format!("Не удалось переместить трек: {err}"));
                }
            },
        );
    }

    pub fn save_playlist_copy(&mut self, playlist: Playlist) {
        let Some(token) = self.access_token() else {
            return;
        };
        let account = self.account_generation;
        let service = self.service.clone();
        self.spawn(
            async move { service.save_playlist_copy(&playlist, &token).await },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                match res {
                    Ok(_) => {
                        core.load_playlists(true);
                        core.toast("Копия плейлиста сохранена в медиатеку");
                    }
                    Err(err) => core.toast(format!("Ошибка: {err}")),
                }
            },
        );
    }

    /// Optimistic; reverted if the request fails.
    pub fn toggle_artist_follow(&mut self, artist_id: String) {
        let (Some(token), Some(uid)) = (self.access_token(), self.signed_in_user_id()) else {
            return;
        };
        let Some(info) = self
            .artist_pages
            .get_mut(&artist_id)
            .and_then(|p| p.info.as_mut())
        else {
            return;
        };
        let follow = !info.is_followed.unwrap_or(false);
        info.is_followed = Some(follow);
        self.mark(Dirty::PAGES);

        let account = self.account_generation;
        let service = self.service.clone();
        let req_id = artist_id.clone();
        self.spawn(
            async move {
                service
                    .set_artist_followed(&req_id, uid, follow, &token)
                    .await
            },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                if let Err(err) = res {
                    if let Some(info) = core
                        .artist_pages
                        .get_mut(&artist_id)
                        .and_then(|p| p.info.as_mut())
                    {
                        info.is_followed = Some(!follow);
                    }
                    core.toast(format!("Ошибка: {err}"));
                    core.mark(Dirty::PAGES);
                }
            },
        );
    }

    pub fn toggle_owner_follow(&mut self, owner_id: i64) {
        let Some(token) = self.access_token() else {
            return;
        };
        let follow = !self.followed_owners.contains(&owner_id);
        let account = self.account_generation;
        let service = self.service.clone();
        self.spawn(
            async move { service.set_owner_followed(owner_id, follow, &token).await },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                match res {
                    Ok(_) => {
                        if follow {
                            core.followed_owners.insert(owner_id);
                        } else {
                            core.followed_owners.remove(&owner_id);
                        }
                        core.toast(if follow {
                            "Вы подписались на музыку пользователя"
                        } else {
                            "Подписка на музыку отменена"
                        });
                        core.mark(Dirty::FRIENDS);
                    }
                    Err(err) => core.toast(format!("Ошибка: {err}")),
                }
            },
        );
    }

    /// Search inside a friend's music. An empty query clears the results.
    pub fn search_in_profile(&mut self, owner_id: i64, query: String) {
        if query.is_empty() {
            self.profile_search = None;
            self.mark(Dirty::FRIENDS);
            return;
        }
        let Some(token) = self.access_token() else {
            return;
        };
        self.profile_search = Some(ProfileSearchData {
            owner_id,
            query: query.clone(),
            results: Default::default(),
            state: LoadState::Loading,
        });
        self.mark(Dirty::FRIENDS);

        let account = self.account_generation;
        let service = self.service.clone();
        let req_query = query.clone();
        self.spawn(
            async move {
                service
                    .search_in_profile(owner_id, &req_query, &token)
                    .await
            },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                let Some(search) = core.profile_search.as_mut() else {
                    return;
                };
                if search.owner_id != owner_id || search.query != query {
                    return;
                }
                match res {
                    Ok(results) => {
                        search.results = results;
                        search.state = LoadState::Loaded;
                    }
                    Err(err) => search.state = LoadState::Failed(err.to_string()),
                }
                core.mark(Dirty::FRIENDS);
            },
        );
    }
}
