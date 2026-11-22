use crate::design::colors::ToHsla;
use std::{path::PathBuf, rc::Rc, time::Duration};

use gpui_kit::{
    Animation, AnimationExt as _, AnyElement, Context, DispatchPhase, Div, ElementId, FontWeight,
    Hsla, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    canvas, div, px, relative,
};
use vively_core::models::{Audio, Playlist, RecommendedPlaylist};

use crate::{
    design::{
        artwork::{render_cover, render_liked_cover},
        colors::{Palette, RgbColor, parse_hex_color},
        icons::{Icon, icon, spinner},
    },
    state::{
        app_state::VivelyApp,
        router::ContextMenuKind,
        runtime::{fmt_duration, fmt_duration_f64},
    },
};

pub fn icon_button(
    id: impl Into<ElementId>,
    ic: Icon,
    icon_px: f32,
    active: bool,
    pal: &Palette,
) -> gpui_kit::Stateful<Div> {
    let fg = if active { pal.ink } else { pal.ink_2 };
    let bg = if active {
        pal.fill_2
    } else {
        gpui_kit::transparent_black()
    };
    let hover_bg = pal.fill;
    let hover_fg = pal.ink;

    div()
        .id(id)
        .size(px(32.0))
        .rounded(px(6.0))
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .cursor_pointer()
        .bg(bg)
        .text_color(fg)
        .hover(move |s| s.bg(hover_bg).text_color(hover_fg))
        .child(icon(ic, icon_px, fg))
}

pub fn primary_play_pill(
    id: impl Into<ElementId>,
    ic: Icon,
    label: impl Into<String>,
    pal: &Palette,
) -> gpui_kit::Stateful<Div> {
    let hover_bg = pal.cta_hover;
    div()
        .id(id)
        .h(px(40.0))
        .pl(px(14.0))
        .pr(px(18.0))
        .rounded(px(6.0))
        .bg(pal.cta_bg)
        .text_color(pal.cta_fg)
        .flex()
        .items_center()
        .gap(px(8.0))
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .child(icon(ic, 18.0, pal.cta_fg))
        .child(
            div()
                .text_size(px(15.0))
                .font_weight(FontWeight::BOLD)
                .child(label.into()),
        )
}

pub fn pill_button(
    id: impl Into<ElementId>,
    ic: Option<Icon>,
    label: impl Into<String>,
    selected: bool,
    pal: &Palette,
) -> gpui_kit::Stateful<Div> {
    let bg = if selected { pal.ink } else { pal.fill };
    let fg = if selected { pal.ground } else { pal.ink };
    let hover_bg = if selected { pal.ink } else { pal.fill_2 };

    let mut el = div()
        .id(id)
        .h(px(36.0))
        .px(px(13.0))
        .rounded(px(6.0))
        .bg(bg)
        .text_color(fg)
        .flex()
        .items_center()
        .gap(px(7.0))
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg));

    if let Some(ic) = ic {
        el = el.child(icon(ic, 17.0, fg));
    }
    el.child(
        div()
            .text_size(px(14.0))
            .font_weight(FontWeight::SEMIBOLD)
            .child(label.into()),
    )
}

