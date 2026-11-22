use gpui_kit::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _,
    component::scroll::ScrollableElement as _, div, px, relative,
};
use vively_core::{
    catalog::{HomeBlockKind, MusicContent, MusicSection, TrackStyle},
    models::ArtistMixCard,
    playback::PlaybackOrigin,
};

use crate::{
    design::{
        artwork::{render_cover, render_liked_cover},
        colors::Palette,
        icons::{Icon, icon},
    },
    state::{
        app_state::VivelyApp,
        models::LoadState,
        router::{ActiveModal, RecentTargetKind, Route, SectionTarget},
    },
    ui::components::{
        TrackTableOptions, mood_chip, pill_button, primary_play_pill, render_error_state,
        render_home_skeleton, render_playlist_card, render_recommended_card, render_track_list,
    },
};

pub fn render_home_page(
    app: &VivelyApp,
    center_w: f32,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let mix_active = app.core.active_mix_session.is_some();
    let mix_playing = mix_active && app.core.audio_engine.is_playing();
    let mix_title = app
        .core.primary_mix
        .as_ref()
        .map(|m| m.display_title().to_owned())
        .unwrap_or_else(|| "VK Микс".into());
    let mix_id = app
        .core.primary_mix
        .as_ref()
        .map(|m| m.id.clone())
        .unwrap_or_else(|| "common".into());

    let mut mood_wrap = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(6.0))
        .mb(px(20.0));
    if let Some(first_cat) = app.core.mix_settings.mix_categories.first() {
        let cat_id = first_cat.id.clone();
        let selected_list = app.core.mix_picks.get(&cat_id).cloned().unwrap_or_default();
        for opt in &first_cat.options {
            let sel = selected_list.contains(&opt.id);
            let cid = cat_id.clone();
            let oid = opt.id.clone();
            mood_wrap = mood_wrap.child(
                mood_chip(
                    format!("home-mood-{cid}-{oid}"),
                    opt.title.clone(),
                    sel,
                    pal,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_mix_option(&cid, &oid, cx);
                })),
            );
        }
    }
    let has_picks = app.core.mix_picks.values().any(|v| !v.is_empty());
    if has_picks {
        let hover_ink = pal.ink;
        mood_wrap = mood_wrap.child(
            div()
                .id("home-reset-mix")
                .ml(px(8.0))
                .text_size(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink_2)
                .cursor_pointer()
                .hover(move |s| s.text_color(hover_ink))
                .on_click(cx.listener(|this, _, _, cx| this.clear_mix_picks(cx)))
                .child("Сбросить"),
        );
    }

    let tune_mix_id = mix_id.clone();
    let fill_2 = pal.fill_2;
    let wave_actions = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(8.0))
        .child(
            primary_play_pill(
                "play-vkmix-btn",
                if mix_playing { Icon::Pause } else { Icon::Play },
                if mix_playing {
                    "Пауза"
                } else {
                    "Слушать"
                },
                pal,
            )
            .on_click(cx.listener(|this, _, _, cx| this.toggle_or_start_home_mix(cx))),
        )
        .child(
            div()
                .id("open-mix-tune-btn")
                .h(px(40.0))
                .px(px(14.0))
                .rounded(px(6.0))
                .bg(pal.fill)
                .text_color(pal.ink)
                .flex()
                .items_center()
                .gap(px(7.0))
                .cursor_pointer()
                .hover(move |s| s.bg(fill_2))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.router.modal = Some(ActiveModal::MixTune {
                        mix_id: tune_mix_id.clone(),
                    });
                    cx.notify();
                }))
                .child(icon(Icon::Sliders, 18.0, pal.ink))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Настроить"),
                ),
        );

    let wave_hero = div()
        .flex_1()
        .min_w(px(280.0))
        .pt(px(26.0))
        .px(px(28.0))
        .pb(px(24.0))
        .rounded(px(10.0))
        .bg(pal.wave_hero_bg())
        .border_1()
        .border_color(pal.line)
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(px(34.0))
                .font_weight(FontWeight::BOLD)
                .line_height(relative(1.1))
                .text_color(pal.ink)
                .child(mix_title),
        )
        .child(
            div()
                .mt(px(8.0))
                .mb(px(16.0))
                .max_w(px(420.0))
                .text_size(px(15.0))
                .text_color(pal.ink_2)
                .child("Бесконечный поток треков под ваш вкус."),
        )
        .child(mood_wrap)
        .child(wave_actions);

    let quick_block = build_quick_links_block(app, pal, cx);

    let lead = if center_w >= 860.0 {
        div()
            .mt(px(8.0))
            .mb(px(44.0))
            .flex()
            .items_start()
            .gap(px(32.0))
            .child(div().flex_1().min_w_0().child(wave_hero))
            .child(div().w(px(290.0)).flex_shrink_0().child(quick_block))
    } else {
        div()
            .mt(px(8.0))
            .mb(px(44.0))
            .flex()
            .flex_col()
            .gap(px(24.0))
            .child(wave_hero)
            .child(quick_block)
    };

    let mut page = div()
        .pt(px(8.0))
        .px(px(32.0))
        .pb(px(40.0))
        .flex()
        .flex_col()
        .child(lead);

    match &app.core.home_state {
        LoadState::Idle | LoadState::Loading if app.core.home_blocks.is_empty() => {
            page = page.child(render_home_skeleton("home-loading-skel", pal));
        }
        LoadState::Failed(err) if app.core.home_blocks.is_empty() => {
            let retry = pill_button("retry-home", None, "Повторить", false, pal)
                .on_click(cx.listener(|this, _, _, cx| this.load_home(true, cx)))
                .into_any_element();
            page = page.child(render_error_state(
                "Не удалось загрузить Главную",
                err.clone(),
                retry,
                pal,
            ));
        }
        _ => {}
    }

    for block in &app.core.home_blocks {
        match &block.kind {
            HomeBlockKind::StreamMixes(_) | HomeBlockKind::Links(_) => {}
            HomeBlockKind::Recommended {
                title,
                show_all_section_id,
                playlists,
            } => {
                if playlists.is_empty() {
                    continue;
                }
                page = page
                    .child(render_section_header(
                        &format!("home-rec-{}", block.id),
                        title,
                        None,
                        show_all_section_id.as_deref(),
                        pal,
                        cx,
                    ))
                    .child(
                        div()
                            .mb(px(40.0))
                            .flex()
                            .flex_wrap()
                            .gap_x(px(18.0))
                            .gap_y(px(22.0))
                            .children(
                                playlists
                                    .iter()
                                    .map(|rec| render_recommended_card(app, rec, pal, cx)),
                            ),
                    );
            }
            HomeBlockKind::MixTiles { title, tiles } => {
                if tiles.is_empty() {
                    continue;
                }
                let mut row = div().flex().gap(px(8.0)).flex_none().pb(px(8.0));
                for tile in tiles {
                    let mix_id = tile.mix_id.clone();
                    let options = tile.options_json.clone();
                    let label = tile.title.clone();
                    row = row.child(
                        pill_button(
                            format!("mix-tile-{}", tile.id),
                            None,
                            tile.title.clone(),
                            false,
                            pal,
                        )
                        .h(px(42.0))
                        .px(px(18.0))
                        .flex_shrink_0()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.start_vk_mix(
                                mix_id.clone(),
                                None,
                                options.clone(),
                                label.clone(),
                                cx,
                            );
                        })),
                    );
                }
                page = page
                    .child(render_section_header(
                        &format!("home-tiles-{}", block.id),
                        title,
                        None,
                        None,
                        pal,
                        cx,
                    ))
                    .child(
                        div()
                            .id(format!("home-tiles-scroll-{}", block.id))
                            .mb(px(34.0))
                            .min_w_0()
                            .overflow_x_scrollbar()
                            .child(row),
                    );
            }
            HomeBlockKind::ArtistMixes { title, cards } => {
                if cards.is_empty() {
                    continue;
                }
                page = page
                    .child(render_section_header(
                        &format!("home-amix-{}", block.id),
                        title,
                        None,
                        None,
                        pal,
                        cx,
                    ))
                    .child(
                        div()
                            .mb(px(40.0))
                            .flex()
                            .flex_wrap()
                            .gap_x(px(18.0))
                            .gap_y(px(22.0))
                            .children(
                                cards
                                    .iter()
                                    .enumerate()
                                    .map(|(i, c)| render_artist_mix_card(app, i, c, pal, cx)),
                            ),
                    );
            }
            HomeBlockKind::Music(section) => {
                page = page.child(render_music_section_block(
                    app,
                    &format!("home-sec-{}", block.id),
                    section,
                    center_w,
                    pal,
                    cx,
                ));
            }
        }
    }

    page.into_any_element()
}

