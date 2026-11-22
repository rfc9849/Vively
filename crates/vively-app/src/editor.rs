use std::{collections::HashSet, path::PathBuf, time::Duration};

use vively_core::{
    Error,
    api::Page,
    models::{Audio, Playlist},
};

use crate::{
    app::{AppCore, CoreEvent, Dirty},
    fmt::fmt_tracks_count,
    models::LoadState,
};

const EDITOR_SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);

impl AppCore {
    /// Reset the picker list; `playlist` drills into that playlist's tracks.
    pub fn reset_editor_picker(&mut self, playlist: Option<Playlist>) {
        self.editor_picker_generation += 1;
        self.editor_picker_playlist = playlist;
        self.editor_picker_offset = 0;
        self.editor_picker_playlists.clear();
        self.editor_picker_tracks.clear();
        self.editor_picker_state = LoadState::Idle;
        self.editor_picker_more = true;
        self.mark(Dirty::EDITOR);
    }

    pub fn clear_editor_search(&mut self) {
        self.editor_search_gen += 1;
        self.editor_search_state = LoadState::Idle;
        self.editor_search_tracks.clear();
        self.mark(Dirty::EDITOR);
    }

    /// Fresh create/edit session seeded with `draft`.
    pub fn begin_playlist_editor(&mut self, draft: Vec<Audio>, hidden: bool) {
        self.editor_saving = false;
        self.editor_error = None;
        self.reset_editor_picker(None);
        self.clear_editor_search();
        self.editor_cover_bytes = None;
        self.editor_cover_preview = None;
        self.editor_hidden = hidden;
        self.editor_draft_tracks = draft;
    }

    /// Fresh "add tracks to an existing playlist" session.
    pub fn begin_add_tracks(&mut self) {
        self.editor_saving = false;
        self.editor_error = None;
        self.reset_editor_picker(None);
        self.clear_editor_search();
        self.editor_draft_tracks.clear();
    }

    /// Next page of the playlists picker (or of the drilled-in playlist).
    /// The caller checks the picker is open on the playlists source.
    pub fn load_editor_picker_page(&mut self) {
        if !self.editor_picker_more || matches!(self.editor_picker_state, LoadState::Loading) {
            return;
        }
        let (Some(uid), Some(token)) = (self.signed_in_user_id(), self.access_token()) else {
            return;
        };
        let service = self.service.clone();
        let playlist = self.editor_picker_playlist.clone();
        let generation = self.editor_picker_generation;
        let page = Page {
            offset: self.editor_picker_offset,
            count: 100,
        };
        self.editor_picker_state = LoadState::Loading;
        self.mark(Dirty::EDITOR);
        self.spawn(
            async move {
                if let Some(playlist) = playlist {
                    let list = service
                        .playlist_tracks_page(&playlist, page, &token)
                        .await?;
                    Ok::<_, Error>((Vec::new(), list.items, list.count))
                } else {
                    let list = service.playlists(uid, page, &token).await?;
                    Ok((list.items, Vec::new(), list.count))
                }
            },
            move |core, res| {
                if core.editor_picker_generation != generation {
                    return;
                }
                match res {
                    Ok((playlists, tracks, total)) => {
                        core.editor_picker_offset += page.count;
                        core.editor_picker_more = (!playlists.is_empty() || !tracks.is_empty())
                            && (core.editor_picker_offset as i64) < total;
                        let mut ids: HashSet<_> = core
                            .editor_picker_playlists
                            .iter()
                            .map(Playlist::full_id)
                            .collect();
                        core.editor_picker_playlists
                            .extend(playlists.into_iter().filter(|pl| ids.insert(pl.full_id())));
                        let mut ids: HashSet<_> = core
                            .editor_picker_tracks
                            .iter()
                            .map(Audio::full_id)
                            .collect();
                        core.editor_picker_tracks
                            .extend(tracks.into_iter().filter(|tr| ids.insert(tr.full_id())));
                        core.editor_picker_state = LoadState::Loaded;
                    }
                    Err(err) => core.editor_picker_state = LoadState::Failed(err.to_string()),
                }
                core.mark(Dirty::EDITOR);
            },
        );
    }