pub fn mood_chip(
    id: impl Into<ElementId>,
    label: impl Into<String>,
    selected: bool,
    pal: &Palette,
) -> gpui_kit::Stateful<Div> {
    let bg = if selected { pal.ink } else { pal.fill };
    let fg = if selected { pal.ground } else { pal.ink };
    let hover_bg = if selected { pal.ink } else { pal.fill_2 };

    div()
        .id(id)
        .h(px(32.0))
        .px(px(13.0))
        .rounded(px(6.0))
        .bg(bg)
        .text_color(fg)
        .flex()
        .items_center()
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .child(
            div()
                .text_size(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(label.into()),
        )
}

pub fn toggle_pill(
    id: impl Into<ElementId>,
    is_on: bool,
    pal: &Palette,
) -> gpui_kit::Stateful<Div> {
    let bg = if is_on { pal.hi } else { pal.fill_2 };
    let knob_left = if is_on { px(19.0) } else { px(3.0) };
    let knob_color = if is_on && pal.is_dark {
        pal.cta_fg
    } else {
        gpui_kit::rgb(0xFFFFFF).into()
    };

    div()
        .id(id)
        .relative()
        .w(px(40.0))
        .h(px(24.0))
        .rounded(px(12.0))
        .bg(bg)
        .flex_shrink_0()
        .cursor_pointer()
        .child(
            div()
                .absolute()
                .top(px(3.0))
                .left(knob_left)
                .size(px(18.0))
                .rounded_full()
                .bg(knob_color),
        )
}

pub fn explicit_badge(pal: &Palette) -> Div {
    div()
        .px(px(4.0))
        .py(px(2.0))
        .rounded(px(2.0))
        .bg(pal.fill_2)
        .text_color(pal.ink_2)
        .text_size(px(10.0))
        .font_weight(FontWeight::BOLD)
        .flex_shrink_0()
        .child("E")
}

pub enum HeroArt {
    Cover {
        local_path: Option<PathBuf>,
        round: bool,
    },
    Liked,
    Avatar {
        local_path: Option<PathBuf>,
        seed_id: i64,
        initials: String,
    },
}

pub fn render_hero_banner(
    art: HeroArt,
    tint: Option<RgbColor>,
    title: &str,
    byline: Option<AnyElement>,
    description: Option<&str>,
    facts: &[String],
    pal: &Palette,
) -> Div {
    let cover_el: AnyElement = match art {
        HeroArt::Cover { local_path, round } => render_cover(local_path, 200.0, round, pal)
            .shadow(pal.shadow_art())
            .into_any_element(),
        HeroArt::Liked => render_liked_cover(200.0, pal.art_ring)
            .shadow(pal.shadow_art())
            .into_any_element(),
        HeroArt::Avatar {
            local_path,
            seed_id,
            initials,
        } => crate::design::artwork::render_avatar(
            local_path,
            seed_id,
            &initials,
            200.0,
            pal.art_ring,
        )
        .shadow(pal.shadow_art())
        .into_any_element(),
    };

    let joined_facts = facts
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if i == 0 {
                s.clone()
            } else {
                let mut chars = s.chars();
                match chars.next() {
                    Some(first) => format!("{}{}", first.to_lowercase(), chars.as_str()),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    let mut details = div()
        .flex_1()
        .min_w_0()
        .pb(px(2.0))
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(px(40.0))
                .font_weight(FontWeight::BOLD)
                .line_height(relative(1.1))
                .text_color(pal.ink)
                .truncate()
                .child(title.to_owned()),
        );

    if let Some(byline) = byline {
        details = details.child(div().mt(px(12.0)).child(byline));
    }

    if let Some(desc) = description.filter(|d| !d.trim().is_empty()) {
        details = details.child(
            div()
                .mt(px(6.0))
                .max_w(px(560.0))
                .text_size(px(14.5))
                .text_color(pal.ink_2)
                .child(desc.to_owned()),
        );
    }

    if !joined_facts.is_empty() {
        details = details.child(
            div()
                .mt(px(6.0))
                .text_size(px(14.0))
                .text_color(pal.ink_2)
                .child(joined_facts),
        );
    }

    div()
        .mx(px(-32.0))
        .mt(px(-8.0))
        .mb(px(22.0))
        .pt(px(32.0))
        .px(px(32.0))
        .pb(px(28.0))
        .bg(pal.hero_banner_bg(tint))
        .border_b_1()
        .border_color(pal.line)
        .flex()
        .items_end()
        .gap(px(28.0))
        .child(cover_el)
        .child(details)
}

pub struct TrackTableOptions {
    pub id_prefix: String,
    pub is_album: bool,
    pub album_artist: Option<String>,
    pub hide_album: bool,
    pub no_header: bool,
    pub from_label: String,
    pub origin: vively_core::playback::PlaybackOrigin,
    pub editable_playlist: Option<Playlist>,
}

pub fn render_track_list(
    app: &VivelyApp,
    tracks: &[Audio],
    opts: TrackTableOptions,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    if tracks.is_empty() {
        return div()
            .py(px(32.0))
            .px(px(12.0))
            .text_size(px(14.0))
            .text_color(pal.ink_2)
            .child("Ничего не найдено")
            .into_any_element();
    }

    let hide_album = opts.is_album || opts.hide_album;
    let current_id = app.core.queue.current().map(Audio::full_id);
    let is_playing = app.core.audio_engine.is_playing();
    let full_queue = cached_queue(app, &opts.id_prefix, tracks);

    let mut container = div().flex().flex_col();

    if !opts.no_header {
        let mut head = div()
            .h(px(32.0))
            .px(px(10.0))
            .mb(px(4.0))
            .border_b_1()
            .border_color(pal.line)
            .flex()
            .items_center()
            .gap(px(12.0))
            .text_size(px(13.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(pal.ink_3)
            .child(div().w(px(32.0)).flex_shrink_0().text_center().child("#"))
            .child(div().flex_1().min_w_0().child("Трек"));

        if !hide_album {
            head = head.child(div().w(px(190.0)).flex_shrink_0().child("Альбом"));
        }

        head = head
            .child(div().w(px(34.0)).flex_shrink_0())
            .child(
                div()
                    .w(px(50.0))
                    .flex_shrink_0()
                    .text_right()
                    .child("Время"),
            )
            .child(div().w(px(32.0)).flex_shrink_0());

        container = container.child(head);
    }

    let virtualize = tracks.len() > VIRTUALIZE_MIN_ROWS;
    let range = if virtualize {
        visible_track_rows(app, &opts.id_prefix, tracks.len())
    } else {
        0..tracks.len()
    };

    let rows = tracks[range.clone()]
        .iter()
        .enumerate()
        .map(|(offset, track)| {
            render_single_track_row(
                app,
                track,
                range.start + offset,
                &full_queue,
                &opts,
                hide_album,
                current_id.as_deref() == Some(track.full_id().as_str()),
                is_playing,
                pal,
                cx,
            )
        })
        .collect::<Vec<_>>();

    if !virtualize {
        return container.children(rows).into_any_element();
    }

    let key = opts.id_prefix.clone();
    let tops = Rc::clone(&app.track_list_tops);
    let scroll = app.canvas_scroll.clone();
    let measure = canvas(
        move |bounds, window, _| {
            let top = bounds.top() - scroll.bounds().top() - scroll.offset().y;
            let prev = tops.borrow_mut().insert(key.clone(), top);
            if prev.is_none_or(|p| (p - top).abs() > px(1.0)) {
                window.refresh();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();

    container
        .child(
            div()
                .relative()
                .h(px(tracks.len() as f32 * TRACK_ROW_H))
                .child(measure)
                .child(
                    div()
                        .absolute()
                        .top(px(range.start as f32 * TRACK_ROW_H))
                        .left_0()
                        .right_0()
                        .flex()
                        .flex_col()
                        .children(rows),
                ),
        )
        .into_any_element()
}

const TRACK_ROW_H: f32 = 52.0;
const VIRTUALIZE_MIN_ROWS: usize = 40;
const OVERSCAN_ROWS: f32 = 8.0;

fn visible_track_rows(app: &VivelyApp, key: &str, len: usize) -> std::ops::Range<usize> {
    let Some(list_top) = app.track_list_tops.borrow().get(key).copied() else {
        return 0..len.min(VIRTUALIZE_MIN_ROWS);
    };
    let viewport = app.canvas_scroll.bounds().size.height;
    let viewport = if viewport > px(0.0) {
        viewport
    } else {
        px(1200.0)
    };
    let scrolled = -app.canvas_scroll.offset().y;
    let first = f32::from(scrolled - list_top) / TRACK_ROW_H - OVERSCAN_ROWS;
    let last = f32::from(scrolled + viewport - list_top) / TRACK_ROW_H + OVERSCAN_ROWS;
    let start = (first.floor().max(0.0) as usize).min(len);
    let end = (last.ceil().max(0.0) as usize).clamp(start, len);
    start..end
}

pub fn cached_queue(app: &VivelyApp, key: &str, tracks: &[Audio]) -> Rc<[Audio]> {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    tracks.len().hash(&mut hasher);
    for t in tracks {
        t.owner_id.hash(&mut hasher);
        t.id.hash(&mut hasher);
        t.url.hash(&mut hasher);
    }
    let fingerprint = hasher.finish();

    let mut cache = app.track_queue_cache.borrow_mut();
    if let Some((fp, queue)) = cache.get(key)
        && *fp == fingerprint
    {
        return Rc::clone(queue);
    }
    let queue: Rc<[Audio]> = Rc::from(tracks);
    cache.insert(key.to_owned(), (fingerprint, Rc::clone(&queue)));
    queue
}

#[allow(clippy::too_many_arguments)]
fn render_single_track_row(
    app: &VivelyApp,
    track: &Audio,
    idx: usize,
    full_queue: &Rc<[Audio]>,
    opts: &TrackTableOptions,
    hide_album: bool,
    is_current: bool,
    is_playing: bool,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let row_id = format!("{}-row-{}-{}", opts.id_prefix, idx, track.full_id());
    let liked = app.is_liked(track);
    let downloaded = app.is_downloaded(track);
    let cover_path = if opts.is_album {
        None
    } else {
        app.resolve_artwork(track.thumbnail_url().or_else(|| track.artwork_url()), cx)
    };

    let row_group = SharedString::from(format!("grp-{row_id}"));
    let idx_el: AnyElement = if is_current && app.core.track_loading {
        spinner(format!("{row_id}-spin"), 16.0, pal.hi)
    } else if is_current {
        let h1 = if is_playing { 13.0 } else { 8.0 };
        let h2 = if is_playing { 8.0 } else { 11.0 };
        let h3 = if is_playing { 11.0 } else { 6.0 };
        div()
            .flex()
            .items_end()
            .gap(px(2.0))
            .h(px(14.0))
            .child(div().w(px(3.0)).h(px(h1)).bg(pal.hi))
            .child(div().w(px(3.0)).h(px(h2)).bg(pal.hi))
            .child(div().w(px(3.0)).h(px(h3)).bg(pal.hi))
            .into_any_element()
    } else {
        let g1 = row_group.clone();
        let g2 = row_group.clone();
        div()
            .relative()
            .size(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(pal.ink_3)
                    .group_hover(g1, |s| s.opacity(0.0))
                    .child(format!("{}", idx + 1)),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .opacity(0.0)
                    .group_hover(g2, |s| s.opacity(1.0))
                    .child(icon(Icon::Play, 17.0, pal.ink)),
            )
            .into_any_element()
    };

    let show_artist = !opts.is_album
        || opts
            .album_artist
            .as_deref()
            .is_none_or(|aa| aa != track.artist);

    let queue_for_play = Rc::clone(full_queue);
    let from_for_play = opts.from_label.clone();
    let origin = opts.origin;

    let mut title_row = div().flex().items_center().gap(px(6.0)).min_w_0().child(
        div()
            .text_size(px(15.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if is_current { pal.hi } else { pal.ink })
            .truncate()
            .child(track.title.clone()),
    );

    if let Some(sub) = track.subtitle.as_deref().filter(|s| !s.is_empty()) {
        title_row = title_row.child(
            div()
                .text_size(px(13.0))
                .text_color(pal.ink_3)
                .truncate()
                .child(sub.to_owned()),
        );
    }
    if track.is_explicit {
        title_row = title_row.child(explicit_badge(pal));
    }

    let mut text_col = div().flex_1().min_w_0().flex().flex_col().child(title_row);

    if show_artist || downloaded {
        let mut subline = div()
            .mt(px(1.0))
            .flex()
            .items_center()
            .gap(px(5.0))
            .min_w_0();
        if downloaded {
            subline = subline.child(icon(Icon::Downloaded, 14.0, pal.ink_2));
        }
        if show_artist {
            let artist_id = track
                .main_artists
                .as_ref()
                .and_then(|a| a.first())
                .map(|a| (a.id.clone(), a.name.clone()));
            let artist_name = track.artist.clone();
            let hover_ink = pal.ink;
            let mut ar_el = div()
                .id(format!("{row_id}-artist"))
                .text_size(px(13.5))
                .text_color(pal.ink_2)
                .truncate();
            if let Some((aid, artist_name)) = artist_id {
                let artist_name = if artist_name.trim().is_empty() {
                    track.artist.clone()
                } else {
                    artist_name
                };
                ar_el = ar_el
                    .cursor_pointer()
                    .hover(move |s| s.text_color(hover_ink))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_artist(aid.clone(), &artist_name, window, cx);
                    }));
            }
            subline = subline.child(ar_el.child(artist_name));
        }
        text_col = text_col.child(subline);
    }

    let mut main_col = div().flex_1().min_w_0().flex().items_center().gap(px(12.0));
    if !opts.is_album {
        main_col = main_col.child(render_cover(cover_path, 40.0, false, pal));
    }
    main_col = main_col.child(text_col);

    let like_track = track.clone();
    let heart_fg = if liked { pal.ink } else { pal.ink_3 };
    let heart_hover = pal.ink;
    let g_heart = row_group.clone();
    let heart_btn = div()
        .id(format!("{row_id}-like"))
        .size(px(32.0))
        .rounded(px(6.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .opacity(if liked { 1.0 } else { 0.0 })
        .group_hover(g_heart, |s| s.opacity(1.0))
        .hover(move |s| s.bg(pal.fill).text_color(heart_hover))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |this, _, _, cx| {
            this.toggle_like_track(like_track.clone(), cx);
        }))
        .child(icon(
            if liked { Icon::HeartFill } else { Icon::Heart },
            18.0,
            heart_fg,
        ));

    let menu_track = track.clone();
    let menu_pl = opts.editable_playlist.clone();
    let fill_bg = pal.fill;
    let g_more = row_group.clone();
    let more_btn = div()
        .id(format!("{row_id}-more"))
        .size(px(32.0))
        .rounded(px(6.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .opacity(0.0)
        .group_hover(g_more, |s| s.opacity(1.0))
        .hover(move |s| s.bg(fill_bg))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                this.router.context_menu = Some(ContextMenuKind::Track {
                    track: menu_track.clone(),
                    position: ev.position,
                    editable_playlist: menu_pl.clone(),
                });
                cx.notify();
            }),
        )
        .child(icon(Icon::More, 18.0, pal.ink_2));

    let ctx_track = track.clone();
    let ctx_pl = opts.editable_playlist.clone();
    let hover_row = pal.hover;

    let mut row = div()
        .group(row_group)
        .id(row_id.clone())
        .h(px(TRACK_ROW_H))
        .px(px(10.0))
        .rounded(px(6.0))
        .flex()
        .items_center()
        .gap(px(12.0))
        .cursor_pointer()
        .hover(move |s| s.bg(hover_row))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                this.router.context_menu = Some(ContextMenuKind::Track {
                    track: ctx_track.clone(),
                    position: ev.position,
                    editable_playlist: ctx_pl.clone(),
                });
                cx.notify();
            }),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.play_tracks(
                queue_for_play.as_ref().to_vec(),
                idx,
                origin,
                from_for_play.clone(),
                None,
                cx,
            );
        }))
        .child(
            div()
                .w(px(32.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .child(idx_el),
        )
        .child(main_col);

    if !hide_album {
        let album_cell = if let Some(album) = &track.album
            && let Some(album_title) = album.title.as_deref().filter(|t| !t.is_empty())
        {
            let album_clone = album.clone();
            let hover_ink = pal.ink;
            div()
                .id(format!("{row_id}-album"))
                .w(px(190.0))
                .flex_shrink_0()
                .text_size(px(13.5))
                .text_color(pal.ink_2)
                .truncate()
                .cursor_pointer()
                .hover(move |s| s.text_color(hover_ink))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_album(&album_clone, window, cx);
                }))
                .child(album_title.to_owned())
                .into_any_element()
        } else {
            div().w(px(190.0)).flex_shrink_0().into_any_element()
        };
        row = row.child(album_cell);
    }

    row = row
        .child(
            div()
                .w(px(34.0))
                .flex_shrink_0()
                .flex()
                .justify_center()
                .child(heart_btn),
        )
        .child(
            div()
                .w(px(50.0))
                .flex_shrink_0()
                .text_right()
                .text_size(px(13.5))
                .text_color(pal.ink_2)
                .child(fmt_duration(track.duration)),
        )
        .child(div().w(px(32.0)).flex_shrink_0().child(more_btn));

    row.into_any_element()
}