fn build_quick_links_block(
    app: &VivelyApp,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let vk_links: Vec<_> = app
        .core.home_blocks
        .iter()
        .find_map(|b| match &b.kind {
            HomeBlockKind::Links(links) => Some(links.iter().take(4).cloned().collect()),
            _ => None,
        })
        .unwrap_or_default();

    let hover_bg = pal.hover;
    let mut list = div().flex().flex_col().gap(px(2.0));

    if !vk_links.is_empty() {
        for (idx, link) in vk_links.into_iter().enumerate() {
            let cov = app.resolve_artwork(link.artwork_url(), cx);
            let is_lib = link.is_my_audios();
            let link_clone = link.clone();
            let cover_el: AnyElement = if is_lib {
                render_liked_cover(46.0, pal.art_ring).into_any_element()
            } else {
                render_cover(cov, 46.0, false, pal).into_any_element()
            };

            let mut text_col = div().flex_1().min_w_0().flex().flex_col().child(
                div()
                    .text_size(px(15.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(pal.ink)
                    .truncate()
                    .child(link.title.clone()),
            );
            if let Some(sub) = link.subtitle.as_deref().filter(|s| !s.is_empty()) {
                text_col = text_col.child(
                    div()
                        .text_size(px(13.0))
                        .text_color(pal.ink_2)
                        .truncate()
                        .child(sub.to_owned()),
                );
            }

            list = list.child(
                div()
                    .id(format!("ql-vk-{idx}"))
                    .mx(px(-6.0))
                    .p(px(6.0))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover_bg))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_catalog_link(&link_clone, window, cx);
                    }))
                    .child(cover_el)
                    .child(text_col),
            );
        }
    } else {
        list = list.child(
            div()
                .id("ql-my-music")
                .mx(px(-6.0))
                .p(px(6.0))
                .rounded(px(6.0))
                .flex()
                .items_center()
                .gap(px(12.0))
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.navigate_to(Route::Library, window, cx);
                }))
                .child(render_liked_cover(46.0, pal.art_ring))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(pal.ink)
                                .child("Моя музыка"),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(pal.ink_2)
                                .child("Любимые треки и плейлисты"),
                        ),
                ),
        );

        for (idx, item) in app.core.recent_items.iter().take(3).enumerate() {
            let cov = app.resolve_artwork(item.artwork_url.as_ref(), cx);
            let is_round = matches!(item.kind, RecentTargetKind::Artist { .. });
            let kind_clone = item.kind.clone();
            let mut text_col = div().flex_1().min_w_0().flex().flex_col().child(
                div()
                    .text_size(px(15.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(pal.ink)
                    .truncate()
                    .child(item.title.clone()),
            );
            if let Some(sub) = &item.subtitle {
                text_col = text_col.child(
                    div()
                        .text_size(px(13.0))
                        .text_color(pal.ink_2)
                        .truncate()
                        .child(sub.clone()),
                );
            }

            list = list.child(
                div()
                    .id(format!("ql-rec-{idx}"))
                    .mx(px(-6.0))
                    .p(px(6.0))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover_bg))
                    .on_click(cx.listener(move |this, _, window, cx| match &kind_clone {
                        RecentTargetKind::Playlist(pl) => {
                            this.open_playlist(pl.clone(), window, cx)
                        }
                        RecentTargetKind::Artist { id, name } => {
                            this.open_artist(id.clone(), name, window, cx)
                        }
                        RecentTargetKind::Library => this.navigate_to(Route::Library, window, cx),
                    }))
                    .child(render_cover(cov, 46.0, is_round, pal))
                    .child(text_col),
            );
        }
    }

    div()
        .min_w_0()
        .flex()
        .flex_col()
        .child(
            div()
                .mb(px(8.0))
                .text_size(px(20.0))
                .font_weight(FontWeight::BOLD)
                .text_color(pal.ink)
                .child("Недавнее"),
        )
        .child(list)
        .into_any_element()
}

