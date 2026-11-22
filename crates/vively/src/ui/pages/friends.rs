use std::rc::Rc;

use gpui_kit::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, component::input::Input, div, px, relative,
};
use vively_core::playback::PlaybackOrigin;

use crate::{
    design::{
        artwork::{make_initials, render_avatar},
        colors::Palette,
        icons::{Icon, icon, spinner},
    },
    state::{
        app_state::VivelyApp,
        models::{LoadState, ProfileSearchData},
        runtime::{fmt_friends_count, fmt_playlists_count, fmt_tracks_count},
    },
    ui::components::{
        HeroArt, TrackTableOptions, cached_queue, pill_button, primary_play_pill,
        render_action_bar_skeleton, render_empty_state, render_error_state,
        render_friends_grid_skeleton, render_hero_banner, render_music_page_skeleton,
        render_playlist_card, render_playlist_shelf, render_track_list, skeleton_bar,
    },
};

pub fn render_friends_page(
    app: &VivelyApp,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let loading_initial =
        matches!(app.core.friends_state, LoadState::Idle | LoadState::Loading) && app.core.friends.is_empty();
    let count = app.core.friends_total.max(app.core.friends.len() as i64);
    let hover_bg = pal.hover;

    let mut page = div()
        .pt(px(8.0))
        .px(px(32.0))
        .pb(px(40.0))
        .flex()
        .flex_col()
        .child(
            div()
                .mt(px(10.0))
                .mb(px(8.0))
                .text_size(px(30.0))
                .font_weight(FontWeight::BOLD)
                .line_height(relative(1.15))
                .text_color(pal.ink)
                .child("Друзья"),
        );

    if !loading_initial && count > 0 {
        page = page.child(
            div()
                .mb(px(28.0))
                .text_size(px(15.0))
                .text_color(pal.ink_2)
                .child(fmt_friends_count(count)),
        );
    }

    if loading_initial {
        return page
            .child(div().mb(px(28.0)).child(skeleton_bar(96.0, 14.0, pal)))
            .child(render_friends_grid_skeleton("friends-grid-skel", 12, pal))
            .into_any_element();
    }

    if let LoadState::Failed(err) = &app.core.friends_state
        && app.core.friends.is_empty()
    {
        let retry = pill_button("friends-retry", None, "Повторить", false, pal)
            .on_click(cx.listener(|this, _, _, cx| this.load_friends(true, cx)))
            .into_any_element();
        return page
            .child(render_error_state(
                "Не удалось загрузить список друзей",
                err.clone(),
                retry,
                pal,
            ))
            .into_any_element();
    }

    if app.core.friends.is_empty() {
        return page
            .child(render_empty_state(
                Icon::Users,
                "Список друзей пуст",
                "Когда появятся друзья ВКонтакте, их музыка и плейлисты будут доступны здесь",
                None,
                pal,
            ))
            .into_any_element();
    }

    let mut grid = div().flex().flex_wrap().gap(px(24.0));
    for (idx, friend) in app.core.friends.iter().enumerate() {
        let avatar_path = app.resolve_artwork(friend.photo_base.as_ref(), cx);
        let initials = make_initials(&friend.first_name, &friend.last_name);
        let deactivated = friend.deactivated.is_some();
        let fr_clone = friend.clone();

        grid = grid.child(
            div()
                .id(format!("friend-card-{idx}-{}", friend.id))
                .w(px(136.0))
                .p(px(10.0))
                .rounded(px(10.0))
                .flex()
                .flex_col()
                .items_center()
                .cursor_pointer()
                .opacity(if deactivated { 0.4 } else { 1.0 })
                .hover(move |s| s.bg(hover_bg))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_friend(fr_clone.clone(), window, cx);
                }))
                .child(div().mb(px(10.0)).child(render_avatar(
                    avatar_path,
                    friend.id,
                    &initials,
                    116.0,
                    pal.art_ring,
                )))
                .child(
                    div()
                        .w_full()
                        .text_center()
                        .text_size(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(pal.ink)
                        .truncate()
                        .child(friend.full_name()),
                ),
        );
    }

    page.child(grid).into_any_element()
}

