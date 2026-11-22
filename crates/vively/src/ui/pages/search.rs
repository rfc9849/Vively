use gpui_kit::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, div, px, relative,
};
use vively_core::playback::PlaybackOrigin;

use crate::{
    design::{
        colors::Palette,
        icons::{Icon, icon},
    },
    state::{app_state::VivelyApp, models::LoadState},
    ui::{
        components::{
            TrackTableOptions, pill_button, render_artist_card, render_empty_state,
            render_error_state, render_playlist_card, render_search_skeleton, render_track_list,
        },
        pages::home::render_music_section_block,
    },
};

pub fn render_search_page(
    app: &VivelyApp,
    center_w: f32,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let q = app.core.search_query.trim().to_owned();

    if q.is_empty() {
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
                    .child("Поиск"),
            )
            .child(
                div()
                    .mb(px(28.0))
                    .text_size(px(15.0))
                    .text_color(pal.ink_2)
                    .child("Треки, альбомы, плейлисты и исполнители"),
            );

        if app.core.search_history.entries.is_empty() {
            page = page.child(render_empty_state(
                Icon::Search,
                "Найдите любимую музыку",
                "Введите название трека, альбома, плейлиста или имя исполнителя в строке поиска выше",
                None,
                pal,
            ));
        } else {
            let hover_ink = pal.ink;
            let fill_2 = pal.fill_2;
            let mut chips = div()
                .mb(px(24.0))
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(6.0));
            for (idx, item) in app.core.search_history.entries.iter().enumerate() {
                let pick_q = item.clone();
                let del_q = item.clone();
                chips = chips.child(
                    div()
                        .h(px(32.0))
                        .pl(px(12.0))
                        .pr(px(4.0))
                        .rounded(px(6.0))
                        .bg(pal.fill)
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .child(
                            div()
                                .id(format!("hist-pick-{idx}"))
                                .text_size(px(14.0))
                                .text_color(pal.ink)
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.pick_search_history(pick_q.clone(), window, cx);
                                }))
                                .child(item.clone()),
                        )
                        .child(
                            div()
                                .id(format!("hist-del-{idx}"))
                                .size(px(24.0))
                                .rounded(px(4.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(move |s| s.bg(fill_2).text_color(hover_ink))
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.delete_search_history(&del_q, cx);
                                }))
                                .child(icon(Icon::Close, 13.0, pal.ink_2)),
                        ),
                );
            }
            chips = chips.child(
                pill_button("clear-search-hist", None, "Очистить", false, pal)
                    .on_click(cx.listener(|this, _, _, cx| this.clear_all_search_history(cx))),
            );
            page = page.child(chips);
        }

        return page.into_any_element();
    }

    let mut page = div()
        .pt(px(8.0))
        .px(px(32.0))
        .pb(px(40.0))
        .flex()
        .flex_col()
        .child(
            div()
                .mt(px(10.0))
                .mb(px(24.0))
                .text_size(px(30.0))
                .font_weight(FontWeight::BOLD)
                .line_height(relative(1.15))
                .text_color(pal.ink)
                .child(format!("По запросу «{q}»")),
        );

    match &app.core.search_state {
        LoadState::Idle | LoadState::Loading if app.core.search_results.is_empty() => {
            return page
                .child(render_search_skeleton("search-loading-skel", pal))
                .into_any_element();
        }
        LoadState::Failed(err) => {
            let retry = pill_button("search-retry", None, "Повторить", false, pal)
                .on_click(cx.listener(|this, _, window, cx| this.commit_search_now(window, cx)))
                .into_any_element();
            return page
                .child(render_error_state(
                    "Не удалось выполнить поиск",
                    err.clone(),
                    retry,
                    pal,
                ))
                .into_any_element();
        }
        _ if app.core.search_results.is_empty() => {
            return page
                .child(render_empty_state(
                    Icon::Search,
                    format!("По запросу «{q}» ничего не найдено"),
                    "Проверьте раскладку или попробуйте написать название латиницей",
                    None,
                    pal,
                ))
                .into_any_element();
        }
        _ => {}
    }

    if !app.core.search_results.tracks.is_empty() {
        page = page
            .child(
                div()
                    .mb(px(14.0))
                    .text_size(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(pal.ink)
                    .child("Треки"),
            )
            .child(div().mb(px(32.0)).child(render_track_list(
                app,
                &app.core.search_results.tracks,
                TrackTableOptions {
                    id_prefix: "search-tr".into(),
                    is_album: false,
                    album_artist: None,
                    hide_album: false,
                    no_header: false,
                    from_label: format!("Поиск: {q}"),
                    origin: PlaybackOrigin::Other,
                    editable_playlist: None,
                },
                pal,
                cx,
            )));
    }

    if !app.core.search_results.artists.is_empty() {
        page = page
            .child(
                div()
                    .mb(px(14.0))
                    .text_size(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(pal.ink)
                    .child("Исполнители"),
            )
            .child(
                div()
                    .mb(px(40.0))
                    .flex()
                    .flex_wrap()
                    .gap_x(px(18.0))
                    .gap_y(px(22.0))
                    .children(
                        app.core.search_results
                            .artists
                            .iter()
                            .enumerate()
                            .map(|(i, ar)| {
                                render_artist_card(
                                    app,
                                    format!("search-ar-{i}-{}", ar.id),
                                    &ar.id,
                                    &ar.name,
                                    ar.avatar.as_ref(),
                                    pal,
                                    cx,
                                )
                            }),
                    ),
            );
    }

    if !app.core.search_results.playlists.is_empty() {
        page = page
            .child(
                div()
                    .mb(px(14.0))
                    .text_size(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(pal.ink)
                    .child("Плейлисты и альбомы"),
            )
            .child(
                div()
                    .mb(px(40.0))
                    .flex()
                    .flex_wrap()
                    .gap_x(px(18.0))
                    .gap_y(px(22.0))
                    .children(
                        app.core.search_results
                            .playlists
                            .iter()
                            .map(|pl| render_playlist_card(app, "search-pl", pl, false, pal, cx)),
                    ),
            );
    }

    for (idx, sec) in app.core.search_results.sections.iter().enumerate() {
        page = page.child(render_music_section_block(
            app,
            &format!("search-sec-{idx}"),
            sec,
            center_w,
            pal,
            cx,
        ));
    }

    page.into_any_element()
}