pub fn render_section_header(
    id_prefix: &str,
    title: &str,
    subtitle: Option<&str>,
    show_all_id: Option<&str>,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let mut heading = div().flex().flex_col().min_w_0().gap(px(4.0)).child(
        div()
            .text_size(px(20.0))
            .font_weight(FontWeight::BOLD)
            .line_height(relative(1.2))
            .text_color(pal.ink)
            .child(title.to_owned()),
    );
    if let Some(subtitle) = subtitle.map(str::trim).filter(|s| !s.is_empty()) {
        heading = heading.child(
            div()
                .text_size(px(14.0))
                .text_color(pal.ink_2)
                .child(subtitle.to_owned()),
        );
    }

    let mut row = div()
        .mb(px(14.0))
        .flex()
        .items_start()
        .justify_between()
        .gap(px(16.0))
        .child(heading);

    if let Some(sec_id) = show_all_id.filter(|s| !s.is_empty()) {
        let sid = sec_id.to_owned();
        let stitle = title.to_owned();
        let hover_ink = pal.ink;
        row = row.child(
            div()
                .id(format!("{id_prefix}-more"))
                .flex_shrink_0()
                .text_size(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink_2)
                .cursor_pointer()
                .hover(move |s| s.text_color(hover_ink))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_section(
                        SectionTarget::Id {
                            section_id: sid.clone(),
                            title: stitle.clone(),
                        },
                        window,
                        cx,
                    );
                }))
                .child("Показать все"),
        );
    }

    row.into_any_element()
}

