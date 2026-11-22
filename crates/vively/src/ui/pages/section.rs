use gpui_kit::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, px, relative,
};
use vively_core::{catalog::MusicContent, models::Audio, playback::PlaybackOrigin};

use crate::{
    design::{
        colors::Palette,
        icons::{Icon, spinner},
    },
    state::{app_state::VivelyApp, models::LoadState, router::SectionTarget},
    ui::{
        components::{
            pill_button, primary_play_pill, render_empty_state, render_error_state,
            render_section_skeleton,
        },
        pages::home::render_music_section_block,
    },
};

pub fn render_section_page(
    app: &VivelyApp,
    target: &SectionTarget,
    center_w: f32,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let key = target.key();
    let page_data = app.core.catalog_pages.get(&key);
    let title = page_data
        .map(|p| p.title.clone())
        .unwrap_or_else(|| target.fallback_title().to_owned());

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
                .child(title.clone()),
        );

    let Some(data) = page_data else {
        return page
            .child(render_section_skeleton("sec-init-skel", pal))
            .into_any_element();
    };

    match &data.state {
        LoadState::Idle | LoadState::Loading if data.sections.is_empty() => {
            return page
                .child(render_section_skeleton("sec-load-skel", pal))
                .into_any_element();
        }
        LoadState::Failed(err) if data.sections.is_empty() => {
            let tgt_retry = target.clone();
            let retry = pill_button("sec-retry", None, "Повторить", false, pal)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.load_section(tgt_retry.clone(), true, cx);
                }))
                .into_any_element();
            return page
                .child(render_error_state(
                    "Не удалось загрузить раздел",
                    err.clone(),
                    retry,
                    pal,
                ))
                .into_any_element();
        }
        _ if data.sections.is_empty() => {
            return page
                .child(render_empty_state(
                    Icon::Album,
                    "Раздел пока пуст",
                    "Здесь пока нет доступных треков и подборок",
                    None,
                    pal,
                ))
                .into_any_element();
        }
        _ => {}
    }

    if data.sections.iter().any(
        |section| matches!(&section.content, MusicContent::Tracks(tracks) if !tracks.is_empty()),
    ) {
        let from_lbl = title;
        let key = key.clone();
        page = page.child(div().mb(px(22.0)).child(
            primary_play_pill("sec-play-all", Icon::Play, "Слушать", pal).on_click(cx.listener(
                move |this, _, _, cx| {
                    let Some(data) = this.core.catalog_pages.get(&key) else {
                        return;
                    };
                    let all_tracks: Vec<Audio> = data
                        .sections
                        .iter()
                        .flat_map(|section| match &section.content {
                            MusicContent::Tracks(tracks) => tracks.as_slice(),
                            MusicContent::Playlists(_) => &[],
                        })
                        .cloned()
                        .collect();
                    this.play_tracks(
                        all_tracks,
                        0,
                        PlaybackOrigin::Other,
                        from_lbl.clone(),
                        None,
                        cx,
                    );
                },
            )),
        ));
    }

    for (idx, sec) in data.sections.iter().enumerate() {
        page = page.child(render_music_section_block(
            app,
            &format!("catsec-{idx}"),
            sec,
            center_w,
            pal,
            cx,
        ));
    }

    if matches!(target, SectionTarget::Id { .. })
        && matches!(data.state, LoadState::Loaded)
        && data.next_from.as_ref().is_some_and(|c| !c.is_empty())
    {
        let tgt = target.clone();
        if data.more_failed {
            page = page.child(div().mt(px(16.0)).flex().justify_center().child(
                pill_button("sec-load-retry", None, "Повторить", false, pal).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.load_more_section(tgt.clone(), cx);
                    },
                )),
            ));
        } else if data.loading_more {
            page = page.child(
                div()
                    .mt(px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .text_size(px(14.0))
                    .text_color(pal.ink_2)
                    .child(spinner("sec-more-spin", 15.0, pal.hi))
                    .child("Загрузка…"),
            );
        } else if app.canvas_near_bottom() {
            cx.spawn(async move |this, cx| {
                let _ = this.update(cx, |this, cx| this.load_more_section(tgt, cx));
            })
            .detach();
        }
    }

    page.into_any_element()
}
