use gpui_kit::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, px,
};
use vively_core::playback::PlaybackOrigin;

use crate::{
    design::{colors::Palette, icons::Icon},
    state::{app_state::VivelyApp, models::LoadState},
    ui::{
        components::{
            HeroArt, TrackTableOptions, cached_queue, pill_button, primary_play_pill,
            render_action_bar_skeleton, render_artist_card, render_empty_state, render_error_state,
            render_hero_banner, render_music_page_skeleton, render_playlist_card,
            render_track_list,
        },
        pages::home::render_music_section_block,
    },
};

pub fn render_artist_page(
    app: &VivelyApp,
    artist_id: &str,
    center_w: f32,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let page_data = app.core.artist_pages.get(artist_id);
    let name = page_data
        .as_ref()
        .and_then(|p| p.info.as_ref())
        .map(|i| i.name.trim())
        .filter(|name| !name.is_empty() && *name != artist_id)
        .or_else(|| app.artist_name_hints.get(artist_id).map(String::as_str))
        .unwrap_or("Исполнитель")
        .to_owned();
    let photo_url = page_data
        .as_ref()
        .and_then(|p| p.info.as_ref())
        .and_then(|i| i.best_photo().cloned());
    let cover_path = app.resolve_artwork(photo_url.as_ref(), cx);
    let tint = app.tint_for_artwork(photo_url.as_ref(), None);

    let play_tracks = cached_queue(
        app,
        &format!("ar-all-{artist_id}"),
        page_data.map(|p| p.tracks.as_slice()).unwrap_or_default(),
    );
    let from_name = name.clone();
    let mix_aid = artist_id.to_owned();
    let mix_label = format!("Микс: {name}");

    let is_loading = page_data.is_none_or(|d| {
        matches!(d.state, LoadState::Idle | LoadState::Loading)
            && d.sections.is_empty()
            && d.tracks.is_empty()
    });
    let action_bar: AnyElement = if is_loading {
        render_action_bar_skeleton(
            "artist-actions-skel",
            &[(116.0, 40.0), (168.0, 40.0)],
            false,
            pal,
        )
    } else {
        let mut bar = div()
            .mb(px(22.0))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.0));
        if !play_tracks.is_empty() {
            bar = bar.child(
                primary_play_pill("ar-play", Icon::Play, "Слушать", pal).on_click(cx.listener(
                    move |this, _, _, cx| {
                        if !play_tracks.is_empty() {
                            this.play_tracks(
                                play_tracks.to_vec(),
                                0,
                                PlaybackOrigin::Other,
                                from_name.clone(),
                                None,
                                cx,
                            );
                        }
                    },
                )),
            );
        }
        bar = bar.child(
            pill_button("ar-mix", Some(Icon::Wave), "Микс по артисту", false, pal)
                .h(px(40.0))
                .px(px(15.0))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.start_vk_mix(
                        "artist".into(),
                        Some(mix_aid.clone()),
                        None,
                        mix_label.clone(),
                        cx,
                    );
                })),
        );
        if let Some(info) = page_data.and_then(|p| p.info.as_ref()) {
            let followed = info.is_followed.unwrap_or(false);
            let follow_id = artist_id.to_owned();
            bar = bar.child(
                pill_button(
                    "ar-follow",
                    Some(if followed { Icon::Check } else { Icon::Plus }),
                    if followed {
                        "Вы подписаны"
                    } else {
                        "Подписаться"
                    },
                    followed,
                    pal,
                )
                .h(px(40.0))
                .px(px(15.0))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_artist_follow(follow_id.clone(), cx);
                })),
            );
        }
        bar.into_any_element()
    };

    let mut page = div()
        .pt(px(8.0))
        .px(px(32.0))
        .pb(px(40.0))
        .flex()
        .flex_col()
        .child(render_hero_banner(
            HeroArt::Cover {
                local_path: cover_path,
                round: true,
            },
            tint,
            &name,
            None,
            None,
            &[],
            pal,
        ))
        .child(action_bar);

    let Some(data) = page_data else {
        return page
            .child(render_music_page_skeleton("artist-init-skel", 4, 5, false, pal))
            .into_any_element();
    };

    match &data.state {
        LoadState::Idle | LoadState::Loading
            if data.sections.is_empty() && data.tracks.is_empty() =>
        {
            page = page.child(render_music_page_skeleton(
                "artist-load-skel",
                4,
                5,
                false,
                pal,
            ));
        }
        LoadState::Failed(err) if data.sections.is_empty() && data.tracks.is_empty() => {
            let aid_retry = artist_id.to_owned();
            let retry = pill_button("artist-retry", None, "Повторить", false, pal)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.load_artist(&aid_retry, true, cx);
                }))
                .into_any_element();
            page = page.child(render_error_state(
                "Не удалось загрузить страницу исполнителя",
                err.clone(),
                retry,
                pal,
            ));
        }
        _ if data.sections.is_empty() && data.tracks.is_empty() && data.playlists.is_empty() => {
            page = page.child(render_empty_state(
                Icon::Mic,
                "Нет доступных треков",
                "У этого исполнителя пока нет доступных аудиозаписей и релизов",
                None,
                pal,
            ));
        }
        _ => {
            if !data.sections.is_empty() {
                for (idx, sec) in data.sections.iter().enumerate() {
                    page = page.child(render_music_section_block(
                        app,
                        &format!("ar-{artist_id}-{idx}"),
                        sec,
                        center_w,
                        pal,
                        cx,
                    ));
                }
            } else {
                if !data.tracks.is_empty() {
                    let slice: Vec<_> = data.tracks.iter().take(10).cloned().collect();
                    page = page
                        .child(
                            div()
                                .mb(px(14.0))
                                .text_size(px(20.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(pal.ink)
                                .child("Популярные треки"),
                        )
                        .child(div().mb(px(36.0)).child(render_track_list(
                            app,
                            &slice,
                            TrackTableOptions {
                                id_prefix: format!("ar-pop-{artist_id}"),
                                is_album: false,
                                album_artist: None,
                                hide_album: false,
                                no_header: false,
                                from_label: name.clone(),
                                origin: PlaybackOrigin::Other,
                                editable_playlist: None,
                            },
                            pal,
                            cx,
                        )));
                }
                if !data.playlists.is_empty() {
                    page = page
                        .child(
                            div()
                                .mb(px(14.0))
                                .text_size(px(20.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(pal.ink)
                                .child("Релизы и плейлисты"),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_x(px(18.0))
                                .gap_y(px(22.0))
                                .children(data.playlists.iter().map(|pl| {
                                    render_playlist_card(app, "ar-pl", pl, false, pal, cx)
                                })),
                        );
                }
            }
        }
    }

    if !data.related.is_empty() {
        page = page
            .child(
                div()
                    .mt(px(36.0))
                    .mb(px(14.0))
                    .text_size(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(pal.ink)
                    .child("Похожие исполнители"),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_x(px(18.0))
                    .gap_y(px(22.0))
                    .children(data.related.iter().map(|ar| {
                        render_artist_card(
                            app,
                            format!("ar-rel-{artist_id}-{}", ar.id),
                            &ar.id,
                            &ar.name,
                            ar.best_photo(),
                            pal,
                            cx,
                        )
                    })),
            );
    }

    page.into_any_element()
}
