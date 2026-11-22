use std::{cell::RefCell, rc::Rc};

use gpui_kit::{
    AnyElement, Context, FontWeight, Hsla, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement as _, ScrollHandle, StatefulInteractiveElement as _, Styled as _,
    WindowControlArea,
    base::ScrollBounce,
    canvas,
    SharedString,
    component::scroll::{Scrollbar, ScrollbarAxis},
    div, linear_color_stop, linear_gradient, px, relative,
};
use vively_core::{
    lyrics::{LyricsContent, LyricsSource},
    models::Audio,
};

use crate::{
    design::{
        artwork::render_cover,
        colors::Palette,
        icons::{Icon, icon, spinner},
    },
    state::{
        app_state::VivelyApp,
        models::{LyricsLoadState, LyricsScrollState, active_synced_lyric_index},
        router::{ContextMenuKind, PanelTab},
    },
    ui::components::icon_button,
};

pub fn render_lyrics_scroll_area(
    id_prefix: &'static str,
    handle: &ScrollHandle,
    scroll_state: Rc<RefCell<LyricsScrollState>>,
    fade_bg: Option<Hsla>,
    content: impl IntoElement,
) -> AnyElement {
    let bounce_handle = handle.clone();
    let bounce_state = Rc::clone(&scroll_state);

    let scroll_area = div()
        .id(format!("{id_prefix}-scroll"))
        .track_scroll(handle)
        .size_full()
        .flex()
        .flex_col()
        .overflow_y_scroll()
        .child(div().w_full().flex_none().min_h_full().child(content));

    let mut wrapper = div()
        .relative()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            ScrollBounce::new(format!("{id_prefix}-bounce"), handle, scroll_area).on_scroll(
                move |_, cx| {
                    let offset_y = f32::from(bounce_handle.offset().y);
                    bounce_state
                        .borrow_mut()
                        .mark_manual_scroll(cx.background_executor().now(), offset_y);
                },
            ),
        );

    if let Some(bg) = fade_bg {
        let clear = bg.opacity(0.0);
        wrapper = wrapper
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(28.0))
                    .bg(linear_gradient(
                        180.0,
                        linear_color_stop(bg, 0.0),
                        linear_color_stop(clear, 1.0),
                    )),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(40.0))
                    .bg(linear_gradient(
                        0.0,
                        linear_color_stop(bg, 0.0),
                        linear_color_stop(clear, 1.0),
                    )),
            );
    }

    wrapper
        .child(
            div().absolute().inset_0().child(
                Scrollbar::new(handle)
                    .id(format!("{id_prefix}-scrollbar"))
                    .axis(ScrollbarAxis::Vertical)
                    .viewport_from_layout(),
            ),
        )
        .into_any_element()
}

pub fn render_side_panel(
    app: &VivelyApp,
    viewport_h: f32,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let tab = app.router.panel_tab;
    let panel_h = (viewport_h - 60.0).max(320.0);

    let scroll_body = match tab {
        PanelTab::Lyrics => render_lyrics_scroll_area(
            "right-panel-lyrics",
            &app.right_scroll,
            Rc::clone(&app.panel_lyrics_scroll),
            None,
            div()
                .w_full()
                .pt(px(4.0))
                .px(px(20.0))
                .pb(px(24.0))
                .child(render_panel_lyrics(app, panel_h, pal, cx)),
        ),
        PanelTab::Queue => {
            let scroll_area = div()
                .id("right-panel-queue-scroll")
                .track_scroll(&app.panel_queue_scroll)
                .size_full()
                .flex()
                .flex_col()
                .overflow_y_scroll()
                .child(
                    div()
                        .w_full()
                        .flex_none()
                        .pt(px(4.0))
                        .px(px(20.0))
                        .pb(px(24.0))
                        .child(render_queue_content(app, "panel", pal, cx)),
                );
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(ScrollBounce::new(
                    "right-panel-queue-bounce",
                    &app.panel_queue_scroll,
                    scroll_area,
                ))
                .child(
                    div().absolute().inset_0().child(
                        Scrollbar::new(&app.panel_queue_scroll)
                            .id("right-panel-queue-scrollbar")
                            .axis(ScrollbarAxis::Vertical)
                            .viewport_from_layout(),
                    ),
                )
                .into_any_element()
        }
    };

    div()
        .w(px(340.0))
        .h_full()
        .min_h_0()
        .flex_shrink_0()
        .bg(pal.rail)
        .border_l_1()
        .border_color(pal.line)
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(60.0))
                .pl(px(12.0))
                .pr(px(14.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(2.0))
                        .child(seg_tab_btn(
                            "ptab-lyrics",
                            "Текст",
                            tab == PanelTab::Lyrics,
                            pal,
                            cx,
                            PanelTab::Lyrics,
                        ))
                        .child(seg_tab_btn(
                            "ptab-queue",
                            "Очередь",
                            tab == PanelTab::Queue,
                            pal,
                            cx,
                            PanelTab::Queue,
                        )),
                )
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .window_control_area(WindowControlArea::Drag),
                )
                .child(
                    icon_button("close-panel-btn", Icon::Close, 18.0, false, pal).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.router.panel_open = false;
                            cx.notify();
                        }),
                    ),
                ),
        )
        .child(scroll_body)
        .into_any_element()
}