pub fn render_playlist_card(
    app: &VivelyApp,
    id_prefix: &str,
    playlist: &Playlist,
    offline_route: bool,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let card_id = format!("{id_prefix}-pl-{}", playlist.stable_id());
    let cover_path = app.resolve_artwork(playlist.artwork_url(), cx);
    let pl_open = playlist.clone();
    let pl_play = playlist.clone();
    let pl_ctx = playlist.clone();
    let cta_bg = pal.cta_bg;
    let cta_fg = pal.cta_fg;
    let cta_hover = pal.cta_hover;

    let card_grp = SharedString::from(format!("grp-{card_id}"));
    let card_grp_fab = card_grp.clone();
    div()
        .group(card_grp)
        .id(card_id.clone())
        .w(px(164.0))
        .flex()
        .flex_col()
        .cursor_pointer()
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
            if offline_route {
                this.navigate_to(
                    crate::state::router::Route::DownloadedCol(pl_open.full_id()),
                    window,
                    cx,
                );
            } else {
                this.open_playlist(pl_open.clone(), window, cx);
            }
        }))
        .child(
            div()
                .relative()
                .size(px(164.0))
                .mb(px(10.0))
                .child(render_cover(cover_path, 164.0, false, pal).shadow(pal.shadow_art()))
                .child(
                    div()
                        .id(format!("{card_id}-play"))
                        .absolute()
                        .left(px(8.0))
                        .bottom(px(8.0))
                        .size(px(36.0))
                        .rounded_full()
                        .bg(cta_bg)
                        .flex()
                        .items_center()
                        .justify_center()
                        .opacity(0.0)
                        .group_hover(card_grp_fab, |s| s.opacity(1.0))
                        .hover(move |s| s.bg(cta_hover))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.play_playlist_immediately(pl_play.clone(), offline_route, cx);
                        }))
                        .child(icon(Icon::Play, 17.0, cta_fg)),
                ),
        )
        .child(
            div()
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink)
                .truncate()
                .child(playlist.title.clone()),
        )
        .child(
            div()
                .mt(px(1.0))
                .text_size(px(13.0))
                .text_color(pal.ink_2)
                .truncate()
                .child(playlist.byline()),
        )
        .into_any_element()
}

