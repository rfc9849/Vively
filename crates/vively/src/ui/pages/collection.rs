use std::rc::Rc;

use gpui_kit::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, component::input::Input, div, px,
};
use vively_core::{models::Audio, playback::PlaybackOrigin};

use crate::{
    design::{
        colors::Palette,
        icons::{Icon, icon, spinner},
    },
    state::{
        app_state::VivelyApp,
        models::LoadState,
        runtime::{fmt_plays_count, fmt_total_duration, fmt_tracks_count},
    },
    ui::components::{
        HeroArt, TrackTableOptions, cached_queue, pill_button, primary_play_pill,
        render_action_bar_skeleton, render_empty_state, render_error_state, render_hero_banner,
        render_track_list, render_track_list_skeleton,
    },
};

pub fn render_collection_page(
    app: &VivelyApp,
    collection_id: &str,
    offline_only: bool,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let offline_tracks: Vec<Audio>;
    let (playlist, tracks, state): (_, &[Audio], _) = if offline_only {
        let col = app
            .core.offline_collections
            .iter()
            .find(|c| c.full_id == collection_id);
        let Some(col) = col else {
            return div()
                .p(px(32.0))
                .text_color(pal.ink_2)
                .child("Коллекция не найдена в загруженном")
                .into_any_element();
        };
        let pl = col.playlist.clone();
        offline_tracks = app
            .core.offline_store
            .as_ref()
            .and_then(|s| s.collection_tracks(collection_id).ok())
            .unwrap_or_default();
        (pl, &offline_tracks, LoadState::Loaded)
    } else if let Some(detail) = app.core.playlist_details.get(collection_id) {
        (
            detail.playlist.clone(),
            &detail.tracks,
            detail.state.clone(),
        )
    } else if let Some(pl) = app.find_playlist_by_any_id(collection_id) {
        (pl, &[], LoadState::Loading)
    } else {
        return div()
            .p(px(32.0))
            .text_color(pal.ink_2)
            .child("Плейлист не найден")
            .into_any_element();
    };

    let cover_path = app.resolve_artwork(playlist.artwork_url(), cx);
    let col_tint = app.tint_for_artwork(playlist.artwork_url(), playlist.main_color.as_deref());

    let is_album = playlist.is_album();
    let is_downloaded = offline_only || app.is_collection_downloaded(&playlist);
    let uid = app.signed_in_user_id().unwrap_or(0);
    let is_own = playlist.owner_id == uid || playlist.is_editable();
    let is_following = playlist.is_following.unwrap_or(false)
        || app
            .core.playlists
            .iter()
            .any(|p| p.full_id() == playlist.full_id());

    let mut facts = vec![if is_album {
        "Альбом".to_owned()
    } else if playlist.no_discover == Some(true) {
        "Скрытый плейлист".to_owned()
    } else {
        "Плейлист".to_owned()
    }];

    if is_album {
        if let Some(genres) = &playlist.genres
            && let Some(g) = genres.first()
        {
            facts.push(g.name.clone());
        }
        if let Some(year) = playlist.year {
            facts.push(year.to_string());
        }
    } else if let Some(sub) = playlist.subtitle.as_deref().filter(|s| !s.is_empty()) {
        facts.push(sub.to_owned());
    }
    if let Some(plays) = playlist.plays.filter(|p| *p > 0) {
        facts.push(fmt_plays_count(plays));
    }
    if is_downloaded {
        facts.push("Загружено".into());
    }

    let owner_label = if is_album {
        playlist
            .artist_line()
            .or_else(|| playlist.author_name().map(str::to_owned))
            .unwrap_or_default()
    } else {
        playlist
            .author_name()
            .map(str::to_owned)
            .unwrap_or_else(|| {
                if is_own {
                    "Мой плейлист".into()
                } else {
                    String::new()
                }
            })
    };

    let music_owner = if offline_only {
        None
    } else {
        app.core.playlist_details
            .get(collection_id)
            .filter(|d| d.owner_audio_open)
            .and_then(|d| d.owner.clone())
    };

    let byline: Option<AnyElement> = if owner_label.is_empty() {
        None
    } else if is_album
        && let Some((aid, artist_name)) = playlist
            .main_artists
            .as_ref()
            .and_then(|a| a.first())
            .map(|a| (a.id.clone(), a.name.clone()))
    {
        let artist_name = if artist_name.trim().is_empty() {
            owner_label.clone()
        } else {
            artist_name
        };
        Some(
            div()
                .id("col-hero-artist")
                .text_size(px(16.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink)
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_artist(aid.clone(), &artist_name, window, cx);
                }))
                .child(owner_label.clone())
                .into_any_element(),
        )
    } else if let Some(owner) = music_owner {
        Some(
            div()
                .id("col-hero-owner")
                .text_size(px(16.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink)
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_playlist_owner(owner.clone(), window, cx);
                }))
                .child(owner_label.clone())
                .into_any_element(),
        )
    } else {
        Some(
            div()
                .text_size(px(16.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink)
                .child(owner_label.clone())
                .into_any_element(),
        )
    };

    let list_key = format!("col-{}", playlist.full_id());
    let play_tracks = cached_queue(app, &list_key, tracks);
    let shuf_tracks = Rc::clone(&play_tracks);
    let trs_dl = Rc::clone(&play_tracks);
    let title_from = playlist.title.clone();
    let shuf_from = playlist.title.clone();

    let mut left_actions = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(8.0));

    if !tracks.is_empty() {
        left_actions = left_actions
            .child(
                primary_play_pill("col-play", Icon::Play, "Слушать", pal).on_click(cx.listener(
                    move |this, _, _, cx| {
                        if !play_tracks.is_empty() {
                            this.play_tracks(
                                play_tracks.to_vec(),
                                0,
                                PlaybackOrigin::Other,
                                title_from.clone(),
                                None,
                                cx,
                            );
                        }
                    },
                )),
            )
            .child(
                pill_button("col-shuffle", Some(Icon::Shuffle), "Перемешать", false, pal)
                    .h(px(40.0))
                    .px(px(15.0))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !shuf_tracks.is_empty() {
                            this.core.queue.set_shuffled(true);
                            this.play_tracks(
                                shuf_tracks.to_vec(),
                                0,
                                PlaybackOrigin::Other,
                                shuf_from.clone(),
                                None,
                                cx,
                            );
                        }
                    })),
            );
    }

    if !offline_only {
        if playlist.is_editable() {
            let pl_add = playlist.clone();
            left_actions = left_actions.child(
                pill_button(
                    "col-add-tracks",
                    Some(Icon::Plus),
                    "Добавить треки",
                    false,
                    pal,
                )
                .h(px(40.0))
                .px(px(15.0))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_add_tracks_to_playlist(pl_add.clone(), window, cx);
                })),
            );
            let pl_ed = playlist.clone();
            left_actions = left_actions.child(
                pill_button("col-edit", Some(Icon::Edit), "Редактировать", false, pal)
                    .h(px(40.0))
                    .px(px(15.0))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_playlist_editor(Some(pl_ed.clone()), None, window, cx);
                    })),
            );
            let pl_del = playlist.clone();
            left_actions = left_actions.child(
                pill_button("col-del", Some(Icon::Trash), "Удалить", false, pal)
                    .h(px(40.0))
                    .px(px(15.0))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.request_delete_playlist(pl_del.clone(), cx);
                    })),
            );
        } else {
            let pl_fol = playlist.clone();
            left_actions = left_actions.child(
                pill_button(
                    "col-follow",
                    Some(if is_following {
                        Icon::Check
                    } else {
                        Icon::Plus
                    }),
                    if is_following {
                        "В медиатеке"
                    } else {
                        "Добавить в медиатеку"
                    },
                    false,
                    pal,
                )
                .h(px(40.0))
                .px(px(15.0))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_follow_playlist(pl_fol.clone(), cx);
                })),
            );
        }

        if !tracks.is_empty() || is_downloaded {
            let pl_dl = playlist.clone();
            let dl_label = if is_downloaded {
                "Удалить загрузку"
            } else if is_album {
                "Скачать альбом"
            } else {
                "Скачать плейлист"
            };
            left_actions = left_actions.child(
                pill_button("col-dl", Some(Icon::Download), dl_label, false, pal)
                    .h(px(40.0))
                    .px(px(15.0))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_download_collection(pl_dl.clone(), trs_dl.to_vec(), cx);
                    })),
            );
        }
    }

    let col_q = app.collection_query.trim().to_lowercase();
    let filtered_owned: Vec<Audio>;
    let visible_tracks: &[Audio] = if col_q.is_empty() {
        tracks
    } else {
        filtered_owned = tracks
            .iter()
            .filter(|t| {
                t.title.to_lowercase().contains(&col_q)
                    || t.artist.to_lowercase().contains(&col_q)
            })
            .cloned()
            .collect();
        &filtered_owned
    };

    let mut filter_box = div()
        .w(px(220.0))
        .h(px(34.0))
        .px(px(12.0))
        .rounded(px(6.0))
        .bg(pal.fill)
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(icon(Icon::Search, 15.0, pal.ink_2))
        .child(
            div().flex_1().min_w_0().child(
                Input::new(&app.col_filter_input)
                    .appearance(false)
                    .bordered(false)
                    .focus_bordered(false),
            ),
        );
    if !app.collection_query.is_empty() {
        let h = pal.fill_2;
        filter_box = filter_box.child(
            div()
                .id("col-clear-filter")
                .size(px(20.0))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(move |s| s.bg(h))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.clear_collection_filter(window, cx);
                    cx.notify();
                }))
                .child(icon(Icon::Close, 12.0, pal.ink_2)),
        );
    }

    let is_loading = matches!(state, LoadState::Idle | LoadState::Loading) && tracks.is_empty();
    let action_bar: AnyElement = if is_loading {
        render_action_bar_skeleton(
            "col-actions-skel",
            &[(116.0, 40.0), (140.0, 40.0), (166.0, 40.0)],
            true,
            pal,
        )
    } else {
        div()
            .mb(px(22.0))
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(12.0))
            .child(left_actions)
            .children((tracks.len() > 1).then_some(filter_box))
            .into_any_element()
    };

    let total_secs: i64 = tracks.iter().map(|t| t.duration.max(0)).sum();
    let dur_str = fmt_total_duration(total_secs);

    let mut page = div()
        .pt(px(8.0))
        .px(px(32.0))
        .pb(px(40.0))
        .flex()
        .flex_col()
        .child(render_hero_banner(
            HeroArt::Cover {
                local_path: cover_path,
                round: false,
            },
            col_tint,
            &playlist.title,
            byline,
            playlist.description.as_deref(),
            &facts,
            pal,
        ))
        .child(action_bar);

    match state {
        LoadState::Idle | LoadState::Loading if tracks.is_empty() => {
            page = page.child(render_track_list_skeleton(
                "col-tracks-skel",
                8,
                is_album,
                true,
                pal,
            ));
        }
        LoadState::Failed(err) if tracks.is_empty() => {
            let pl_retry = playlist.clone();
            let retry = pill_button("col-retry", None, "Повторить", false, pal)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.load_playlist_detail(pl_retry.clone(), true, cx);
                }))
                .into_any_element();
            page = page.child(render_error_state(
                "Не удалось загрузить плейлист",
                err,
                retry,
                pal,
            ));
        }
        _ if tracks.is_empty() => {
            let add_action = (!offline_only && playlist.is_editable()).then(|| {
                let pl_add = playlist.clone();
                primary_play_pill("col-empty-add", Icon::Plus, "Добавить треки", pal)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_add_tracks_to_playlist(pl_add.clone(), window, cx);
                    }))
                    .into_any_element()
            });
            page = page.child(render_empty_state(
                Icon::Album,
                if is_album {
                    "В этом альбоме нет доступных треков"
                } else {
                    "Плейлист пока пуст"
                },
                if !offline_only && playlist.is_editable() {
                    "Добавьте треки из Моей музыки, других плейлистов или поиска ВКонтакте"
                } else {
                    "Список аудиозаписей пуст или недоступен"
                },
                add_action,
                pal,
            ));
        }
        _ if !col_q.is_empty() && visible_tracks.is_empty() => {
            let reset = pill_button("col-reset-filter", None, "Сбросить фильтр", false, pal)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.clear_collection_filter(window, cx);
                    cx.notify();
                }))
                .into_any_element();
            page = page.child(render_empty_state(
                Icon::Search,
                format!("По фильтру «{}» ничего не найдено", app.collection_query.trim()),
                "Попробуйте другой запрос или сбросьте фильтр",
                Some(reset),
                pal,
            ));
        }
        _ => {
            let footer_text = if dur_str.is_empty() {
                fmt_tracks_count(tracks.len() as i64)
            } else {
                format!("{}, {dur_str}", fmt_tracks_count(tracks.len() as i64))
            };
            page = page
                .child(render_track_list(
                    app,
                    visible_tracks,
                    TrackTableOptions {
                        id_prefix: list_key,
                        is_album,
                        album_artist: is_album.then_some(owner_label),
                        hide_album: is_album,
                        no_header: false,
                        from_label: playlist.title.clone(),
                        origin: PlaybackOrigin::Other,
                        editable_playlist: (!offline_only && playlist.is_editable())
                            .then(|| playlist.clone()),
                    },
                    pal,
                    cx,
                ))
                .child(
                    div()
                        .pt(px(14.0))
                        .px(px(10.0))
                        .text_size(px(13.5))
                        .text_color(pal.ink_2)
                        .child(footer_text),
                );
        }
    }

    if !offline_only
        && let Some(detail) = app.core.playlist_details.get(collection_id)
        && matches!(detail.state, LoadState::Loaded)
        && (detail.tracks.len() as i64) < detail.total_count
    {
        let key = collection_id.to_owned();
        if detail.more_failed {
            page = page.child(div().mt(px(18.0)).flex().justify_center().child(
                pill_button("col-load-retry", None, "Повторить", false, pal).on_click(
                    cx.listener(move |this, _, _, cx| this.load_more_playlist_tracks(&key, cx)),
                ),
            ));
        } else if detail.loading_more {
            page = page.child(
                div()
                    .mt(px(18.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .text_size(px(14.0))
                    .text_color(pal.ink_2)
                    .child(spinner("col-more-spin", 16.0, pal.hi))
                    .child("Загрузка…"),
            );
        } else if app.canvas_near_bottom() {
            cx.spawn(async move |this, cx| {
                let _ = this.update(cx, |this, cx| this.load_more_playlist_tracks(&key, cx));
            })
            .detach();
        }
    }

    page.into_any_element()
}
