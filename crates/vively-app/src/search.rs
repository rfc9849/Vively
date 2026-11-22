use std::time::Duration;

use vively_core::{
    Error,
    catalog::{SearchItem, search_items},
};

use crate::{
    app::{AppCore, CoreEvent, Dirty},
    models::{LoadState, SearchResultsData},
};

const SEARCH_DEBOUNCE: Duration = Duration::from_millis(400);
/// A typed (debounced) query only lands in history if the user keeps
/// looking at its results this long.
const HISTORY_SETTLE: Duration = Duration::from_secs(3);

impl AppCore {
    fn save_search_history(&self) {
        let _ = self.search_history.save(&self.search_history_path);
    }

    fn record_search(&mut self, query: &str) {
        if self.search_history.record(query) {
            self.save_search_history();
            self.mark(Dirty::SEARCH);
        }
    }

    /// Live typing in the search box: debounced request, history settles later.
    pub fn edit_search_query(&mut self, query: String) {
        self.search_query = query.clone();
        self.search_generation += 1;
        let seq = self.search_generation;
        let trimmed = query.trim().to_owned();
        if trimmed.is_empty() {
            self.search_state = LoadState::Idle;
            self.search_results = SearchResultsData::default();
            self.mark(Dirty::SEARCH);
            return;
        }
        self.search_state = LoadState::Loading;
        self.mark(Dirty::SEARCH);
        self.after(SEARCH_DEBOUNCE, move |core| {
            if core.search_generation == seq {
                core.run_search_request(trimmed, true);
            }
        });
    }

    /// Enter in the search box: search now and remember the query.
    pub fn commit_search(&mut self, query: String) {
        self.search_query = query.clone();
        let trimmed = query.trim().to_owned();
        if trimmed.is_empty() {
            self.mark(Dirty::SEARCH);
            return;
        }
        self.search_generation += 1;
        self.record_search(&trimmed);
        self.run_search_request(trimmed, false);
    }

    pub fn pick_search(&mut self, entry: String) {
        self.search_query = entry.clone();
        self.search_generation += 1;
        self.record_search(&entry);
        self.run_search_request(entry, false);
    }

    pub fn clear_search(&mut self) {
        self.search_generation += 1;
        self.search_query.clear();
        self.search_state = LoadState::Idle;
        self.search_results = SearchResultsData::default();
        self.mark(Dirty::SEARCH);
    }

    pub fn delete_search_history(&mut self, entry: &str) {
        self.search_history.remove(entry);
        self.save_search_history();
        self.mark(Dirty::SEARCH);
    }

    pub fn clear_search_history(&mut self) {
        self.search_history.clear();
        self.save_search_history();
        self.mark(Dirty::SEARCH);
    }

    /// Answer to [`CoreEvent::SearchSettled`]: the embedder calls this only
    /// if search results are still on screen.
    pub fn settle_search_history(&mut self, query: &str, generation: u64) {
        if self.search_generation == generation {
            self.record_search(query);
        }
    }

    pub fn run_search_request(&mut self, query: String, settle_history: bool) {
        let Some(token) = self.access_token() else {
            return;
        };
        let seq = self.search_generation;
        self.search_state = LoadState::Loading;
        self.mark(Dirty::SEARCH);

        let service = self.service.clone();
        let request = query.clone();
        self.spawn(
            async move {
                let resp = service.search_audio(&request, &token).await?;
                let items = search_items(&resp);
                Ok::<_, Error>((resp, items))
            },
            move |core, res| {
                if core.search_generation != seq {
                    return;
                }
                let mut settle = false;
                match res {
                    Ok((resp, items)) => {
                        let mut data = SearchResultsData::default();
                        for item in items {
                            match item {
                                SearchItem::Artists { artists, .. } => data.artists.extend(artists),
                                SearchItem::Music(sec) => data.sections.push(sec),
                            }
                        }
                        if data.sections.is_empty() {
                            if let Some(audios) = resp.audios {
                                data.tracks = audios;
                            }
                            if let Some(pls) = resp.playlists {
                                data.playlists = pls;
                            }
                        }
                        settle = settle_history && !data.is_empty();
                        core.search_results = data;
                        core.search_state = LoadState::Loaded;
                    }
                    Err(err) => core.search_state = LoadState::Failed(err.to_string()),
                }
                core.mark(Dirty::SEARCH);
                if settle {
                    core.after(HISTORY_SETTLE, move |core| {
                        core.emit(CoreEvent::SearchSettled {
                            query,
                            generation: seq,
                        });
                    });
                }
            },
        );
    }
}