pub fn render_recommended_card(
    app: &VivelyApp,
    rec: &RecommendedPlaylist,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let pl = rec.as_playlist();
    let card_id = format!("rec-pl-{}", rec.full_id());
    let cover_path = app.resolve_artwork(rec.artwork_url(), cx);
    let dot_color: Hsla = rec
        .color
        .as_deref()
        .and_then(parse_hex_color)
        .map(ToHsla::to_hsla)
        .unwrap_or(pal.ink_2);
    let match_label = rec
        .match_text()
        .map(|pct| format!("Совпадение {pct}"))
        .or_else(|| rec.percentage_title.clone())
        .unwrap_or_else(|| "Рекомендация".into());

    let pl_open = pl.clone();
    let pl_play = pl;
    let cta_bg = pal.cta_bg;
    let cta_fg = pal.cta_fg;
    let cta_hover = pal.cta_hover;

    let rec_grp = SharedString::from(format!("grp-{card_id}"));
    let rec_grp_fab = rec_grp.clone();
    div()
        .group(rec_grp)
        .id(card_id.clone())
        .w(px(164.0))
        .flex()
        .flex_col()
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, window, cx| {
            this.open_playlist(pl_open.clone(), window, cx);
        }))
        .child(
            div()
                .relative()
                .size(px(164.0))
                .mb(px(10.0))
                .child(render_cover(cover_path, 164.0, false, pal).shadow(pal.shadow_art()))
                .child(
                    div()
                        .id(format!("{card_id}-play"))
                        .absolute()
                        .left(px(8.0))
                        .bottom(px(8.0))
                        .size(px(36.0))
                        .rounded_full()
                        .bg(cta_bg)
                        .flex()
                        .items_center()
                        .justify_center()
                        .opacity(0.0)
                        .group_hover(rec_grp_fab, |s| s.opacity(1.0))
                        .hover(move |s| s.bg(cta_hover))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.play_playlist_immediately(pl_play.clone(), false, cx);
                        }))
                        .child(icon(Icon::Play, 17.0, cta_fg)),
                ),
        )
        .child(
            div()
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink)
                .truncate()
                .child(rec.title.clone().unwrap_or_default()),
        )
        .child(
            div()
                .mt(px(2.0))
                .flex()
                .items_center()
                .gap(px(7.0))
                .text_size(px(13.0))
                .text_color(pal.ink_2)
                .child(div().size(px(9.0)).bg(dot_color).flex_shrink_0())
                .child(match_label),
        )
        .into_any_element()
}

