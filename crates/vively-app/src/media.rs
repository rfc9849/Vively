use std::{collections::VecDeque, fs, path::PathBuf, sync::Arc};

use vively_core::{
    Error, auth::filename_artist_title, library::MusicLibrary, media::read_id3, models::Audio,
};

use crate::{
    app::{AppCore, CoreEvent, Dirty},
    models::{UploadItem, UploadItemStatus},
};

/// Pre-fill an upload row from ID3 tags, falling back to "Artist - Title"
/// in the file name.
pub fn upload_item_for(path: PathBuf) -> UploadItem {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio.mp3".into());
    let stem = path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tags = fs::read(&path)
        .ok()
        .map(|b| read_id3(&b))
        .unwrap_or_default();
    let (fb_artist, fb_title) = filename_artist_title(&stem);
    UploadItem {
        artist: tags.artist.filter(|s| !s.is_empty()).unwrap_or(fb_artist),
        title: tags.title.filter(|s| !s.is_empty()).unwrap_or(fb_title),
        path,
        file_name,
        status: UploadItemStatus::Pending,
    }
}

fn export_file_name(track: &Audio) -> String {
    format!("{} - {}", track.artist, track.title)
        .chars()
        .map(|c| {
            if c.is_control() || "/\\:*?\"<>|".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(180)
        .collect()
}

impl AppCore {
    pub fn set_upload_files(&mut self, paths: Vec<PathBuf>) {
        self.upload_items = paths.into_iter().map(upload_item_for).collect();
        self.upload_in_flight = false;
        self.mark(Dirty::UPLOAD);
    }

    /// Upload every row not yet done, one at a time. The embedder copies
    /// edited artist/title into `upload_items` first. Emits
    /// [`CoreEvent::UploadFinished`].
    pub fn confirm_audio_upload(&mut self) {
        if self.upload_in_flight || self.upload_items.is_empty() {
            return;
        }
        let Some(token) = self.access_token() else {
            return;
        };
        let pending: VecDeque<usize> = self
            .upload_items
            .iter()
            .enumerate()
            .filter(|(_, item)| !matches!(item.status, UploadItemStatus::Done))
            .map(|(idx, _)| idx)
            .collect();
        if pending.is_empty() {
            self.emit(CoreEvent::UploadFinished { all_done: true });
            return;
        }
        self.upload_in_flight = true;
        self.mark(Dirty::UPLOAD);
        let lib = Arc::clone(&self.library);
        self.upload_next(lib, token, pending, 0);
    }

    fn upload_next(
        &mut self,
        lib: Arc<tokio::sync::Mutex<MusicLibrary>>,
        token: String,
        mut pending: VecDeque<usize>,
        uploaded: usize,
    ) {
        if !Arc::ptr_eq(&self.library, &lib) {
            return;
        }
        let Some(idx) = pending.pop_front() else {
            self.upload_in_flight = false;
            let all_done = self
                .upload_items
                .iter()
                .all(|i| matches!(i.status, UploadItemStatus::Done));
            if uploaded > 0 {
                self.toast(format!("Загружено в Мою музыку: {uploaded}"));
            }
            self.emit(CoreEvent::UploadFinished { all_done });
            self.mark(Dirty::UPLOAD);
            return;
        };
        let Some(item) = self.upload_items.get_mut(idx) else {
            return self.upload_next(lib, token, pending, uploaded);
        };
        item.status = UploadItemStatus::Uploading;
        let (path, artist, title) = (
            item.path.clone(),
            item.artist.trim().to_owned(),
            item.title.trim().to_owned(),
        );
        self.mark(Dirty::UPLOAD);

        let service = self.service.clone();
        let lib_ref = Arc::clone(&lib);
        let tok = token.clone();
        self.spawn(
            async move {
                let bytes = fs::read(&path)?;
                let saved = service
                    .upload_and_save_audio(bytes, &artist, &title, &tok)
                    .await?;
                let mut library = lib_ref.lock().await;
                library.prepend_uploaded(saved);
                Ok::<_, Error>(library.snapshot())
            },
            move |core, res| {
                if !Arc::ptr_eq(&core.library, &lib) {
                    return;
                }
                let mut uploaded = uploaded;
                match res {
                    Ok(snapshot) => {
                        uploaded += 1;
                        if let Some(item) = core.upload_items.get_mut(idx) {
                            item.status = UploadItemStatus::Done;
                        }
                        core.apply_library_snapshot(snapshot);
                    }
                    Err(err) => {
                        if let Some(item) = core.upload_items.get_mut(idx) {
                            item.status = UploadItemStatus::Failed(err.to_string());
                        }
                    }
                }
                core.mark(Dirty::UPLOAD);
                core.upload_next(lib, token, pending, uploaded);
            },
        );
    }

    /// Write the track as an MP3 into the exports dir (copying the offline
    /// file when there is one). Emits [`CoreEvent::ExportFinished`].
    pub fn export_track_mp3(&mut self, track: Audio) {
        let local_ready = self
            .offline_store
            .as_ref()
            .and_then(|s| s.local_file(&track).ok().flatten());
        let export_dir = self.exports_dir.clone();
        let downloader = self.downloader.clone();
        let service = self.service.clone();
        let token = self.access_token();
        let full_id = track.full_id();
        self.spawn(
            async move {
                fs::create_dir_all(&export_dir)?;
                if let Some(cached) = local_ready {
                    let out = export_dir.join(format!("{}.mp3", export_file_name(&track)));
                    fs::copy(&cached, &out)?;
                    return Ok::<_, Error>(out);
                }
                let mut live = track;
                if live.stream_url().is_none()
                    && let Some(tok) = &token
                    && let Ok(mut fresh) = service.tracks_by_id(&[live.stable_id()], tok).await
                    && let Some(first) = fresh.pop()
                {
                    live = first;
                }
                downloader.export_track(&live, &export_dir).await
            },
            move |core, res| {
                core.emit(CoreEvent::ExportFinished {
                    full_id,
                    result: res.map_err(|err| err.to_string()),
                });
            },
        );
    }
}