pub fn render_music_section_block(
    app: &VivelyApp,
    id_prefix: &str,
    section: &MusicSection,
    center_w: f32,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let mut block = div().mb(px(40.0)).flex().flex_col();
    if !section.title.is_empty() {
        block = block.child(render_section_header(
            id_prefix,
            &section.title,
            section.subtitle.as_deref(),
            section.show_all_section_id.as_deref(),
            pal,
            cx,
        ));
    }

    let from_label = if section.title.is_empty() {
        "Каталог".to_owned()
    } else {
        section.title.clone()
    };

    match &section.content {
        MusicContent::Tracks(tracks) => {
            if section.track_style == TrackStyle::Columns && center_w >= 860.0 && tracks.len() >= 2
            {
                let mid = tracks.len().div_ceil(2).min(4);
                let left = &tracks[..mid.min(tracks.len())];
                let right = &tracks[mid.min(tracks.len())..(mid * 2).min(tracks.len())];
                block = block.child(
                    div()
                        .flex()
                        .gap(px(24.0))
                        .child(div().flex_1().min_w_0().child(render_track_list(
                            app,
                            left,
                            TrackTableOptions {
                                id_prefix: format!("{id_prefix}-l"),
                                is_album: false,
                                album_artist: None,
                                hide_album: true,
                                no_header: true,
                                from_label: from_label.clone(),
                                origin: PlaybackOrigin::Other,
                                editable_playlist: None,
                            },
                            pal,
                            cx,
                        )))
                        .child(div().flex_1().min_w_0().child(render_track_list(
                            app,
                            right,
                            TrackTableOptions {
                                id_prefix: format!("{id_prefix}-r"),
                                is_album: false,
                                album_artist: None,
                                hide_album: true,
                                no_header: true,
                                from_label,
                                origin: PlaybackOrigin::Other,
                                editable_playlist: None,
                            },
                            pal,
                            cx,
                        ))),
                );
            } else {
                block = block.child(render_track_list(
                    app,
                    tracks,
                    TrackTableOptions {
                        id_prefix: id_prefix.to_owned(),
                        is_album: false,
                        album_artist: None,
                        hide_album: false,
                        no_header: false,
                        from_label,
                        origin: PlaybackOrigin::Other,
                        editable_playlist: None,
                    },
                    pal,
                    cx,
                ));
            }
        }
        MusicContent::Playlists(playlists) => {
            block = block.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_x(px(18.0))
                    .gap_y(px(22.0))
                    .children(
                        playlists
                            .iter()
                            .map(|pl| render_playlist_card(app, id_prefix, pl, false, pal, cx)),
                    ),
            );
        }
    }

    block.into_any_element()
}

fn render_artist_mix_card(
    app: &VivelyApp,
    idx: usize,
    card: &ArtistMixCard,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let cov = app.resolve_artwork(card.photo.as_ref(), cx);
    let mix_id = card.mix_id.clone();
    let entity_id = card.entity_id.clone();
    let lbl = card.title.clone();
    let cta_bg = pal.cta_bg;
    let cta_fg = pal.cta_fg;
    let cta_hover = pal.cta_hover;

    let amix_grp = SharedString::from(format!("grp-amix-{idx}-{}", card.id));
    let amix_grp_fab = amix_grp.clone();
    div()
        .group(amix_grp)
        .id(format!("amix-{idx}-{}", card.id))
        .w(px(164.0))
        .flex()
        .flex_col()
        .items_center()
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| {
            this.start_vk_mix(mix_id.clone(), entity_id.clone(), None, lbl.clone(), cx);
        }))
        .child(
            div()
                .relative()
                .size(px(164.0))
                .mb(px(10.0))
                .child(render_cover(cov, 164.0, true, pal).shadow(pal.shadow_art()))
                .child(
                    div()
                        .absolute()
                        .left(px(64.0))
                        .top(px(64.0))
                        .size(px(36.0))
                        .rounded_full()
                        .bg(cta_bg)
                        .flex()
                        .items_center()
                        .justify_center()
                        .opacity(0.0)
                        .group_hover(amix_grp_fab, |s| s.opacity(1.0))
                        .hover(move |s| s.bg(cta_hover))
                        .child(icon(Icon::Wave, 18.0, cta_fg)),
                ),
        )
        .child(
            div()
                .w_full()
                .text_center()
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink)
                .truncate()
                .child(card.title.clone()),
        )
        .children(card.description.as_ref().map(|d| {
            div()
                .mt(px(1.0))
                .w_full()
                .text_center()
                .text_size(px(13.0))
                .text_color(pal.ink_2)
                .truncate()
                .child(d.clone())
        }))
        .into_any_element()
}