fn progress_path(
    bounds: gpui_kit::Bounds<gpui_kit::Pixels>,
    is_stage: bool,
) -> Option<gpui_kit::Path<gpui_kit::Pixels>> {
    if bounds.size.width <= px(0.0) {
        return None;
    }
    let left = bounds.left();
    let right = bounds.right();
    let top = bounds.top();
    let bottom = bounds.bottom();
    let mut fill = gpui_kit::PathBuilder::fill();
    if is_stage {
        fill.add_polygon(
            &[
                gpui_kit::point(left, top),
                gpui_kit::point(right, top),
                gpui_kit::point(right, bottom),
                gpui_kit::point(left, bottom),
            ],
            true,
        );
    } else {
        let radius = bounds.size.height.min(bounds.size.width) / 2.0;
        let radii = gpui_kit::point(radius, radius);
        fill.move_to(gpui_kit::point(left + radius, top));
        fill.line_to(gpui_kit::point(right - radius, top));
        fill.arc_to(
            radii,
            px(0.),
            false,
            true,
            gpui_kit::point(right, top + radius),
        );
        fill.line_to(gpui_kit::point(right, bottom - radius));
        fill.arc_to(
            radii,
            px(0.),
            false,
            true,
            gpui_kit::point(right - radius, bottom),
        );
        fill.line_to(gpui_kit::point(left + radius, bottom));
        fill.arc_to(
            radii,
            px(0.),
            false,
            true,
            gpui_kit::point(left, bottom - radius),
        );
        fill.line_to(gpui_kit::point(left, top + radius));
        fill.arc_to(
            radii,
            px(0.),
            false,
            true,
            gpui_kit::point(left + radius, top),
        );
        fill.close();
    }
    fill.build().ok()
}

#[allow(clippy::too_many_arguments)]
pub fn render_playlist_shelf(
    app: &VivelyApp,
    id_prefix: &str,
    title: &str,
    playlists: &[Playlist],
    total: usize,
    expanded: bool,
    center_w: f32,
    on_toggle: impl Fn(&mut VivelyApp, &mut Context<VivelyApp>) + 'static,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let per_row = (((center_w - 64.0 + 18.0) / (164.0 + 18.0)).floor() as usize).max(1);
    let overflows = total.max(playlists.len()) > per_row;
    let shown = if expanded || !overflows {
        playlists.len()
    } else {
        per_row
    };

    let mut header = div()
        .mb(px(14.0))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .child(
            div()
                .text_size(px(20.0))
                .font_weight(FontWeight::BOLD)
                .text_color(pal.ink)
                .child(title.to_owned()),
        );
    if overflows {
        let hover_ink = pal.ink;
        header = header.child(
            div()
                .id(SharedString::from(format!("{id_prefix}-more")))
                .flex_shrink_0()
                .text_size(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink_2)
                .cursor_pointer()
                .hover(move |s| s.text_color(hover_ink))
                .on_click(cx.listener(move |this, _, _, cx| {
                    on_toggle(this, cx);
                    cx.notify();
                }))
                .child(if expanded {
                    "Свернуть"
                } else {
                    "Показать все"
                }),
        );
    }

    div()
        .flex()
        .flex_col()
        .child(header)
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_x(px(18.0))
                .gap_y(px(22.0))
                .children(
                    playlists
                        .iter()
                        .take(shown)
                        .map(|pl| render_playlist_card(app, id_prefix, pl, false, pal, cx)),
                ),
        )
        .into_any_element()
}

pub fn render_artist_card(
    app: &VivelyApp,
    id: String,
    artist_id: &str,
    name: &str,
    avatar: Option<&url::Url>,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let cov = app.resolve_artwork(avatar, cx);
    let aid = artist_id.to_owned();
    let open_name = name.to_owned();

    div()
        .id(id)
        .w(px(164.0))
        .flex()
        .flex_col()
        .items_center()
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, window, cx| {
            this.open_artist(aid.clone(), &open_name, window, cx);
        }))
        .child(
            div()
                .mb(px(10.0))
                .child(render_cover(cov, 164.0, true, pal)),
        )
        .child(
            div()
                .w_full()
                .text_center()
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink)
                .truncate()
                .child(name.to_owned()),
        )
        .into_any_element()
}

