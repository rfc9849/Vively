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
        router::{ActiveModal, Route},
        runtime::fmt_tracks_count,
    },
    ui::components::{
        HeroArt, TrackTableOptions, pill_button, primary_play_pill, render_action_bar_skeleton,
        render_empty_state, render_error_state, render_hero_banner, render_music_page_skeleton,
        render_playlist_shelf, render_track_list,
    },
};

pub fn render_library_page(
    app: &VivelyApp,
    center_w: f32,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let q = app.library_query.trim();
    let filtered_owned: Vec<Audio>;
    let filtered: &[Audio] = if q.is_empty() {
        &app.core.library_tracks
    } else {
        filtered_owned = app.filtered_library_tracks();
        &filtered_owned
    };

    let loading_initial = matches!(app.core.library_state, LoadState::Idle | LoadState::Loading)
        && app.core.library_tracks.is_empty();
    let total = app.core.library_total.max(app.core.library_tracks.len() as i64);
    let facts = if loading_initial {
        Vec::new()
    } else {
        vec![fmt_tracks_count(total)]
    };

    let action_bar = div()
        .mb(px(22.0))
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(8.0))
                .child(
                    primary_play_pill("lib-play", Icon::Play, "Слушать", pal).on_click(
                        cx.listener(move |this, _, _, cx| {
                            let tracks = this.filtered_library_tracks();
                            if !tracks.is_empty() {
                                this.play_tracks(
                                    tracks,
                                    0,
                                    PlaybackOrigin::Library,
                                    "Моя музыка",
                                    None,
                                    cx,
                                );
                            }
                        }),
                    ),
                )
                .child(
                    pill_button("lib-shuffle", Some(Icon::Shuffle), "Перемешать", false, pal)
                        .h(px(40.0))
                        .px(px(15.0))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let tracks = this.filtered_library_tracks();
                            if !tracks.is_empty() {
                                this.core.queue.set_shuffled(true);
                                this.play_tracks(
                                    tracks,
                                    0,
                                    PlaybackOrigin::Library,
                                    "Моя музыка",
                                    None,
                                    cx,
                                );
                            }
                        })),
                )
                .child(
                    pill_button(
                        "lib-upload",
                        Some(Icon::Upload),
                        "Загрузить аудио",
                        false,
                        pal,
                    )
                    .h(px(40.0))
                    .px(px(15.0))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.pick_audio_files_for_upload(window, cx);
                    })),
                )
                .child(
                    pill_button(
                        "lib-bulk-dl",
                        Some(Icon::Download),
                        "Скачать медиатеку",
                        false,
                        pal,
                    )
                    .h(px(40.0))
                    .px(px(15.0))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.router.modal = Some(ActiveModal::BulkDownloadLibrary);
                        cx.notify();
                    })),
                ),
        )
        .child({
            let mut filter = div()
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
                        Input::new(&app.lib_filter_input)
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false),
                    ),
                );
            if !app.library_query.is_empty() {
                let h = pal.fill_2;
                filter = filter.child(
                    div()
                        .id("lib-clear-filter")
                        .size(px(20.0))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(move |s| s.bg(h))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.clear_library_filter(window, cx);
                        }))
                        .child(icon(Icon::Close, 12.0, pal.ink_2)),
                );
            }
            filter
        });

    let mut page = div()
        .pt(px(8.0))
        .px(px(32.0))
        .pb(px(40.0))
        .flex()
        .flex_col()
        .child(render_hero_banner(
            HeroArt::Liked,
            None,
            "Моя музыка",
            None,
            None,
            &facts,
            pal,
        ));

    if loading_initial {
        page = page.child(render_action_bar_skeleton(
            "lib-actions-skel",
            &[(116.0, 40.0), (140.0, 40.0), (158.0, 40.0), (176.0, 40.0)],
            true,
            pal,
        ));
    } else if !app.core.library_tracks.is_empty() {
        page = page.child(action_bar);
    }

    if !app.core.playlists.is_empty() {
        page = page.child(div().mb(px(40.0)).child(render_playlist_shelf(
            app,
            "libshelf",
            "Плейлисты и альбомы",
            &app.core.playlists,
            app.core.playlists.len(),
            app.library_playlists_expanded,
            center_w,
            |this, _| this.library_playlists_expanded = !this.library_playlists_expanded,
            pal,
            cx,
        )));
    }

    if !app.core.library_tracks.is_empty() {
        page = page.child(
            div()
                .mb(px(14.0))
                .text_size(px(20.0))
                .font_weight(FontWeight::BOLD)
                .text_color(pal.ink)
                .child("Треки"),
        );
    }

    if let Some(restorable) = &app.core.restorable_own_track {
        page = page.child(
            div()
                .mb(px(14.0))
                .py(px(8.0))
                .pl(px(14.0))
                .pr(px(8.0))
                .rounded(px(6.0))
                .bg(pal.fill)
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(pal.ink)
                        .child(format!("Трек «{}» удалён из Моей музыки", restorable.title)),
                )
                .child(
                    pill_button("restore-own-btn", None, "Восстановить", false, pal)
                        .on_click(cx.listener(|this, _, _, cx| this.restore_own_removed_track(cx))),
                ),
        );
    }

    if loading_initial {
        let shelf_cards = if app.core.playlists.is_empty() { 4 } else { 0 };
        page = page.child(render_music_page_skeleton(
            "lib-init-skel",
            shelf_cards,
            8,
            false,
            pal,
        ));
    } else if let LoadState::Failed(err) = &app.core.library_state
        && app.core.library_tracks.is_empty()
    {
        let retry = pill_button("lib-init-retry", None, "Повторить", false, pal)
            .on_click(cx.listener(|this, _, _, cx| this.load_library(true, cx)))
            .into_any_element();
        page = page.child(render_error_state(
            "Не удалось загрузить медиатеку",
            err.clone(),
            retry,
            pal,
        ));
    } else if app.core.library_tracks.is_empty() {
        let actions = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_center()
            .gap(px(10.0))
            .child(
                primary_play_pill("lib-empty-home", Icon::Home, "Открыть Главную", pal).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.navigate_to(Route::Home, window, cx);
                    }),
                ),
            )
            .child(
                pill_button(
                    "lib-empty-upload",
                    Some(Icon::Upload),
                    "Загрузить аудио",
                    false,
                    pal,
                )
                .h(px(40.0))
                .px(px(16.0))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.pick_audio_files_for_upload(window, cx);
                })),
            )
            .into_any_element();
        page = page.child(render_empty_state(
            Icon::Heart,
            "В «Моей музыке» пока пусто",
            "Добавляйте понравившиеся треки из рекомендаций и поиска или загрузите свои аудиофайлы",
            Some(actions),
            pal,
        ));
    } else {
        if !q.is_empty() && filtered.is_empty() {
            let q_vk = q.to_owned();
            let mut actions = div()
                .flex()
                .flex_wrap()
                .justify_center()
                .gap(px(8.0));
            if app.core.library_can_more {
                actions = actions.child(
                    pill_button(
                        "lib-filter-load-more-empty",
                        None,
                        if app.core.library_loading_more {
                            "Загрузка…"
                        } else {
                            "Загрузить ещё треки"
                        },
                        false,
                        pal,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.load_more_library(cx))),
                );
            }
            actions = actions.child(
                pill_button(
                    "lib-filter-vk-empty",
                    Some(Icon::Search),
                    format!("Искать «{q}» в VK"),
                    false,
                    pal,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.search_in_vk(q_vk.clone(), window, cx);
                })),
            );
            page = page.child(render_empty_state(
                Icon::Search,
                format!("По фильтру «{q}» ничего не найдено"),
                "Среди загруженных треков совпадений нет",
                Some(actions.into_any_element()),
                pal,
            ));
        } else {
            page = page.child(render_track_list(
                app,
                filtered,
                TrackTableOptions {
                    id_prefix: "lib".into(),
                    is_album: false,
                    album_artist: None,
                    hide_album: false,
                    no_header: false,
                    from_label: "Моя музыка".into(),
                    origin: PlaybackOrigin::Library,
                    editable_playlist: None,
                },
                pal,
                cx,
            ));

            if !q.is_empty() {
                let q_vk = q.to_owned();
                let loaded = app.core.library_tracks.len();
                let mut bar_actions = div().flex().items_center().gap(px(8.0));
                if app.core.library_can_more {
                    bar_actions = bar_actions.child(
                        pill_button(
                            "lib-filter-load-more",
                            None,
                            if app.core.library_loading_more {
                                "Загрузка…"
                            } else {
                                "Загрузить ещё"
                            },
                            false,
                            pal,
                        )
                        .h(px(32.0))
                        .on_click(cx.listener(|this, _, _, cx| this.load_more_library(cx))),
                    );
                }
                bar_actions = bar_actions.child(
                    pill_button(
                        "lib-filter-vk",
                        Some(Icon::Search),
                        format!("Искать «{q}» в VK"),
                        false,
                        pal,
                    )
                    .h(px(32.0))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.search_in_vk(q_vk.clone(), window, cx);
                    })),
                );
                page = page.child(
                    div()
                        .mt(px(16.0))
                        .py(px(10.0))
                        .px(px(14.0))
                        .rounded(px(8.0))
                        .bg(pal.fill)
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap(px(10.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(pal.ink_2)
                                .child(if app.core.library_can_more {
                                    format!("Поиск среди загруженных {loaded} из {total}")
                                } else {
                                    format!("Найдено: {}", fmt_tracks_count(filtered.len() as i64))
                                }),
                        )
                        .child(bar_actions),
                );
            }
        }

        if matches!(app.core.library_state, LoadState::Loaded) && app.core.library_can_more && q.is_empty() {
            if app.core.library_more_failed {
                page = page.child(
                    div().mt(px(18.0)).flex().justify_center().child(
                        pill_button("lib-load-retry", None, "Повторить", false, pal)
                            .on_click(cx.listener(|this, _, _, cx| this.load_more_library(cx))),
                    ),
                );
            } else if app.core.library_loading_more {
                page = page.child(
                    div()
                        .mt(px(18.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(px(8.0))
                        .text_size(px(14.0))
                        .text_color(pal.ink_2)
                        .child(spinner("lib-more-spin", 16.0, pal.hi))
                        .child("Загрузка…"),
                );
            } else if app.canvas_near_bottom() {
                cx.spawn(async move |this, cx| {
                    let _ = this.update(cx, |this, cx| this.load_more_library(cx));
                })
                .detach();
            }
        }
    }

    page.into_any_element()
}
