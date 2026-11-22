use gpui_kit::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement as _, StatefulInteractiveElement as _, Styled as _,
    WindowControlArea, component::scroll::ScrollableElement as _, div, px,
};

use crate::{
    design::{
        artwork::render_cover,
        colors::Palette,
        icons::{Icon, icon},
    },
    state::{
        app_state::VivelyApp,
        models::LoadState,
        router::{ContextMenuKind, Route},
    },
    ui::components::{icon_button, render_sidebar_playlists_skeleton},
};

pub fn render_left_rail(
    app: &VivelyApp,
    width_px: f32,
    is_fullscreen: bool,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let active_nav = app.router.current().nav_highlight();
    let uid = app.signed_in_user_id().unwrap_or(0);

    let nav_items: [(&str, Route, Icon, &str); 5] = [
        ("home", Route::Home, Icon::Home, "Главная"),
        ("search", Route::Search, Icon::Search, "Поиск"),
        ("library", Route::Library, Icon::Heart, "Моя музыка"),
        ("friends", Route::Friends, Icon::Users, "Друзья"),
        ("downloads", Route::Downloads, Icon::Download, "Загруженное"),
    ];

    let active_pill_bg = if pal.is_neutral {
        pal.active
    } else {
        pal.hi.opacity(0.14)
    };

    let mut nav_col = div().flex().flex_col().gap(px(2.0));
    for (key, route, ic, label) in nav_items {
        let is_active = active_nav == Some(key);
        let icon_kind = if is_active && ic == Icon::Heart {
            Icon::HeartFill
        } else {
            ic
        };
        let fg = if is_active { pal.ink } else { pal.ink_2 };
        let ic_fg = if is_active { pal.hi } else { pal.ink_2 };
        let bg = if is_active {
            active_pill_bg
        } else {
            gpui_kit::transparent_black()
        };
        let hover_bg = if is_active { active_pill_bg } else { pal.hover };
        let hover_fg = pal.ink;

        let btn = div()
            .id(format!("nav-{key}"))
            .h(px(38.0))
            .px(px(12.0))
            .rounded(px(8.0))
            .bg(bg)
            .flex()
            .items_center()
            .gap(px(12.0))
            .cursor_pointer()
            .text_color(fg)
            .hover(move |s| s.bg(hover_bg).text_color(hover_fg))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.navigate_to(route.clone(), window, cx);
            }))
            .child(icon(icon_kind, 19.0, ic_fg))
            .child(
                div()
                    .text_size(px(14.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(label),
            );
        nav_col = nav_col.child(btn);
    }

    let mut pl_list = div().flex().flex_col().gap(px(2.0));
    if app.core.playlists.is_empty() {
        if matches!(app.core.playlists_state, LoadState::Idle | LoadState::Loading) {
            pl_list = pl_list.child(render_sidebar_playlists_skeleton("rail-pl-skel", 5, pal));
        } else if matches!(app.core.playlists_state, LoadState::Loaded) {
            pl_list = pl_list.child(
                div()
                    .px(px(12.0))
                    .py(px(10.0))
                    .text_size(px(13.0))
                    .text_color(pal.ink_3)
                    .child("Пока нет плейлистов"),
            );
        }
    }
    for (idx, pl) in app.core.playlists.iter().enumerate() {
        let is_active =
            matches!(app.router.current(), Route::Collection(cid) if cid == &pl.stable_id());
        let cover_path = app.resolve_artwork(pl.artwork_url(), cx);
        let sub = if pl.owner_id == uid {
            "Мой плейлист".to_owned()
        } else {
            pl.author_name()
                .map(str::to_owned)
                .unwrap_or_else(|| pl.byline())
        };
        let pl_open = pl.clone();
        let pl_ctx = pl.clone();
        let bg = if is_active {
            active_pill_bg
        } else {
            gpui_kit::transparent_black()
        };
        let hover_bg = if is_active { active_pill_bg } else { pal.hover };

        pl_list = pl_list.child(
            div()
                .id(format!("rail-pl-{idx}-{}", pl.stable_id()))
                .px(px(8.0))
                .py(px(5.0))
                .rounded(px(8.0))
                .bg(bg)
                .flex()
                .items_center()
                .gap(px(10.0))
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        this.router.context_menu = Some(ContextMenuKind::Playlist {
                            playlist: pl_ctx.clone(),
                            position: ev.position,
                        });
                        cx.notify();
                    }),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_playlist(pl_open.clone(), window, cx);
                }))
                .child(render_cover(cover_path, 38.0, false, pal))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if is_active && !pal.is_neutral {
                                    pal.hi
                                } else {
                                    pal.ink
                                })
                                .truncate()
                                .child(pl.title.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(12.5))
                                .text_color(pal.ink_2)
                                .truncate()
                                .child(sub),
                        ),
                ),
        );
    }

    let has_macos_traffic_lights = cfg!(target_os = "macos") && !is_fullscreen;
    let mut rail = div()
        .w(px(width_px))
        .h_full()
        .min_h_0()
        .flex_shrink_0()
        .bg(pal.rail)
        .border_r_1()
        .border_color(pal.line)
        .px(px(10.0))
        .pb(px(12.0))
        .flex()
        .flex_col();

    if has_macos_traffic_lights {
        rail = rail.child(
            div()
                .mx(px(-10.0))
                .h(px(42.0))
                .w(px(width_px))
                .flex_shrink_0()
                .window_control_area(WindowControlArea::Drag),
        );
    } else {
        rail = rail.pt(px(18.0));
    }

    rail.child(
        div()
            .pt(px(if has_macos_traffic_lights { 4.0 } else { 0.0 }))
            .px(px(10.0))
            .pb(px(18.0))
            .child(
                div()
                    .id("brand-home-btn")
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate_to(Route::Home, window, cx);
                    }))
                    .child(icon(Icon::Logo, 28.0, pal.hi))
                    .child(
                        div()
                            .text_size(px(19.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(pal.ink)
                            .child("Vively"),
                    ),
            ),
    )
    .child(nav_col)
    .child(
        div()
            .h(px(1.0))
            .bg(pal.line)
            .mx(px(10.0))
            .mt(px(16.0))
            .mb(px(14.0)),
    )
    .child(
        div()
            .pl(px(12.0))
            .pr(px(4.0))
            .pb(px(6.0))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(pal.ink_2)
                    .child("Плейлисты"),
            )
            .child(
                icon_button("create-pl-btn", Icon::Plus, 18.0, false, pal).on_click(cx.listener(
                    |this, _, window, cx| {
                        this.open_playlist_editor(None, None, window, cx);
                    },
                )),
            ),
    )
    .child(
        div()
            .id("rail-playlists-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scrollbar()
            .child(pl_list),
    )
    .into_any_element()
}