fn seg_tab_btn(
    id: &'static str,
    label: &'static str,
    active: bool,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
    target: PanelTab,
) -> AnyElement {
    let fg = if active { pal.ink } else { pal.ink_3 };
    let hover_fg = if active { pal.ink } else { pal.ink_2 };

    let mut btn = div()
        .id(id)
        .relative()
        .px(px(8.0))
        .pt(px(4.0))
        .pb(px(8.0))
        .cursor_pointer()
        .text_size(px(14.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(fg)
        .hover(move |s| s.text_color(hover_fg))
        .on_click(cx.listener(move |this, _, _, cx| {
            this.router.panel_tab = target;
            cx.notify();
        }))
        .child(label);

    if active {
        btn = btn.child(
            div()
                .absolute()
                .left(px(8.0))
                .right(px(8.0))
                .bottom_0()
                .h(px(3.0))
                .bg(pal.hi),
        );
    }
    btn.into_any_element()
}

fn render_panel_lyrics(
    app: &VivelyApp,
    panel_h: f32,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let Some(track) = app.core.queue.current().cloned() else {
        return div()
            .text_size(px(14.0))
            .text_color(pal.ink_2)
            .child("Включите трек, чтобы увидеть текст песни")
            .into_any_element();
    };

    let cover_path = app.resolve_artwork(track.artwork_url(), cx);
    let liked = app.is_liked(&track);
    let artist_id = track
        .main_artists
        .as_ref()
        .and_then(|a| a.first())
        .map(|a| (a.id.clone(), a.name.clone()));
    let like_tr = track.clone();
    let more_tr = track.clone();
    let ctx_tr = track.clone();
    let hover_ink = pal.ink;

    let mut artist_el = div()
        .id("np-artist-btn")
        .text_size(px(14.0))
        .text_color(pal.ink_2)
        .truncate();
    if let Some((aid, artist_name)) = artist_id {
        let artist_name = if artist_name.trim().is_empty() {
            track.artist.clone()
        } else {
            artist_name
        };
        artist_el = artist_el
            .cursor_pointer()
            .hover(move |s| s.text_color(hover_ink))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_artist(aid.clone(), &artist_name, window, cx);
            }));
    }

    let song_row = div()
        .id("np-song-row")
        .pb(px(16.0))
        .mb(px(18.0))
        .border_b_1()
        .border_color(pal.line)
        .flex()
        .items_center()
        .gap(px(12.0))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                this.router.context_menu = Some(ContextMenuKind::Track {
                    track: ctx_tr.clone(),
                    position: ev.position,
                    editable_playlist: None,
                });
                cx.notify();
            }),
        )
        .child(render_cover(cover_path, 56.0, false, pal))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(15.5))
                        .font_weight(FontWeight::BOLD)
                        .line_height(relative(1.25))
                        .text_color(pal.ink)
                        .truncate()
                        .child(track.title.clone()),
                )
                .child(artist_el.child(track.artist.clone())),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(2.0))
                .flex_shrink_0()
                .child(
                    icon_button(
                        "np-like",
                        if liked { Icon::HeartFill } else { Icon::Heart },
                        18.0,
                        liked,
                        pal,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_like_track(like_tr.clone(), cx);
                    })),
                )
                .child(
                    icon_button("np-more", Icon::More, 18.0, false, pal).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.router.context_menu = Some(ContextMenuKind::Track {
                                track: more_tr.clone(),
                                position: ev.position,
                                editable_playlist: None,
                            });
                            cx.notify();
                        }),
                    ),
                ),
        );

    let scroll_sync = (!app.router.stage_open)
        .then(|| (&app.right_scroll, Rc::clone(&app.panel_lyrics_scroll)));

    div()
        .flex()
        .flex_col()
        .child(song_row)
        .child(render_lyrics_body(
            app,
            &track,
            LyricsBodyOpts {
                font_px: 19.0,
                id_prefix: "panel",
                scroll_sync,
                synced_bottom_pad: (panel_h * 0.45).max(160.0),
            },
            pal,
            cx,
        ))
        .into_any_element()
}