pub fn render_friend_music_page(
    app: &VivelyApp,
    friend_id: i64,
    center_w: f32,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let page_data = app.core.friend_pages.get(&friend_id);
    let Some(data) = page_data else {
        return div()
            .pt(px(8.0))
            .px(px(32.0))
            .pb(px(40.0))
            .flex()
            .flex_col()
            .child(render_action_bar_skeleton(
                "fr-init-actions-skel",
                &[(116.0, 40.0), (140.0, 40.0)],
                false,
                pal,
            ))
            .child(render_music_page_skeleton("fr-init-skel", 4, 8, false, pal))
            .into_any_element();
    };

    let f = &data.friend;
    let full_name = f.full_name();
    let initials = make_initials(&f.first_name, &f.last_name);
    let avatar_path = app.resolve_artwork(f.photo_base.as_ref(), cx);
    let tint = app.tint_for_artwork(f.photo_base.as_ref(), None);

    if f.deactivated.is_some() || (f.is_closed == Some(true) && f.can_access_closed != Some(true)) {
        let msg = if f.deactivated.is_some() {
            "Страница удалена".to_owned()
        } else {
            "Нет доступа к аудиозаписям закрытого профиля".to_owned()
        };
        return div()
            .pt(px(8.0))
            .px(px(32.0))
            .pb(px(40.0))
            .child(render_hero_banner(
                HeroArt::Avatar {
                    local_path: avatar_path,
                    seed_id: f.id,
                    initials,
                },
                tint,
                &full_name,
                None,
                None,
                &[msg],
                pal,
            ))
            .into_any_element();
    }

    let is_loading = matches!(data.state, LoadState::Idle | LoadState::Loading)
        && data.tracks.is_empty()
        && data.playlists.is_empty();
    let facts = if is_loading {
        Vec::new()
    } else {
        vec![
            fmt_tracks_count(data.total_tracks),
            fmt_playlists_count(data.total_playlists.max(0) as usize),
        ]
    };

    let mut page = div()
        .pt(px(8.0))
        .px(px(32.0))
        .pb(px(40.0))
        .flex()
        .flex_col()
        .child(render_hero_banner(
            HeroArt::Avatar {
                local_path: avatar_path,
                seed_id: f.id,
                initials,
            },
            tint,
            &full_name,
            None,
            None,
            &facts,
            pal,
        ));

    if is_loading {
        return page
            .child(render_action_bar_skeleton(
                "fr-actions-skel",
                &[(116.0, 40.0), (140.0, 40.0)],
                false,
                pal,
            ))
            .child(render_music_page_skeleton("fr-music-skel", 4, 8, false, pal))
            .into_any_element();
    }

    if let LoadState::Failed(err) = &data.state
        && data.tracks.is_empty()
        && data.playlists.is_empty()
    {
        let fr_retry = f.clone();
        let retry = pill_button("fr-retry", None, "Повторить", false, pal)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.load_friend_music(fr_retry.clone(), true, cx);
            }))
            .into_any_element();
        return page
            .child(render_error_state(
                "Не удалось загрузить аудиозаписи",
                err.clone(),
                retry,
                pal,
            ))
            .into_any_element();
    }

    if data.tracks.is_empty() && data.playlists.is_empty() && data.playlists_error.is_none() {
        return page
            .child(render_empty_state(
                Icon::Album,
                "Нет доступной музыки",
                "У пользователя пока нет аудиозаписей или доступ к ним ограничен настройками приватности",
                None,
                pal,
            ))
            .into_any_element();
    }

    let list_key = format!("fr-tr-{friend_id}");
    let mut actions = div().flex().flex_wrap().items_center().gap(px(8.0));
    if !data.tracks.is_empty() {
        let play_tracks = cached_queue(app, &list_key, &data.tracks);
        let shuf_tracks = Rc::clone(&play_tracks);
        let from_play = full_name.clone();
        let from_shuf = full_name.clone();
        actions = actions
            .child(
                primary_play_pill("fr-play", Icon::Play, "Слушать", pal).on_click(cx.listener(
                    move |this, _, _, cx| {
                        if !play_tracks.is_empty() {
                            this.play_tracks(
                                play_tracks.to_vec(),
                                0,
                                PlaybackOrigin::Other,
                                from_play.clone(),
                                None,
                                cx,
                            );
                        }
                    },
                )),
            )
            .child(
                pill_button("fr-shuf", Some(Icon::Shuffle), "Перемешать", false, pal)
                    .h(px(40.0))
                    .px(px(15.0))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !shuf_tracks.is_empty() {
                            this.core.queue.set_shuffled(true);
                            this.play_tracks(
                                shuf_tracks.to_vec(),
                                0,
                                PlaybackOrigin::Other,
                                from_shuf.clone(),
                                None,
                                cx,
                            );
                        }
                    })),
            );
    }
    if app.signed_in_user_id() != Some(friend_id) {
        let followed = app.core.followed_owners.contains(&friend_id);
        actions = actions.child(
            pill_button(
                "fr-follow",
                Some(if followed { Icon::Check } else { Icon::Plus }),
                if followed {
                    "Вы подписаны"
                } else {
                    "Подписаться на музыку"
                },
                followed,
                pal,
            )
            .h(px(40.0))
            .px(px(15.0))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_owner_follow(friend_id, cx);
            })),
        );
    }
    let search_box = div()
        .w(px(240.0))
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
                Input::new(&app.profile_search_input)
                    .appearance(false)
                    .bordered(false)
                    .focus_bordered(false),
            ),
        );
    page = page.child(
        div()
            .mb(px(22.0))
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(12.0))
            .child(actions)
            .child(search_box),
    );

    if let Some(search) = app
        .core.profile_search
        .as_ref()
        .filter(|s| s.owner_id == friend_id)
    {
        return page
            .child(render_profile_search(app, search, &full_name, pal, cx))
            .into_any_element();
    }

    if !data.playlists.is_empty() {
        let expanded = app.friend_playlists_expanded;
        let mut shelf = div()
            .mb(px(40.0))
            .flex()
            .flex_col()
            .child(render_playlist_shelf(
                app,
                "fr-pl",
                "Плейлисты",
                &data.playlists,
                data.total_playlists.max(0) as usize,
                expanded,
                center_w,
                |this, _| this.friend_playlists_expanded = !this.friend_playlists_expanded,
                pal,
                cx,
            ));
        if expanded && (data.playlists.len() as i64) < data.total_playlists {
            shelf = shelf.child(if data.playlists_loading_more {
                div()
                    .mt(px(18.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .text_size(px(14.0))
                    .text_color(pal.ink_2)
                    .child(spinner("fr-pl-more-spin", 16.0, pal.hi))
                    .child("Загрузка…")
                    .into_any_element()
            } else {
                div()
                    .mt(px(18.0))
                    .flex()
                    .justify_center()
                    .child(
                        pill_button("fr-pl-more", None, "Загрузить ещё", false, pal).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.load_more_friend_playlists(friend_id, cx);
                            }),
                        ),
                    )
                    .into_any_element()
            });
        }
        page = page.child(shelf);
    } else if let Some(err) = &data.playlists_error {
        let fr_retry = f.clone();
        page = page
            .child(
                div()
                    .mb(px(14.0))
                    .text_size(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(pal.ink)
                    .child("Плейлисты"),
            )
            .child(
                div()
                    .mb(px(40.0))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .min_w_0()
                            .text_size(px(14.0))
                            .text_color(pal.ink_2)
                            .child(format!("Не удалось загрузить плейлисты: {err}")),
                    )
                    .child(
                        pill_button("fr-pl-retry", None, "Повторить", false, pal).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.load_friend_music(fr_retry.clone(), true, cx);
                            }),
                        ),
                    ),
            );
    }

    if !data.tracks.is_empty() {
        page = page
            .child(
                div()
                    .mb(px(14.0))
                    .text_size(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(pal.ink)
                    .child("Треки"),
            )
            .child(render_track_list(
                app,
                &data.tracks,
                TrackTableOptions {
                    id_prefix: list_key,
                    is_album: false,
                    album_artist: None,
                    hide_album: false,
                    no_header: false,
                    from_label: full_name,
                    origin: PlaybackOrigin::Other,
                    editable_playlist: None,
                },
                pal,
                cx,
            ));
    }

    if (data.tracks.len() as i64) < data.total_tracks {
        if data.more_failed {
            page = page.child(div().mt(px(18.0)).flex().justify_center().child(
                pill_button("fr-load-retry", None, "Повторить", false, pal).on_click(
                    cx.listener(move |this, _, _, cx| this.load_more_friend_tracks(friend_id, cx)),
                ),
            ));
        } else if data.loading_more {
            page = page.child(
                div()
                    .mt(px(18.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .text_size(px(14.0))
                    .text_color(pal.ink_2)
                    .child(spinner("fr-more-spin", 16.0, pal.hi))
                    .child("Загрузка…"),
            );
        } else if app.canvas_near_bottom() {
            cx.spawn(async move |this, cx| {
                let _ = this.update(cx, |this, cx| this.load_more_friend_tracks(friend_id, cx));
            })
            .detach();
        }
    }

    page.into_any_element()
}

fn render_profile_search(
    app: &VivelyApp,
    search: &ProfileSearchData,
    owner_name: &str,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    match &search.state {
        LoadState::Idle | LoadState::Loading => {
            return div()
                .mt(px(18.0))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(8.0))
                .text_size(px(14.0))
                .text_color(pal.ink_2)
                .child(spinner("fr-search-spin", 16.0, pal.hi))
                .child("Поиск…")
                .into_any_element();
        }
        LoadState::Failed(err) => {
            return div()
                .text_size(px(14.0))
                .text_color(pal.ink_2)
                .child(format!("Не удалось выполнить поиск: {err}"))
                .into_any_element();
        }
        LoadState::Loaded => {}
    }

    let results = &search.results;
    if results.audios.is_empty() && results.playlists.items.is_empty() {
        return render_empty_state(
            Icon::Search,
            format!("По запросу «{}» ничего не найдено", search.query),
            "Попробуйте другой запрос",
            None,
            pal,
        );
    }

    let mut col = div().flex().flex_col();
    if !results.playlists.items.is_empty() {
        col = col
            .child(
                div()
                    .mb(px(14.0))
                    .text_size(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(pal.ink)
                    .child("Плейлисты"),
            )
            .child(
                div()
                    .mb(px(40.0))
                    .flex()
                    .flex_wrap()
                    .gap_x(px(18.0))
                    .gap_y(px(22.0))
                    .children(
                        results.playlists.items.iter().map(|pl| {
                            render_playlist_card(app, "fr-search-pl", pl, false, pal, cx)
                        }),
                    ),
            );
    }
    if !results.audios.is_empty() {
        col = col
            .child(
                div()
                    .mb(px(14.0))
                    .text_size(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(pal.ink)
                    .child("Треки"),
            )
            .child(render_track_list(
                app,
                &results.audios,
                TrackTableOptions {
                    id_prefix: format!("fr-search-{}", search.owner_id),
                    is_album: false,
                    album_artist: None,
                    hide_album: false,
                    no_header: false,
                    from_label: owner_name.to_owned(),
                    origin: PlaybackOrigin::Other,
                    editable_playlist: None,
                },
                pal,
                cx,
            ));
    }
    col.into_any_element()
}
