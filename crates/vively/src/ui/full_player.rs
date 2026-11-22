use std::rc::Rc;

use gpui_kit::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement as _, StatefulInteractiveElement as _, Styled as _,
    WindowControlArea,
    base::{ScrollBounce, Scrollbar, ScrollbarAxis},
    div, px, relative,
};
use vively_core::playback::RepeatMode;

use crate::{
    design::{
        artwork::render_cover,
        colors::Palette,
        icons::{Icon, icon, spinner},
    },
    state::{
        app_state::VivelyApp,
        router::{ContextMenuKind, PanelTab},
        runtime::{fmt_duration, fmt_duration_f64},
    },
    ui::{
        components::{explicit_badge, icon_button, render_scrub_bar, render_volume_slider},
        side_panel::{
            LyricsBodyOpts, render_lyrics_body, render_lyrics_scroll_area, render_queue_content,
        },
    },
};

pub fn render_full_player(
    app: &VivelyApp,
    viewport_w: f32,
    viewport_h: f32,
    is_fullscreen: bool,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    if !app.router.stage_open {
        return div().into_any_element();
    }
    let Some(track) = app.core.queue.current().cloned() else {
        return div().into_any_element();
    };

    let playing = app.core.audio_engine.is_playing();
    let loading = app.core.track_loading;
    let liked = app.is_liked(&track);
    let shuffled = app.core.queue.is_shuffled();
    let repeat = app.core.queue.repeat_mode;
    let cover_path = app.resolve_full_artwork(track.artwork_url(), cx);
    let dur = track.duration.max(0);
    let eff_vol = app.core.audio_engine.effective_volume();
    let stage_view = app.router.stage_view;
    let show_right = stage_view.is_some() && viewport_w >= 860.0;
    let cover_side = (viewport_h - 320.0).clamp(240.0, 420.0);

    let has_macos_traffic = cfg!(target_os = "macos") && !is_fullscreen;
    let fill_bg = pal.fill;
    let top_bar = div()
        .h(px(58.0))
        .pl(px(if has_macos_traffic { 84.0 } else { 28.0 }))
        .pr(px(28.0))
        .flex()
        .items_center()
        .child(
            div().flex_shrink_0().child(
                div()
                    .id("close-stage-btn")
                    .size(px(32.0))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(move |s| s.bg(fill_bg))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.router.stage_open = false;
                        cx.notify();
                    }))
                    .child(icon(Icon::ChevDown, 20.0, pal.ink)),
            ),
        )
        .child(
            div()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .window_control_area(WindowControlArea::Drag)
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(pal.ink_2)
                        .child(app.core.playing_from.clone()),
                ),
        )
        .child(
            div().flex_shrink_0().flex().justify_end().child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(stage_seg_btn(
                        "sview-lyrics",
                        "Текст",
                        stage_view == Some(PanelTab::Lyrics),
                        PanelTab::Lyrics,
                        pal,
                        cx,
                    ))
                    .child(stage_seg_btn(
                        "sview-queue",
                        "Очередь",
                        stage_view == Some(PanelTab::Queue),
                        PanelTab::Queue,
                        pal,
                        cx,
                    )),
            ),
        );

    let album_opt = track.album.clone();
    let artist_id = track
        .main_artists
        .as_ref()
        .and_then(|a| a.first())
        .map(|a| (a.id.clone(), a.name.clone()));
    let like_tr = track.clone();
    let more_tr = track.clone();
    let ctx_tr = track.clone();

    let mut title_el = div()
        .id("stage-song-title")
        .text_size(px(26.0))
        .font_weight(FontWeight::BOLD)
        .line_height(relative(1.15))
        .text_color(pal.ink)
        .truncate();
    if let Some(alb) = album_opt.filter(|a| a.id.unwrap_or(0) != 0) {
        title_el = title_el
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_album(&alb, window, cx);
            }));
    }

    let mut title_row = div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .min_w_0()
        .child(title_el.child(track.title.clone()));
    if track.is_explicit {
        title_row = title_row.child(explicit_badge(pal));
    }

    let mut artist_el = div()
        .id("stage-song-artist")
        .mt(px(4.0))
        .text_size(px(17.0))
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
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_artist(aid.clone(), &artist_name, window, cx);
            }));
    }

    let cta_hover = pal.cta_hover;
    let mut rep_wrap = div().relative().child(
        icon_button(
            "stage-repeat",
            Icon::Repeat,
            20.0,
            repeat != RepeatMode::Off,
            pal,
        )
        .on_click(cx.listener(|this, _, _, cx| this.cycle_repeat(cx))),
    );
    if repeat == RepeatMode::One {
        rep_wrap = rep_wrap.child(
            div()
                .absolute()
                .right(px(3.0))
                .bottom(px(2.0))
                .text_size(px(9.0))
                .font_weight(FontWeight::EXTRA_BOLD)
                .text_color(pal.ink)
                .child("1"),
        );
    }

    let art_col = div()
        .w(px(cover_side))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap(px(18.0))
        .child(render_cover(cover_path, cover_side, false, pal).shadow(pal.shadow_stage_cover()))
        .child(
            div()
                .flex()
                .items_end()
                .justify_between()
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
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(title_row)
                        .child(artist_el.child(track.artist.clone())),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .flex_shrink_0()
                        .child(
                            icon_button(
                                "stage-like",
                                if liked { Icon::HeartFill } else { Icon::Heart },
                                19.0,
                                liked,
                                pal,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.toggle_like_track(like_tr.clone(), cx);
                                },
                            )),
                        )
                        .child(
                            icon_button("stage-more", Icon::More, 19.0, false, pal).on_mouse_down(
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
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .child(render_scrub_bar(app, "stage-scrub", true, pal, cx))
                .child(
                    div()
                        .mt(px(2.0))
                        .flex()
                        .justify_between()
                        .text_size(px(13.0))
                        .text_color(pal.ink_2)
                        .child(fmt_duration_f64(app.core.current_elapsed))
                        .child(fmt_duration(dur)),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(16.0))
                .child(
                    icon_button("stage-shuffle", Icon::Shuffle, 20.0, shuffled, pal)
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_shuffle(cx))),
                )
                .child(
                    icon_button("stage-prev", Icon::Prev, 20.0, false, pal)
                        .on_click(cx.listener(|this, _, _, cx| this.prev_track(cx))),
                )
                .child(
                    div()
                        .id("stage-play")
                        .size(px(60.0))
                        .rounded_full()
                        .bg(pal.cta_bg)
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(move |s| s.bg(cta_hover))
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_play_pause(cx)))
                        .child(if loading {
                            spinner("stage-play-spinner", 24.0, pal.cta_fg)
                        } else {
                            icon(
                                if playing { Icon::Pause } else { Icon::Play },
                                24.0,
                                pal.cta_fg,
                            )
                            .into_any_element()
                        }),
                )
                .child(
                    icon_button("stage-next", Icon::Next, 20.0, false, pal)
                        .on_click(cx.listener(|this, _, _, cx| this.next_track(cx))),
                )
                .child(rep_wrap),
        )
        .child(
            div()
                .px(px(12.0))
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(icon(Icon::Mute, 16.0, pal.ink_2))
                .child(render_volume_slider(
                    "stage-vol-slider",
                    eff_vol,
                    None,
                    true,
                    pal,
                    cx,
                ))
                .child(icon(Icon::Volume, 16.0, pal.ink_2)),
        );

    let mut content_row = div()
        .flex_1()
        .min_h_0()
        .w_full()
        .max_w(px(1320.0))
        .mx_auto()
        .pt(px(8.0))
        .px(px(48.0))
        .pb(px(40.0))
        .flex()
        .items_center();

    if show_right {
        let stage_h = (viewport_h * 0.76).max(320.0);
        let right_col = match stage_view {
            Some(PanelTab::Queue) => {
                let scroll_area = div()
                    .id("stage-right-queue-scroll")
                    .track_scroll(&app.stage_queue_scroll)
                    .size_full()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .w_full()
                            .flex_none()
                            .py(px(16.0))
                            .px(px(8.0))
                            .child(render_queue_content(app, "stage", pal, cx)),
                    );
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .max_h(px(stage_h))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(ScrollBounce::new(
                        "stage-queue-bounce",
                        &app.stage_queue_scroll,
                        scroll_area,
                    ))
                    .child(
                        div().absolute().inset_0().child(
                            Scrollbar::new(&app.stage_queue_scroll)
                                .id("stage-queue-scrollbar")
                                .axis(ScrollbarAxis::Vertical)
                                .viewport_from_layout(),
                        ),
                    )
                    .into_any_element()
            }
            _ => div()
                .flex_1()
                .min_w_0()
                .h_full()
                .max_h(px(stage_h))
                .flex()
                .flex_col()
                .child(render_lyrics_scroll_area(
                    "stage-right-lyrics",
                    &app.stage_scroll,
                    Rc::clone(&app.stage_lyrics_scroll),
                    Some(pal.stage_bg),
                    div()
                        .w_full()
                        .pt(px(28.0))
                        .pb(px(16.0))
                        .px(px(8.0))
                        .child(render_lyrics_body(
                            app,
                            &track,
                            LyricsBodyOpts {
                                font_px: 30.0,
                                id_prefix: "stage",
                                scroll_sync: Some((
                                    &app.stage_scroll,
                                    Rc::clone(&app.stage_lyrics_scroll),
                                )),
                                synced_bottom_pad: (stage_h * 0.50).max(160.0),
                            },
                            pal,
                            cx,
                        )),
                ))
                .into_any_element(),
        };
        content_row = content_row.gap(px(80.0)).child(art_col).child(right_col);
    } else {
        content_row = content_row.justify_center().child(art_col);
    }

    div()
        .id("immersive-stage")
        .absolute()
        .inset_0()
        .occlude()
        .bg(pal.stage_bg)
        .flex()
        .flex_col()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(top_bar)
        .child(content_row)
        .into_any_element()
}

fn stage_seg_btn(
    id: &'static str,
    label: &'static str,
    active: bool,
    target: PanelTab,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
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
            this.router.toggle_stage_tab(target);
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