pub struct LyricsBodyOpts<'a> {
    pub font_px: f32,
    pub id_prefix: &'a str,
    pub scroll_sync: Option<(&'a ScrollHandle, Rc<RefCell<LyricsScrollState>>)>,
    pub synced_bottom_pad: f32,
}

pub fn render_lyrics_body(
    app: &VivelyApp,
    track: &Audio,
    opts: LyricsBodyOpts<'_>,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let LyricsBodyOpts {
        font_px,
        id_prefix,
        scroll_sync,
        synced_bottom_pad,
    } = opts;
    let state = app.core.lyrics_states.get(&track.stable_id());
    match state {
        None | Some(LyricsLoadState::Loading) => div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .text_size(px(14.5))
            .text_color(pal.ink_2)
            .child(spinner(format!("{id_prefix}-lyrics-spin"), 16.0, pal.hi))
            .child("Загрузка текста…")
            .into_any_element(),
        Some(LyricsLoadState::Failed(_)) | Some(LyricsLoadState::Loaded(None)) => div()
            .max_w(px(320.0))
            .text_size(px(14.5))
            .text_color(pal.ink_2)
            .child("Текст для этого трека не найден.")
            .into_any_element(),
        Some(LyricsLoadState::Loaded(Some(lyrics))) => {
            let is_stage_scale = font_px >= 24.0;
            let elapsed_ms = (app.playback_position() * 1000.0).round() as i64;
            let is_synced = matches!(&lyrics.content, LyricsContent::Synced(_));

            let flow = match &lyrics.content {
                LyricsContent::Synced(lines) => {
                    let mut flow = div()
                        .flex()
                        .flex_col()
                        .gap(px(if is_stage_scale { 14.0 } else { 10.0 }));
                    let active_pos = active_synced_lyric_index(lines, elapsed_ms);

                    for (idx, line) in lines.iter().enumerate() {
                        let is_active = active_pos == Some(idx);
                        let is_past = active_pos.is_some_and(|a| idx < a);
                        let base_opacity = if is_active {
                            1.0
                        } else if is_past {
                            0.24
                        } else {
                            0.38
                        };
                        let hover_opacity = if is_active { 1.0 } else { 0.85 };
                        let text = line.text.clone().unwrap_or_else(|| "•••".into());
                        let target_secs = (line.begin.max(0) as f64) / 1000.0;

                        let mut row = div()
                            .id(format!("{id_prefix}-lyr-{idx}"))
                            .relative()
                            .py(px(if is_stage_scale { 4.0 } else { 2.0 }))
                            .cursor_pointer()
                            .text_size(px(font_px))
                            .font_weight(FontWeight::BOLD)
                            .line_height(relative(1.3))
                            .text_color(pal.ink)
                            .opacity(base_opacity)
                            .hover(move |s| s.opacity(hover_opacity))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.seek_to(target_secs, cx);
                            }))
                            .child(text);

                        if is_active && let Some((handle, ref scroll_state)) = scroll_sync {
                            let handle = handle.clone();
                            let scroll_state = Rc::clone(scroll_state);
                            row = row.child(
                                canvas(
                                    move |bounds, window, cx| {
                                        if scroll_state.borrow_mut().record_active_line_bounds(
                                            &handle,
                                            bounds,
                                            cx.reduce_motion(),
                                        ) {
                                            window.request_animation_frame();
                                        }
                                    },
                                    |_, _, _, _| {},
                                )
                                .absolute()
                                .inset_0(),
                            );
                        }

                        flow = flow.child(row);
                    }
                    flow
                }
                LyricsContent::Plain(lines) => {
                    let stanza_gap = if is_stage_scale { 26.0 } else { 18.0 };
                    let line_gap = if is_stage_scale { 8.0 } else { 5.0 };
                    let plain_size = if is_stage_scale { 24.0 } else { 16.0 };
                    let plain_color = pal.ink.opacity(0.9);
                    let mut flow = div().flex().flex_col().gap(px(stanza_gap));
                    let mut line_idx = 0usize;

                    for stanza in lines.split(|l| l.is_empty()).filter(|s| !s.is_empty()) {
                        let mut group = div().flex().flex_col().gap(px(line_gap));
                        for line in stanza {
                            let idx = line_idx;
                            line_idx += 1;
                            group = group.child(
                                div()
                                    .id(format!("{id_prefix}-plain-{idx}"))
                                    .text_size(px(plain_size))
                                    .font_weight(FontWeight::MEDIUM)
                                    .line_height(relative(1.4))
                                    .text_color(plain_color)
                                    .child(line.clone()),
                            );
                        }
                        flow = flow.child(group);
                    }
                    flow
                }
            };

            let credit = match lyrics.source {
                LyricsSource::Vk => "Текст: ВКонтакте",
                LyricsSource::Musixmatch => "Текст: Musixmatch",
                LyricsSource::Genius => "Текст: Genius",
            };

            let mut container = div().flex().flex_col().child(flow).child(
                div()
                    .mt(px(22.0))
                    .pt(px(10.0))
                    .border_t_1()
                    .border_color(pal.line)
                    .text_size(px(12.5))
                    .text_color(pal.ink_3)
                    .child(credit),
            );
            if is_synced && synced_bottom_pad > 0.0 {
                container = container.pb(px(synced_bottom_pad));
            }
            container.into_any_element()
        }
    }
}