    pub fn toggle_editor_draft_track(&mut self, track: Audio) {
        if self.editor_saving {
            return;
        }
        if let Some(pos) = self
            .editor_draft_tracks
            .iter()
            .position(|t| t.id == track.id && t.owner_id == track.owner_id)
        {
            self.editor_draft_tracks.remove(pos);
        } else {
            self.editor_draft_tracks.push(track);
        }
        self.mark(Dirty::EDITOR);
    }

    pub fn move_editor_draft_track(&mut self, from: usize, to: usize) {
        let len = self.editor_draft_tracks.len();
        if self.editor_saving || from >= len || to >= len || from == to {
            return;
        }
        let track = self.editor_draft_tracks.remove(from);
        self.editor_draft_tracks.insert(to, track);
        self.mark(Dirty::EDITOR);
    }

    pub fn clear_editor_draft_tracks(&mut self) {
        if self.editor_saving || self.editor_draft_tracks.is_empty() {
            return;
        }
        self.editor_draft_tracks.clear();
        self.mark(Dirty::EDITOR);
    }

    pub fn add_editor_tracks_bulk(&mut self, tracks: Vec<Audio>) {
        if self.editor_saving {
            return;
        }
        let mut known: HashSet<(i64, i64)> = self
            .editor_draft_tracks
            .iter()
            .map(|t| (t.owner_id, t.id))
            .collect();
        let before = self.editor_draft_tracks.len();
        self.editor_draft_tracks
            .extend(tracks.into_iter().filter(|t| known.insert((t.owner_id, t.id))));
        if self.editor_draft_tracks.len() != before {
            self.mark(Dirty::EDITOR);
        }
    }

    pub fn remove_editor_tracks_bulk(&mut self, tracks: &[Audio]) {
        if self.editor_saving {
            return;
        }
        let to_remove: HashSet<(i64, i64)> = tracks.iter().map(|t| (t.owner_id, t.id)).collect();
        let before = self.editor_draft_tracks.len();
        self.editor_draft_tracks
            .retain(|t| !to_remove.contains(&(t.owner_id, t.id)));
        if self.editor_draft_tracks.len() != before {
            self.mark(Dirty::EDITOR);
        }
    }

    /// `jpeg` is the already re-encoded cover; `preview` is shown in the UI.
    pub fn set_editor_cover(&mut self, jpeg: Vec<u8>, preview: PathBuf) {
        self.editor_cover_bytes = Some(jpeg);
        self.editor_cover_preview = Some(preview);
        self.mark(Dirty::EDITOR);
    }

    pub fn clear_editor_cover(&mut self) {
        if self.editor_saving {
            return;
        }
        self.editor_cover_bytes = None;
        self.editor_cover_preview = None;
        self.mark(Dirty::EDITOR);
    }

    /// VK track search inside the picker; debounced unless `immediate`.
    pub fn search_editor_tracks(&mut self, query: String, immediate: bool) {
        self.editor_search_gen += 1;
        let seq = self.editor_search_gen;
        let trimmed = query.trim().to_owned();
        self.editor_search_tracks.clear();
        if trimmed.is_empty() {
            self.editor_search_state = LoadState::Idle;
            self.mark(Dirty::EDITOR);
            return;
        }
        if self.access_token().is_none() {
            return;
        }
        self.editor_search_state = LoadState::Loading;
        self.mark(Dirty::EDITOR);
        if immediate {
            self.run_editor_search(seq, trimmed);
        } else {
            self.after(EDITOR_SEARCH_DEBOUNCE, move |core| {
                core.run_editor_search(seq, trimmed);
            });
        }
    }

    fn run_editor_search(&mut self, seq: u64, query: String) {
        if self.editor_search_gen != seq {
            return;
        }
        let Some(token) = self.access_token() else {
            return;
        };
        let service = self.service.clone();
        self.spawn(
            async move { service.search_audio(&query, &token).await },
            move |core, res| {
                if core.editor_search_gen != seq {
                    return;
                }
                match res {
                    Ok(resp) => {
                        core.editor_search_tracks = resp.audios.unwrap_or_default();
                        core.editor_search_state = LoadState::Loaded;
                    }
                    Err(err) => core.editor_search_state = LoadState::Failed(err.to_string()),
                }
                core.mark(Dirty::EDITOR);
            },
        );
    }

