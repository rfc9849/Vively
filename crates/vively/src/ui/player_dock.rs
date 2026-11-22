use gpui_kit::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement as _, StatefulInteractiveElement as _, Styled as _, div, px,
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
    ui::components::{explicit_badge, icon_button, render_scrub_bar, render_volume_slider},
};

pub fn render_player_dock(
    app: &VivelyApp,
    center_width: f32,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let Some(track) = app.core.queue.current().cloned() else {
        return div().into_any_element();
    };

    let playing = app.core.audio_engine.is_playing();
    let loading = app.core.track_loading;
    let liked = app.is_liked(&track);
    let shuffled = app.core.queue.is_shuffled();
    let repeat = app.core.queue.repeat_mode;
    let cover_path = app.resolve_artwork(track.artwork_url(), cx);

    let dur = track.duration.max(0);

    let show_vol_slider = center_width >= 1000.0;
    let show_opt = center_width >= 740.0;
    let show_panel_ctls = center_width >= 600.0;
    let playback_width = (center_width * 0.32).clamp(148.0, if show_opt { 460.0 } else { 180.0 });

    let cta_hover = pal.cta_hover;
    let play_btn = div()
        .id("pb-play-btn")
        .size(px(38.0))
        .rounded_full()
        .bg(pal.cta_bg)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(move |s| s.bg(cta_hover))
        .on_click(cx.listener(|this, _, _, cx| {
            this.toggle_play_pause(cx);
        }))
        .child(if loading {
            spinner("pb-play-spinner", 19.0, pal.cta_fg)
        } else {
            icon(
                if playing { Icon::Pause } else { Icon::Play },
                19.0,
                pal.cta_fg,
            )
            .into_any_element()
        });

    let mut transport = div().flex().items_center().gap(px(5.0)).flex_shrink_0();
    if show_opt {
        transport = transport.child(
            icon_button("pb-shuffle", Icon::Shuffle, 19.0, shuffled, pal)
                .on_click(cx.listener(|this, _, _, cx| this.toggle_shuffle(cx))),
        );
    }
    transport = transport
        .child(
            icon_button("pb-prev", Icon::Prev, 19.0, false, pal)
                .on_click(cx.listener(|this, _, _, cx| this.prev_track(cx))),
        )
        .child(play_btn)
        .child(
            icon_button("pb-next", Icon::Next, 19.0, false, pal)
                .on_click(cx.listener(|this, _, _, cx| this.next_track(cx))),
        );
    if show_opt {
        let mut rep_wrap = div().relative().child(
            icon_button(
                "pb-repeat",
                Icon::Repeat,
                19.0,
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
        transport = transport.child(rep_wrap);
    }

    let artist_id = track
        .main_artists
        .as_ref()
        .and_then(|a| a.first())
        .map(|a| (a.id.clone(), a.name.clone()));

    let mut title = div()
        .id("pb-title-open")
        .min_w_0()
        .text_size(px(15.0))
        .font_weight(FontWeight::BOLD)
        .text_color(pal.ink)
        .truncate();
    if let Some(album) = track
        .album
        .as_ref()
        .filter(|a| a.id.unwrap_or(0) != 0)
        .cloned()
    {
        title = title
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_album(&album, window, cx);
            }));
    }
    let mut title_line = div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .min_w_0()
        .child(title.child(track.title.clone()));

    if track.is_explicit {
        title_line = title_line.child(explicit_badge(pal));
    }

    let hover_ink = pal.ink;
    let mut artist_btn = div()
        .id("pb-artist-open")
        .min_w_0()
        .text_size(px(12.5))
        .text_color(pal.ink_2)
        .truncate();
    if let Some((aid, artist_name)) = artist_id {
        let artist_name = if artist_name.trim().is_empty() {
            track.artist.clone()
        } else {
            artist_name
        };
        artist_btn = artist_btn
            .cursor_pointer()
            .hover(move |s| s.text_color(hover_ink))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_artist(aid.clone(), &artist_name, window, cx);
            }));
    }
    let artist_line = artist_btn.child(track.artist.clone());

    let ctx_track = track.clone();
    let track_area = div()
        .id("pb-track-area")
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(12.0))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                this.router.context_menu = Some(ContextMenuKind::Track {
                    track: ctx_track.clone(),
                    position: ev.position,
                    editable_playlist: None,
                });
                cx.notify();
            }),
        )
        .child(
            div()
                .id("pb-cover-expand")
                .relative()
                .size(px(48.0))
                .flex_shrink_0()
                .cursor_pointer()
                .group("pb-cov")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.router.stage_open = true;
                    cx.notify();
                }))
                .child(render_cover(cover_path, 48.0, false, pal))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(px(3.0))
                        .bg(gpui_kit::rgba(0x0000008c))
                        .flex()
                        .items_center()
                        .justify_center()
                        .opacity(0.0)
                        .group_hover("pb-cov", |s| s.opacity(1.0))
                        .child(icon(Icon::Expand, 18.0, gpui_kit::rgb(0xFFFFFF).into())),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(title_line)
                .child(artist_line),
        );

    let time_label = |text| {
        div()
            .min_w(px(32.0))
            .flex_shrink_0()
            .text_size(px(11.0))
            .text_color(pal.ink_2)
            .child(text)
    };
    let playback = div()
        .w(px(playback_width))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(4.0))
        .child(transport)
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(time_label(fmt_duration_f64(app.core.current_elapsed)).text_right())
                .child(render_scrub_bar(app, "pb-scrub", false, pal, cx))
                .child(time_label(fmt_duration(dur))),
        );

    let like_tr = track.clone();
    let more_tr = track;
    let eff_vol = app.core.audio_engine.effective_volume();
    let mut right_bar = div()
        .flex()
        .items_center()
        .gap(px(2.0))
        .flex_shrink_0()
        .child(
            icon_button(
                "pb-like",
                if liked { Icon::HeartFill } else { Icon::Heart },
                19.0,
                liked,
                pal,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_like_track(like_tr.clone(), cx);
            })),
        );

    if show_panel_ctls {
        right_bar = right_bar.child(
            icon_button("pb-more", Icon::More, 19.0, false, pal).on_mouse_down(
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
        );
    }

    if show_panel_ctls {
        let lyr_active = app.router.panel_open && app.router.panel_tab == PanelTab::Lyrics;
        let q_active = app.router.panel_open && app.router.panel_tab == PanelTab::Queue;
        right_bar = right_bar
            .child(
                icon_button("pb-lyrics", Icon::Lyrics, 19.0, lyr_active, pal).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.router.toggle_panel(PanelTab::Lyrics);
                        cx.notify();
                    }),
                ),
            )
            .child(
                icon_button("pb-queue", Icon::Queue, 19.0, q_active, pal).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.router.toggle_panel(PanelTab::Queue);
                        cx.notify();
                    },
                )),
            );
    }

    let mut vol_group = div().flex().items_center().gap(px(4.0)).child(
        icon_button(
            "pb-mute",
            if eff_vol <= 0.001 {
                Icon::Mute
            } else {
                Icon::Volume
            },
            19.0,
            false,
            pal,
        )
        .on_click(cx.listener(|this, _, _, cx| this.toggle_mute(cx))),
    );
    if show_vol_slider {
        vol_group = vol_group.child(render_volume_slider(
            "pb-vol-slider",
            eff_vol,
            Some(76.0),
            false,
            pal,
            cx,
        ));
    }
    right_bar = right_bar.child(vol_group);

    if show_opt {
        right_bar = right_bar.child(
            icon_button("pb-fullscreen", Icon::Expand, 18.0, false, pal).on_click(cx.listener(
                |this, _, _, cx| {
                    this.router.stage_open = true;
                    cx.notify();
                },
            )),
        );
    }

    let action_area = div().flex().justify_end().child(right_bar);
    let action_area = if show_panel_ctls {
        action_area.flex_1().min_w_0()
    } else {
        action_area.w(px(88.0)).flex_shrink_0()
    };

    div()
        .absolute()
        .left(px(16.0))
        .right(px(16.0))
        .bottom(px(14.0))
        .h(px(80.0))
        .px(px(12.0))
        .rounded(px(10.0))
        .bg(pal.dock)
        .border_1()
        .border_color(pal.line)
        .shadow(pal.shadow_pop())
        .flex()
        .items_center()
        .gap(px(12.0))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(track_area)
        .child(playback)
        .child(action_area)
        .into_any_element()
}