pub fn render_scrub_bar(
    app: &VivelyApp,
    id: impl Into<ElementId>,
    is_stage: bool,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let view = cx.entity();
    let player = view.downgrade();
    let color = pal.ink;
    let fill = canvas(
        |_, _, _| (),
        move |mut bounds, _, window, cx| {
            let Some(player) = player.upgrade() else {
                return;
            };
            let player = player.read(cx);
            let duration = player.core.queue.current().map_or(0, |track| track.duration);
            if duration <= 0 {
                return;
            }
            let drag_ratio = if is_stage {
                player.stage_scrub_drag_ratio
            } else {
                player.scrub_drag_ratio
            };
            let engine = &player.core.audio_engine;
            let position = if let Some(ratio) = drag_ratio {
                ratio as f64 * duration as f64
            } else if engine.has_active() {
                if engine.is_playing()
                    && window.is_visible()
                    && (is_stage || !player.router.stage_open)
                {
                    window.request_animation_frame();
                }
                engine.progress_position()
            } else {
                player.core.current_elapsed
            };
            let fraction = (position / duration as f64).clamp(0.0, 1.0) as f32;
            bounds.size.width *= fraction;
            if let Some(fill) = progress_path(bounds, is_stage) {
                window.paint_path(fill, color);
            }
        },
    )
    .size_full();
    let mut rail = div().w_full().bg(pal.fill_2).overflow_hidden();
    if !is_stage {
        rail = rail.rounded_full();
    }

    let drag_ratio = if is_stage {
        app.stage_scrub_drag_ratio
    } else {
        app.scrub_drag_ratio
    };
    let hover_ratio = drag_ratio.or(if is_stage {
        app.stage_scrub_hover_ratio
    } else {
        app.scrub_hover_ratio
    });
    let hover_bounds = if is_stage {
        app.last_stage_scrub_bounds
    } else {
        app.last_scrub_bounds
    };
    let hover_dur = app.core.queue.current().map(|t| t.duration.max(0)).unwrap_or(0);

    let mut bar = div()
        .id(id)
        .relative()
        .flex_1()
        .h(px(if is_stage { 14.0 } else { 18.0 }))
        .flex()
        .items_center()
        .cursor_pointer()
        .group("scrub")
        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
            if !hovered {
                let slot = if is_stage {
                    &mut this.stage_scrub_hover_ratio
                } else {
                    &mut this.scrub_hover_ratio
                };
                if slot.take().is_some() {
                    cx.notify();
                }
            }
        }))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                let bounds = if is_stage {
                    this.last_stage_scrub_bounds
                } else {
                    this.last_scrub_bounds
                };
                if let Some(b) = bounds {
                    let w = f32::from(b.size.width).max(1.0);
                    let x = f32::from(ev.position.x - b.origin.x).clamp(0.0, w);
                    let slot = if is_stage {
                        &mut this.stage_scrub_drag_ratio
                    } else {
                        &mut this.scrub_drag_ratio
                    };
                    *slot = Some(x / w);
                    cx.notify();
                }
            }),
        )
        .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| {
            let bounds = if is_stage {
                this.last_stage_scrub_bounds
            } else {
                this.last_scrub_bounds
            };
            if let Some(b) = bounds {
                let w = f32::from(b.size.width).max(1.0);
                let x = f32::from(ev.position.x - b.origin.x).clamp(0.0, w);
                let ratio = (x / w).clamp(0.0, 1.0);
                let slot = if is_stage {
                    &mut this.stage_scrub_hover_ratio
                } else {
                    &mut this.scrub_hover_ratio
                };
                if slot.is_none_or(|prev| (prev - ratio).abs() > 0.002) {
                    *slot = Some(ratio);
                    cx.notify();
                }
            }
        }))
        .child({
            let view = view.clone();
            let bounds_view = view.clone();
            canvas(
                move |bounds, _, cx| {
                    bounds_view.update(cx, |this, _| {
                        if is_stage {
                            this.last_stage_scrub_bounds = Some(bounds);
                        } else {
                            this.last_scrub_bounds = Some(bounds);
                        }
                    });
                },
                move |bounds, _, window, cx| {
                    let dragging = view.read(cx);
                    let dragging = if is_stage {
                        dragging.stage_scrub_drag_ratio.is_some()
                    } else {
                        dragging.scrub_drag_ratio.is_some()
                    };
                    if !dragging {
                        return;
                    }
                    let w = f32::from(bounds.size.width).max(1.0);
                    let move_view = view.clone();
                    window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                        if phase != DispatchPhase::Bubble {
                            return;
                        }
                        let ratio =
                            (f32::from(ev.position.x - bounds.origin.x) / w).clamp(0.0, 1.0);
                        move_view.update(cx, |this, cx| {
                            let slot = if is_stage {
                                &mut this.stage_scrub_drag_ratio
                            } else {
                                &mut this.scrub_drag_ratio
                            };
                            if slot.is_some_and(|prev| (prev - ratio).abs() > 0.0005) {
                                *slot = Some(ratio);
                                cx.notify();
                            }
                        });
                    });
                    let up_view = view.clone();
                    window.on_mouse_event(move |ev: &MouseUpEvent, phase, _, cx| {
                        if phase != DispatchPhase::Bubble || ev.button != MouseButton::Left {
                            return;
                        }
                        let ratio =
                            (f32::from(ev.position.x - bounds.origin.x) / w).clamp(0.0, 1.0);
                        up_view.update(cx, |this, cx| {
                            let slot = if is_stage {
                                &mut this.stage_scrub_drag_ratio
                            } else {
                                &mut this.scrub_drag_ratio
                            };
                            if slot.take().is_none() {
                                return;
                            }
                            let dur = this
                                .core.queue
                                .current()
                                .map(|t| t.duration as f64)
                                .unwrap_or(0.0);
                            this.seek_to(ratio as f64 * dur, cx);
                        });
                    });
                },
            )
            .absolute()
            .inset_0()
        })
        .child(
            rail.h(px(if is_stage { 3.0 } else { 5.0 }))
                .group_hover("scrub", move |s| s.h(px(if is_stage { 5.0 } else { 6.0 })))
                .child(fill),
        );

    if let (Some(ratio), Some(bounds)) = (hover_ratio, hover_bounds)
        && hover_dur > 0
    {
        let bar_w = f32::from(bounds.size.width).max(1.0);
        let tip_w = 46.0_f32;
        let tip_left = (ratio * bar_w - tip_w * 0.5).clamp(0.0, (bar_w - tip_w).max(0.0));
        let dot_left = (ratio * bar_w - 4.5).clamp(0.0, (bar_w - 9.0).max(0.0));
        let label = fmt_duration_f64(ratio as f64 * hover_dur as f64);
        bar = bar
            .child(
                div()
                    .absolute()
                    .left(px(dot_left))
                    .size(px(9.0))
                    .rounded_full()
                    .bg(pal.ink),
            )
            .child(
                div()
                    .absolute()
                    .left(px(tip_left))
                    .bottom(px(if is_stage { 16.0 } else { 19.0 }))
                    .w(px(tip_w))
                    .h(px(20.0))
                    .rounded(px(5.0))
                    .bg(pal.raised)
                    .border_1()
                    .border_color(pal.line)
                    .shadow(pal.shadow_pop())
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(pal.ink)
                    .child(label),
            );
    }

    bar.into_any_element()
}