    /// Create (`editing == None`) or update a playlist from the editor draft.
    /// Emits [`CoreEvent::PlaylistSaved`] on success.
    pub fn save_playlist_editor(&mut self, editing: Option<Playlist>, title: &str, desc: &str) {
        if self.editor_saving {
            return;
        }
        let title = title.trim().to_owned();
        if title.is_empty() {
            self.editor_error = Some("Укажите название плейлиста".into());
            self.mark(Dirty::EDITOR);
            return;
        }
        let desc = desc.trim().to_owned();
        let hidden = self.editor_hidden;
        let cover_jpeg = self.editor_cover_bytes.clone();
        let draft_ids: Vec<String> = self
            .editor_draft_tracks
            .iter()
            .map(Audio::full_id)
            .collect();
        let (Some(token), Some(uid)) = (self.access_token(), self.signed_in_user_id()) else {
            self.editor_error = Some("Войдите в аккаунт, чтобы сохранить плейлист".into());
            self.mark(Dirty::EDITOR);
            return;
        };
        self.editor_error = None;
        self.editor_saving = true;
        self.mark(Dirty::EDITOR);

        let account = self.account_generation;
        let service = self.service.clone();
        self.spawn(
            async move {
                let mut target = if let Some(mut existing) = editing {
                    if service
                        .edit_playlist(&existing, &title, &desc, hidden, &token)
                        .await?
                        <= 0
                    {
                        return Err(Error::InvalidState("VK не сохранил изменения".into()));
                    }
                    existing.title = title;
                    existing.description = Some(desc);
                    existing.no_discover = Some(hidden);
                    existing
                } else {
                    service
                        .create_playlist(uid, &title, &desc, &draft_ids, hidden, &token)
                        .await?
                };
                let cover_error = match cover_jpeg {
                    Some(jpeg) => match service.upload_playlist_cover(&target, jpeg, &token).await {
                        Ok(thumb) => {
                            target.photo = Some(thumb);
                            None
                        }
                        Err(err) => Some(err.to_string()),
                    },
                    None => None,
                };
                Ok::<_, Error>((target, cover_error))
            },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                core.editor_saving = false;
                match res {
                    Ok((saved, cover_error)) => {
                        core.playlist_details.remove(&saved.stable_id());
                        core.load_playlists(true);
                        match cover_error {
                            Some(err) => {
                                core.toast(format!("Плейлист сохранён, обложка не загружена: {err}"))
                            }
                            None => core.toast("Плейлист сохранён"),
                        }
                        core.emit(CoreEvent::PlaylistSaved { playlist: saved });
                    }
                    Err(err) => {
                        core.editor_error = Some(format!("Не удалось сохранить плейлист: {err}"));
                    }
                }
                core.mark(Dirty::EDITOR | Dirty::PLAYLISTS);
            },
        );
    }

    /// Append the draft to `playlist`. Emits [`CoreEvent::EditorClosed`].
    pub fn submit_add_tracks_to_playlist(&mut self, playlist: Playlist) {
        if self.editor_saving || self.editor_draft_tracks.is_empty() {
            return;
        }
        let Some(token) = self.access_token() else {
            self.editor_error = Some("Войдите в аккаунт, чтобы добавить треки".into());
            self.mark(Dirty::EDITOR);
            return;
        };
        let tracks = self.editor_draft_tracks.clone();
        let added = tracks.len();
        self.editor_error = None;
        self.editor_saving = true;
        self.mark(Dirty::EDITOR);

        let account = self.account_generation;
        let service = self.service.clone();
        let target = playlist.clone();
        self.spawn(
            async move {
                service
                    .add_tracks_to_playlist(&tracks, &target, &token)
                    .await
            },
            move |core, res| {
                if core.account_generation != account {
                    return;
                }
                core.editor_saving = false;
                match res {
                    Ok(_) => {
                        core.editor_draft_tracks.clear();
                        let title = playlist.title.clone();
                        core.load_playlist_detail(playlist, true);
                        core.load_playlists(true);
                        core.toast(format!(
                            "Добавлено в «{title}»: {}",
                            fmt_tracks_count(added as i64)
                        ));
                        core.emit(CoreEvent::EditorClosed);
                    }
                    Err(err) => {
                        core.editor_error = Some(format!("Не удалось добавить треки: {err}"));
                    }
                }
                core.mark(Dirty::EDITOR);
            },
        );
    }
}
