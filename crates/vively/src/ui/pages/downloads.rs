use std::rc::Rc;

use gpui_kit::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, component::input::Input, div, px, relative,
};
use vively_core::{models::Audio, playback::PlaybackOrigin};

use crate::{
    design::{
        artwork::render_cover,
        colors::Palette,
        icons::{Icon, icon, spinner},
    },
    state::{
        app_state::VivelyApp,
        router::{ActiveModal, Route},
        runtime::{fmt_gb, fmt_tracks_count},
    },
    ui::components::{
        TrackTableOptions, cached_queue, icon_button, pill_button, primary_play_pill,
        render_empty_state, render_playlist_card, render_track_list,
    },
};

pub fn render_downloads_page(
    app: &VivelyApp,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let q = app.downloads_query.trim().to_lowercase();
    let filtered: Vec<Audio> = app
        .core.ready_downloads
        .iter()
        .filter(|t| {
            q.is_empty()
                || t.title.to_lowercase().contains(&q)
                || t.artist.to_lowercase().contains(&q)
        })
        .cloned()
        .collect();

    let completely_empty = app.core.ready_downloads.is_empty()
        && app.core.offline_collections.is_empty()
        && app.core.pending_downloads.is_empty();

    let subtitle = if completely_empty {
        "Музыка для прослушивания без интернета".to_owned()
    } else {
        format!(
            "{}, {} на диске",
            fmt_tracks_count(app.core.ready_downloads.len() as i64),
            fmt_gb(app.core.cache_usage.total)
        )
    };

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
                .child("Загруженное"),
        )
        .child(
            div()
                .mb(px(28.0))
                .text_size(px(15.0))
                .text_color(pal.ink_2)
                .child(subtitle),
        );

    if completely_empty {
        let actions = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_center()
            .gap(px(10.0))
            .child(
                primary_play_pill("dl-empty-bulk", Icon::Download, "Скачать медиатеку", pal)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.router.modal = Some(ActiveModal::BulkDownloadLibrary);
                        cx.notify();
                    })),
            )
            .child(
                pill_button("dl-empty-lib", Some(Icon::Heart), "Моя музыка", false, pal)
                    .h(px(40.0))
                    .px(px(16.0))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate_to(Route::Library, window, cx);
                    })),
            )
            .into_any_element();
        return page
            .child(render_empty_state(
                Icon::Download,
                "Нет загруженной музыки",
                "Скачивайте треки, плейлисты или всю медиатеку, чтобы слушать их офлайн",
                Some(actions),
                pal,
            ))
            .into_any_element();
    }

    if !app.core.ready_downloads.is_empty() {
        let play_offline = cached_queue(app, "dl-tr", &filtered);
        let shuf_offline = Rc::clone(&play_offline);
        page = page.child(
            div()
                .mb(px(22.0))
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    primary_play_pill("dl-play", Icon::Play, "Слушать", pal).on_click(cx.listener(
                        move |this, _, _, cx| {
                            if !play_offline.is_empty() {
                                this.play_tracks(
                                    play_offline.to_vec(),
                                    0,
                                    PlaybackOrigin::Other,
                                    "Загруженное",
                                    None,
                                    cx,
                                );
                            }
                        },
                    )),
                )
                .child(
                    pill_button("dl-shuf", Some(Icon::Shuffle), "Перемешать", false, pal)
                        .h(px(40.0))
                        .px(px(15.0))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !shuf_offline.is_empty() {
                                this.core.queue.set_shuffled(true);
                                this.play_tracks(
                                    shuf_offline.to_vec(),
                                    0,
                                    PlaybackOrigin::Other,
                                    "Загруженное",
                                    None,
                                    cx,
                                );
                            }
                        }),
                    ),
                ),
        );
    }

    if !app.core.pending_downloads.is_empty() {
        let mut pending_col = div().mb(px(28.0)).flex().flex_col().gap(px(6.0));
        for (idx, item) in app.core.pending_downloads.iter().enumerate() {
            let parsed = item.audio().ok();
            let cov = parsed.as_ref().and_then(|a| {
                app.resolve_artwork(a.thumbnail_url().or_else(|| a.artwork_url()), cx)
            });
            let fid = item.full_id.clone();
            let pct = app.core.download_progress.get(&fid).copied();
            let pct_label = pct
                .map(|p| format!("{:.0}%", p * 100.0))
                .unwrap_or_default();

            pending_col = pending_col.child(
                div()
                    .py(px(9.0))
                    .px(px(12.0))
                    .rounded(px(6.0))
                    .bg(pal.fill)
                    .flex()
                    .items_center()
                    .gap(px(10.0))
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
                                    .child(item.title.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(pal.ink_2)
                                    .truncate()
                                    .child(item.artist.clone()),
                            ),
                    )
                    .children((!pct_label.is_empty()).then(|| {
                        div()
                            .text_size(px(12.5))
                            .text_color(pal.ink_2)
                            .child(pct_label)
                    }))
                    .child(
                        icon_button(format!("cancel-dl-{idx}"), Icon::Close, 16.0, false, pal)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.remove_downloaded_track(&fid, cx);
                            })),
                    ),
            );
        }

        let pending_len = app.core.pending_downloads.len();
        let active_pct = app
            .core.pending_downloads
            .iter()
            .filter_map(|item| app.core.download_progress.get(&item.full_id).copied())
            .next();
        let mut queue_hdr_left = div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .child(spinner("dl-queue-spin", 16.0, pal.hi))
            .child(
                div()
                    .text_size(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(pal.ink)
                    .child(format!(
                        "В очереди загрузки: {}",
                        fmt_tracks_count(pending_len as i64)
                    )),
            );
        if let Some(pct) = active_pct {
            queue_hdr_left = queue_hdr_left.child(
                div()
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded(px(999.0))
                    .bg(pal.fill)
                    .text_size(px(12.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(pal.ink_2)
                    .child(format!("{:.0}%", pct * 100.0)),
            );
        }

        page = page
            .child(
                div()
                    .mb(px(14.0))
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(queue_hdr_left)
                    .child(
                        pill_button("dl-cancel-all", Some(Icon::Close), "Отменить всё", false, pal)
                            .h(px(32.0))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.cancel_all_pending_downloads(cx);
                            })),
                    ),
            )
            .child(pending_col);
    }

    let dl_playlists: Vec<_> = app
        .core.offline_collections
        .iter()
        .map(|c| c.playlist.clone())
        .collect();
    if !dl_playlists.is_empty() {
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
                        dl_playlists
                            .iter()
                            .map(|pl| render_playlist_card(app, "dl-col", pl, true, pal, cx)),
                    ),
            );
    }

    if !app.core.ready_downloads.is_empty() {
        page = page.child(
            div()
                .mb(px(22.0))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.0))
                .child(
                    div()
                        .text_size(px(20.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(pal.ink)
                        .child("Треки"),
                )
                .child({
                    let mut filter = div()
                        .w(px(220.0))
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
                                Input::new(&app.dl_filter_input)
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false),
                            ),
                        );
                    if !app.downloads_query.is_empty() {
                        let h = pal.fill_2;
                        filter = filter.child(
                            div()
                                .id("dl-clear-filter")
                                .size(px(20.0))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(move |s| s.bg(h))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.clear_downloads_filter(window, cx);
                                }))
                                .child(icon(Icon::Close, 12.0, pal.ink_2)),
                        );
                    }
                    filter
                }),
        );

        if filtered.is_empty() && !q.is_empty() {
            let reset = pill_button("dl-reset-filter", None, "Сбросить фильтр", false, pal)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.clear_downloads_filter(window, cx);
                }))
                .into_any_element();
            page = page.child(render_empty_state(
                Icon::Search,
                format!("По фильтру «{}» ничего не найдено", app.downloads_query.trim()),
                "Попробуйте другой запрос или сбросьте фильтр",
                Some(reset),
                pal,
            ));
        } else {
            page = page.child(render_track_list(
                app,
                &filtered,
                TrackTableOptions {
                    id_prefix: "dl-tr".into(),
                    is_album: false,
                    album_artist: None,
                    hide_album: false,
                    no_header: false,
                    from_label: "Загруженное".into(),
                    origin: PlaybackOrigin::Other,
                    editable_playlist: None,
                },
                pal,
                cx,
            ));
        }
    }

    page.into_any_element()
}