pub fn render_volume_slider(
    id: impl Into<ElementId>,
    volume: f32,
    width_px: Option<f32>,
    is_stage: bool,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let pct = volume.clamp(0.0, 1.0);
    let view = cx.entity();

    let mut base = div()
        .id(id)
        .relative()
        .h(px(16.0))
        .flex()
        .items_center()
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                let bounds = if is_stage {
                    this.last_stage_vol_bounds
                } else {
                    this.last_vol_bounds
                };
                if let Some(b) = bounds {
                    let w = f32::from(b.size.width).max(1.0);
                    let x = f32::from(ev.position.x - b.origin.x).clamp(0.0, w);
                    this.set_volume(x / w, cx);
                }
            }),
        )
        .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| {
            if ev.pressed_button != Some(MouseButton::Left) {
                return;
            }
            let bounds = if is_stage {
                this.last_stage_vol_bounds
            } else {
                this.last_vol_bounds
            };
            if let Some(b) = bounds {
                let w = f32::from(b.size.width).max(1.0);
                let x = f32::from(ev.position.x - b.origin.x).clamp(0.0, w);
                this.set_volume(x / w, cx);
            }
        }));

    if let Some(w) = width_px {
        base = base.w(px(w));
    } else {
        base = base.flex_1();
    }

    base.child({
        let view = view.clone();
        canvas(
            move |bounds, _, cx| {
                view.update(cx, |this, _| {
                    if is_stage {
                        this.last_stage_vol_bounds = Some(bounds);
                    } else {
                        this.last_vol_bounds = Some(bounds);
                    }
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
    })
    .child(
        div()
            .w_full()
            .h(px(4.0))
            .rounded(px(2.0))
            .bg(pal.fill_2)
            .overflow_hidden()
            .child(div().h_full().w(relative(pct)).bg(pal.ink)),
    )
    .into_any_element()
}

fn skeleton_pulse(id: impl Into<ElementId>, content: Div) -> AnyElement {
    content
        .with_animation(
            id,
            Animation::new(Duration::from_millis(1500)).repeat(),
            |el, delta| {
                let wave = (delta * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                el.opacity(0.48 + 0.52 * wave)
            },
        )
        .into_any_element()
}

pub fn skeleton_bar(w: f32, h: f32, pal: &Palette) -> Div {
    div().w(px(w)).h(px(h)).rounded(px(4.0)).bg(pal.fill)
}

pub fn render_action_bar_skeleton(
    id: impl Into<ElementId>,
    pills: &[(f32, f32)],
    with_filter: bool,
    pal: &Palette,
) -> AnyElement {
    let mut left = div().flex().flex_wrap().items_center().gap(px(8.0));
    for &(w, h) in pills {
        left = left.child(div().w(px(w)).h(px(h)).rounded(px(6.0)).bg(pal.fill));
    }

    let mut row = div()
        .mb(px(22.0))
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .child(left);
    if with_filter {
        row = row.child(div().w(px(220.0)).h(px(34.0)).rounded(px(6.0)).bg(pal.fill));
    }

    skeleton_pulse(id, row)
}

fn build_track_list_skeleton_div(
    rows: usize,
    hide_album: bool,
    show_header: bool,
    pal: &Palette,
) -> Div {
    const TITLE_W: [f32; 7] = [190.0, 240.0, 155.0, 215.0, 175.0, 255.0, 165.0];
    const ARTIST_W: [f32; 7] = [115.0, 150.0, 95.0, 130.0, 110.0, 125.0, 100.0];
    const ALBUM_W: [f32; 7] = [135.0, 110.0, 155.0, 95.0, 140.0, 120.0, 105.0];

    let mut list = div().w_full().flex().flex_col();

    if show_header {
        let mut head = div()
            .h(px(32.0))
            .px(px(10.0))
            .mb(px(4.0))
            .border_b_1()
            .border_color(pal.line)
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(
                div()
                    .w(px(32.0))
                    .flex_shrink_0()
                    .flex()
                    .justify_center()
                    .child(div().w(px(12.0)).h(px(10.0)).rounded(px(3.0)).bg(pal.fill)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(div().w(px(48.0)).h(px(10.0)).rounded(px(3.0)).bg(pal.fill)),
            );
        if !hide_album {
            head = head.child(
                div()
                    .w(px(190.0))
                    .flex_shrink_0()
                    .child(div().w(px(58.0)).h(px(10.0)).rounded(px(3.0)).bg(pal.fill)),
            );
        }
        head = head
            .child(div().w(px(34.0)).flex_shrink_0())
            .child(
                div()
                    .w(px(50.0))
                    .flex_shrink_0()
                    .flex()
                    .justify_end()
                    .child(div().w(px(38.0)).h(px(10.0)).rounded(px(3.0)).bg(pal.fill)),
            )
            .child(div().w(px(32.0)).flex_shrink_0());
        list = list.child(head);
    }

    for idx in 0..rows {
        let tw = TITLE_W[idx % TITLE_W.len()];
        let aw = ARTIST_W[idx % ARTIST_W.len()];
        let alw = ALBUM_W[idx % ALBUM_W.len()];
        let fade = (1.0 - idx as f32 * 0.08).max(0.32);

        let mut row = div()
            .h(px(TRACK_ROW_H))
            .px(px(10.0))
            .rounded(px(6.0))
            .opacity(fade)
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(
                div()
                    .w(px(32.0))
                    .flex_shrink_0()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .w(px(14.0))
                            .h(px(10.0))
                            .rounded(px(3.0))
                            .bg(pal.fill.opacity(0.75)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .children((!hide_album).then(|| {
                        div()
                            .size(px(38.0))
                            .rounded(px(3.0))
                            .flex_shrink_0()
                            .bg(pal.fill)
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(7.0))
                            .child(div().w(px(tw)).h(px(12.5)).rounded(px(4.0)).bg(pal.fill))
                            .child(
                                div()
                                    .w(px(aw))
                                    .h(px(10.5))
                                    .rounded(px(4.0))
                                    .bg(pal.fill.opacity(0.65)),
                            ),
                    ),
            );

        if !hide_album {
            row = row.child(
                div().w(px(190.0)).flex_shrink_0().child(
                    div()
                        .w(px(alw))
                        .h(px(11.0))
                        .rounded(px(4.0))
                        .bg(pal.fill.opacity(0.65)),
                ),
            );
        }

        row = row
            .child(div().w(px(34.0)).flex_shrink_0())
            .child(
                div()
                    .w(px(50.0))
                    .flex_shrink_0()
                    .flex()
                    .justify_end()
                    .child(
                        div()
                            .w(px(34.0))
                            .h(px(10.5))
                            .rounded(px(4.0))
                            .bg(pal.fill.opacity(0.65)),
                    ),
            )
            .child(div().w(px(32.0)).flex_shrink_0());

        list = list.child(row);
    }

    list
}

fn build_card_shelf_skeleton_div(
    cards: usize,
    round: bool,
    heading_w: Option<f32>,
    pal: &Palette,
) -> Div {
    const CARD_T: [f32; 5] = [126.0, 108.0, 142.0, 96.0, 132.0];
    const CARD_S: [f32; 5] = [88.0, 102.0, 76.0, 94.0, 82.0];

    let mut shelf = div().w_full().flex().flex_col();
    if let Some(hw) = heading_w {
        shelf = shelf.child(
            div()
                .mb(px(14.0))
                .w(px(hw))
                .h(px(20.0))
                .rounded(px(5.0))
                .bg(pal.fill),
        );
    }

    let mut grid = div()
        .mb(px(36.0))
        .flex()
        .flex_wrap()
        .gap_x(px(18.0))
        .gap_y(px(22.0));

    for idx in 0..cards {
        let tw = CARD_T[idx % CARD_T.len()];
        let sw = CARD_S[idx % CARD_S.len()];
        let fade = (1.0 - idx as f32 * 0.08).max(0.45);
        let cover = div().size(px(164.0)).bg(pal.fill);
        let cover = if round {
            cover.rounded_full()
        } else {
            cover.rounded(px(4.0))
        };

        let mut card = div()
            .w(px(164.0))
            .opacity(fade)
            .flex()
            .flex_col()
            .child(div().mb(px(10.0)).child(cover));

        if round {
            card = card.items_center().child(
                div()
                    .w(px(tw * 0.8))
                    .h(px(13.0))
                    .rounded(px(4.0))
                    .bg(pal.fill),
            );
        } else {
            card = card
                .child(
                    div()
                        .mb(px(6.0))
                        .w(px(tw))
                        .h(px(13.0))
                        .rounded(px(4.0))
                        .bg(pal.fill),
                )
                .child(
                    div()
                        .w(px(sw))
                        .h(px(11.0))
                        .rounded(px(4.0))
                        .bg(pal.fill.opacity(0.65)),
                );
        }
        grid = grid.child(card);
    }

    shelf.child(grid)
}

pub fn render_track_list_skeleton(
    id: impl Into<ElementId>,
    rows: usize,
    hide_album: bool,
    show_header: bool,
    pal: &Palette,
) -> AnyElement {
    skeleton_pulse(
        id,
        build_track_list_skeleton_div(rows, hide_album, show_header, pal),
    )
}

pub fn render_music_page_skeleton(
    id: impl Into<ElementId>,
    shelf_cards: usize,
    track_rows: usize,
    hide_album: bool,
    pal: &Palette,
) -> AnyElement {
    let mut col = div().w_full().flex().flex_col();
    if shelf_cards > 0 {
        col = col.child(build_card_shelf_skeleton_div(
            shelf_cards,
            false,
            Some(168.0),
            pal,
        ));
    }
    col = col
        .child(
            div()
                .mb(px(14.0))
                .w(px(86.0))
                .h(px(20.0))
                .rounded(px(5.0))
                .bg(pal.fill),
        )
        .child(build_track_list_skeleton_div(track_rows, hide_album, true, pal));
    skeleton_pulse(id, col)
}

pub fn render_friends_grid_skeleton(
    id: impl Into<ElementId>,
    count: usize,
    pal: &Palette,
) -> AnyElement {
    const NAME_W: [f32; 6] = [92.0, 108.0, 78.0, 102.0, 86.0, 98.0];
    let mut grid = div().flex().flex_wrap().gap(px(24.0));
    for idx in 0..count {
        let nw = NAME_W[idx % NAME_W.len()];
        let fade = (1.0 - idx as f32 * 0.04).max(0.45);
        grid = grid.child(
            div()
                .w(px(136.0))
                .p(px(10.0))
                .opacity(fade)
                .flex()
                .flex_col()
                .items_center()
                .child(
                    div()
                        .mb(px(10.0))
                        .size(px(116.0))
                        .rounded_full()
                        .bg(pal.fill),
                )
                .child(div().w(px(nw)).h(px(13.0)).rounded(px(4.0)).bg(pal.fill)),
        );
    }
    skeleton_pulse(id, grid)
}

pub fn render_home_skeleton(id: impl Into<ElementId>, pal: &Palette) -> AnyElement {
    let col = div()
        .w_full()
        .flex()
        .flex_col()
        .child(build_card_shelf_skeleton_div(5, false, Some(210.0), pal))
        .child(build_card_shelf_skeleton_div(5, false, Some(175.0), pal));
    skeleton_pulse(id, col)
}

pub fn render_search_skeleton(id: impl Into<ElementId>, pal: &Palette) -> AnyElement {
    let col = div()
        .w_full()
        .flex()
        .flex_col()
        .child(
            div()
                .mb(px(14.0))
                .w(px(80.0))
                .h(px(20.0))
                .rounded(px(5.0))
                .bg(pal.fill),
        )
        .child(div().mb(px(32.0)).child(build_track_list_skeleton_div(5, false, true, pal)))
        .child(build_card_shelf_skeleton_div(4, true, Some(150.0), pal))
        .child(build_card_shelf_skeleton_div(4, false, Some(190.0), pal));
    skeleton_pulse(id, col)
}

pub fn render_section_skeleton(id: impl Into<ElementId>, pal: &Palette) -> AnyElement {
    let col = div()
        .w_full()
        .flex()
        .flex_col()
        .child(
            div()
                .mb(px(22.0))
                .w(px(116.0))
                .h(px(40.0))
                .rounded(px(6.0))
                .bg(pal.fill),
        )
        .child(build_card_shelf_skeleton_div(5, false, None, pal))
        .child(build_track_list_skeleton_div(6, false, false, pal));
    skeleton_pulse(id, col)
}

pub fn render_sidebar_playlists_skeleton(
    id: impl Into<ElementId>,
    rows: usize,
    pal: &Palette,
) -> AnyElement {
    const TITLE_W: [f32; 5] = [118.0, 96.0, 134.0, 108.0, 88.0];
    let mut col = div().flex().flex_col().gap(px(2.0));
    for idx in 0..rows {
        let tw = TITLE_W[idx % TITLE_W.len()];
        let fade = (1.0 - idx as f32 * 0.14).max(0.32);
        col = col.child(
            div()
                .px(px(8.0))
                .py(px(5.0))
                .opacity(fade)
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(
                    div()
                        .size(px(38.0))
                        .rounded(px(3.0))
                        .flex_shrink_0()
                        .bg(pal.fill),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .child(div().w(px(tw)).h(px(11.5)).rounded(px(4.0)).bg(pal.fill))
                        .child(
                            div()
                                .w(px(74.0))
                                .h(px(9.5))
                                .rounded(px(4.0))
                                .bg(pal.fill.opacity(0.65)),
                        ),
                ),
        );
    }
    skeleton_pulse(id, col)
}

pub fn render_empty_state(
    ic: Icon,
    title: impl Into<String>,
    subtitle: impl Into<String>,
    actions: Option<AnyElement>,
    pal: &Palette,
) -> AnyElement {
    let mut col = div()
        .w_full()
        .py(px(56.0))
        .px(px(24.0))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .child(
            div()
                .size(px(52.0))
                .rounded_full()
                .bg(pal.fill)
                .border_1()
                .border_color(pal.line)
                .flex()
                .items_center()
                .justify_center()
                .child(icon(ic, 24.0, pal.ink_2)),
        )
        .child(
            div()
                .mt(px(6.0))
                .text_size(px(16.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink)
                .text_center()
                .child(title.into()),
        )
        .child(
            div()
                .max_w(px(380.0))
                .text_size(px(13.5))
                .line_height(relative(1.45))
                .text_color(pal.ink_2)
                .text_center()
                .child(subtitle.into()),
        );

    if let Some(act) = actions {
        col = col.child(div().mt(px(10.0)).child(act));
    }

    col.into_any_element()
}

pub fn render_error_state(
    title: impl Into<String>,
    detail: impl Into<String>,
    retry_btn: AnyElement,
    pal: &Palette,
) -> AnyElement {
    div()
        .w_full()
        .py(px(48.0))
        .px(px(24.0))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(15.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink)
                .text_center()
                .child(title.into()),
        )
        .child(
            div()
                .max_w(px(400.0))
                .text_size(px(13.0))
                .line_height(relative(1.45))
                .text_color(pal.ink_2)
                .text_center()
                .child(detail.into()),
        )
        .child(div().mt(px(8.0)).child(retry_btn))
        .into_any_element()
}