pub fn render_queue_content(
    app: &VivelyApp,
    id_prefix: &str,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let Some(cur) = app.core.queue.current().cloned() else {
        return div()
            .text_size(px(13.5))
            .text_color(pal.ink_2)
            .child("Очередь воспроизведения пуста")
            .into_any_element();
    };

    let cur_cover = app.resolve_artwork(cur.thumbnail_url().or_else(|| cur.artwork_url()), cx);
    let upcoming: Vec<Audio> = app.core.queue.upcoming().to_vec();

    let mut col = div()
        .flex()
        .flex_col()
        .child(
            div()
                .mt(px(6.0))
                .mb(px(10.0))
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink_2)
                .child(format!("Играет из «{}»", app.core.playing_from)),
        )
        .child(
            div()
                .mx(px(-6.0))
                .p(px(6.0))
                .rounded(px(6.0))
                .bg(pal.active)
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(render_cover(cur_cover, 40.0, false, pal))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(14.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(pal.ink)
                                .truncate()
                                .child(cur.title.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(pal.ink_2)
                                .truncate()
                                .child(cur.artist.clone()),
                        ),
                ),
        )
        .child({
            let has_up = !upcoming.is_empty();
            let ink = pal.ink;
            let danger = pal.danger;
            let mut hdr_actions = div().flex().items_center().gap(px(8.0));
            hdr_actions = hdr_actions.child(
                div()
                    .id(format!("{id_prefix}-q-save-pl"))
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .text_size(px(12.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(pal.ink_3)
                    .cursor_pointer()
                    .hover(move |s| s.text_color(ink))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.save_queue_as_playlist(window, cx);
                    }))
                    .child("В плейлист"),
            );
            if has_up {
                hdr_actions = hdr_actions.child(
                    div()
                        .id(format!("{id_prefix}-q-clear"))
                        .px(px(6.0))
                        .py(px(2.0))
                        .rounded(px(4.0))
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(pal.ink_3)
                        .cursor_pointer()
                        .hover(move |s| s.text_color(danger))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.clear_upcoming_queue(cx);
                        }))
                        .child("Очистить"),
                );
            }
            div()
                .mt(px(22.0))
                .mb(px(10.0))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(pal.ink_2)
                        .child("Далее"),
                )
                .child(hdr_actions)
        });

    if upcoming.is_empty() {
        col = col.child(
            div()
                .text_size(px(13.0))
                .text_color(pal.ink_2)
                .child("Дальше ничего не запланировано"),
        );
    } else {
        let total_up = upcoming.len();
        for (offset, tr) in upcoming.into_iter().enumerate() {
            let cov = app.resolve_artwork(tr.thumbnail_url().or_else(|| tr.artwork_url()), cx);
            let tr_jump = tr.clone();
            let tr_ctx = tr.clone();
            let hover_bg = pal.hover;
            let row_grp = SharedString::from(format!("{id_prefix}-q-grp-{offset}-{}", tr.full_id()));

            let mut actions = div().flex().items_center().gap(px(2.0)).flex_shrink_0();
            if offset > 0 {
                let fill_bg = pal.fill_2;
                let g = row_grp.clone();
                actions = actions.child(
                    div()
                        .id(format!("{id_prefix}-q-up-{offset}"))
                        .size(px(24.0))
                        .rounded(px(5.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .opacity(0.0)
                        .group_hover(g, |s| s.opacity(1.0))
                        .hover(move |s| s.bg(fill_bg))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.move_upcoming_item(offset, offset - 1, cx);
                        }))
                        .child(icon(Icon::ChevUp, 14.0, pal.ink_2)),
                );
            }
            if offset + 1 < total_up {
                let fill_bg = pal.fill_2;
                let g = row_grp.clone();
                actions = actions.child(
                    div()
                        .id(format!("{id_prefix}-q-dn-{offset}"))
                        .size(px(24.0))
                        .rounded(px(5.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .opacity(0.0)
                        .group_hover(g, |s| s.opacity(1.0))
                        .hover(move |s| s.bg(fill_bg))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.move_upcoming_item(offset, offset + 1, cx);
                        }))
                        .child(icon(Icon::ChevDown, 14.0, pal.ink_2)),
                );
            }
            actions = actions.child({
                let fill_bg = pal.fill_2;
                let g = row_grp.clone();
                div()
                    .id(format!("{id_prefix}-q-rm-{offset}"))
                    .size(px(24.0))
                    .rounded(px(5.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .opacity(0.0)
                    .group_hover(g, |s| s.opacity(1.0))
                    .hover(move |s| s.bg(fill_bg))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.remove_upcoming_at(offset, cx);
                    }))
                    .child(icon(Icon::Close, 14.0, pal.ink_2))
            });

            col = col.child(
                div()
                    .id(format!("{id_prefix}-q-row-{offset}-{}", tr.full_id()))
                    .group(row_grp)
                    .mx(px(-6.0))
                    .p(px(6.0))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover_bg))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.router.context_menu = Some(ContextMenuKind::Track {
                                track: tr_ctx.clone(),
                                position: ev.position,
                                editable_playlist: None,
                            });
                            cx.notify();
                        }),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_queue_track(tr_jump.clone(), cx);
                    }))
                    .child(render_cover(cov, 40.0, false, pal))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(14.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(pal.ink)
                                    .truncate()
                                    .child(tr.title.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(pal.ink_2)
                                    .truncate()
                                    .child(tr.artist.clone()),
                            ),
                    )
                    .child(actions),
            );
        }
    }

    col.into_any_element()
}
