use std::{collections::HashSet, path::PathBuf};

use gpui_kit::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ObjectFit, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, StyledImage as _,
    base::{ScrollBounce, Scrollbar, ScrollbarAxis},
    canvas,
    component::input::{Input, Textarea},
    div, img, px, relative, transparent_black, uniform_list,
};
use vively_core::{
    library::StreamMixSession,
    models::{Audio, Playlist},
    settings::{Appearance, CACHE_LIMIT_OPTIONS, UNLIMITED_CACHE},
};

use crate::{
    design::{
        artwork::{make_initials, render_avatar, render_cover},
        colors::Palette,
        icons::{Icon, icon, spinner},
    },
    state::{
        app_state::VivelyApp,
        models::{LoadState, UploadItemStatus},
        router::{ActiveModal, EditorTrackSource, ExportState, SettingsTab},
        runtime::{fmt_duration, fmt_gb, fmt_total_duration, fmt_tracks_count},
    },
    ui::{
        components::{icon_button, mood_chip, pill_button, primary_play_pill, toggle_pill},
        overlays::auth::render_auth_modal_content,
    },
};

pub fn render_modal_layer(
    app: &VivelyApp,
    viewport_w: f32,
    viewport_h: f32,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let Some(modal) = app.router.modal.clone() else {
        return div().into_any_element();
    };

    let height = (viewport_h - 48.0).max(320.0);
    let is_creator = matches!(modal, ActiveModal::PlaylistEditor { editing: None });
    let is_add_tracks = matches!(modal, ActiveModal::AddTracksToPlaylist { .. });
    let is_add_to_pl = matches!(modal, ActiveModal::AddToPlaylist { .. });
    let is_confirm_del = matches!(modal, ActiveModal::ConfirmDeletePlaylist { .. });
    let wide_creator = is_creator && viewport_w >= 860.0;
    let has_fixed_footer = matches!(
        modal,
        ActiveModal::PlaylistEditor { .. }
            | ActiveModal::AddTracksToPlaylist { .. }
            | ActiveModal::AddToPlaylist { .. }
            | ActiveModal::ConfirmDeletePlaylist { .. }
            | ActiveModal::AudioUpload
    );
    let width = if wide_creator {
        (viewport_w - 48.0).min(940.0)
    } else if is_add_tracks {
        (viewport_w - 48.0).min(640.0)
    } else if is_creator {
        (viewport_w - 48.0).min(560.0)
    } else if is_confirm_del {
        460.0
    } else {
        520.0
    };
    let fixed_height = if is_creator {
        Some(height.min(700.0))
    } else if is_add_tracks {
        Some(height.min(680.0))
    } else if is_add_to_pl {
        Some(height.min(560.0))
    } else {
        None
    };

    let dialog_inner = match modal {
        ActiveModal::MixTune { mix_id } => render_mix_tune_modal(app, &mix_id, pal, cx),
        ActiveModal::BulkDownloadLibrary => render_bulk_dl_modal(app, pal, cx),
        ActiveModal::PlaylistEditor { editing } => match editing {
            Some(playlist) => render_playlist_edit_modal(app, &playlist, height, pal, cx),
            None => render_playlist_creator_modal(app, wide_creator, pal, cx),
        },
        ActiveModal::AddTracksToPlaylist { playlist } => {
            render_add_tracks_to_playlist_modal(app, &playlist, pal, cx)
        }
        ActiveModal::ConfirmDeletePlaylist { playlist } => {
            render_confirm_delete_playlist_modal(app, &playlist, pal, cx)
        }
        ActiveModal::AddToPlaylist { track } => render_add_to_playlist_modal(app, &track, pal, cx),
        ActiveModal::AudioUpload => render_audio_upload_modal(app, height, pal, cx),
        ActiveModal::ShareFile { track, state } => {
            render_share_file_modal(app, &track, &state, pal, cx)
        }
        ActiveModal::Settings { tab } => render_settings_modal(app, tab, pal, cx),
        ActiveModal::Auth => render_auth_modal_content(app, pal, cx),
    };

    let mut panel = div()
        .id("modal-dialog-scroll")
        .w_full()
        .max_w(px(width))
        .max_h(px(height))
        .rounded(px(12.0))
        .bg(pal.raised)
        .border_1()
        .border_color(pal.line)
        .shadow(pal.shadow_pop())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
    if let Some(fixed_h) = fixed_height {
        panel = panel.h(px(fixed_h)).flex().flex_col().overflow_hidden();
    } else if has_fixed_footer {
        panel = panel.overflow_hidden();
    } else {
        panel = panel.overflow_y_scroll();
    }

    div()
        .id("modal-backdrop")
        .absolute()
        .inset_0()
        .occlude()
        .bg(pal.scrim)
        .flex()
        .items_center()
        .justify_center()
        .p(px(24.0))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| this.dismiss_modal(cx)),
        )
        .child(panel.child(dialog_inner))
        .into_any_element()
}

fn modal_header(
    title: impl Into<String>,
    right_extra: Option<AnyElement>,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let mut right = div().flex().items_center().gap(px(6.0));
    if let Some(ex) = right_extra {
        right = right.child(ex);
    }
    right = right.child(
        icon_button("modal-close-x", Icon::Close, 18.0, false, pal)
            .on_click(cx.listener(|this, _, _, cx| this.dismiss_modal(cx))),
    );

    div()
        .h(px(62.0))
        .flex_shrink_0()
        .pl(px(24.0))
        .pr(px(20.0))
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_size(px(19.0))
                .font_weight(FontWeight::BOLD)
                .line_height(relative(1.2))
                .text_color(pal.ink)
                .child(title.into()),
        )
        .child(right)
        .into_any_element()
}

fn render_mix_tune_modal(
    app: &VivelyApp,
    mix_id: &str,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let mut body = div()
        .px(px(24.0))
        .py(px(18.0))
        .flex()
        .flex_col()
        .gap(px(16.0));
    for cat in &app.core.mix_settings.mix_categories {
        let cid = cat.id.clone();
        let selected = app.core.mix_picks.get(&cid).cloned().unwrap_or_default();
        let mut chips = div().mt(px(2.0)).flex().flex_wrap().gap(px(8.0));
        for opt in &cat.options {
            let sel = selected.contains(&opt.id);
            let c = cid.clone();
            let o = opt.id.clone();
            chips = chips.child(
                mood_chip(format!("mtune-{c}-{o}"), opt.title.clone(), sel, pal).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.toggle_mix_option(&c, &o, cx);
                    }),
                ),
            );
        }
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(pal.ink_2)
                        .child(cat.title.clone()),
                )
                .child(chips),
        );
    }

    let apply_id = mix_id.to_owned();
    let foot = div()
        .pt(px(12.0))
        .px(px(24.0))
        .pb(px(20.0))
        .flex()
        .justify_end()
        .gap(px(8.0))
        .child(
            pill_button("mix-reset-btn", None, "Сбросить", false, pal)
                .h(px(40.0))
                .px(px(16.0))
                .on_click(cx.listener(|this, _, _, cx| this.clear_mix_picks(cx))),
        )
        .child(
            primary_play_pill("mix-apply-btn", Icon::Play, "Запустить свой микс", pal).on_click(
                cx.listener(move |this, _, _, cx| {
                    this.router.modal = None;
                    let tuned =
                        StreamMixSession::tuned(apply_id.clone(), this.core.mix_picks.clone()).ok();
                    this.start_vk_mix(
                        apply_id.clone(),
                        None,
                        tuned.and_then(|s| s.options_json),
                        "VK Микс".into(),
                        cx,
                    );
                }),
            ),
        );

    div()
        .flex()
        .flex_col()
        .child(modal_header("Настроить микс", None, pal, cx))
        .child(body)
        .child(foot)
        .into_any_element()
}

fn render_bulk_dl_modal(app: &VivelyApp, pal: &Palette, cx: &mut Context<VivelyApp>) -> AnyElement {
    let total = app.core.library_total.max(app.core.library_tracks.len() as i64);
    let options: [(&str, String, Option<usize>); 4] = [
        (
            "Первые 50",
            "50 последних добавленных треков".into(),
            Some(50),
        ),
        (
            "Первые 100",
            "100 последних добавленных треков".into(),
            Some(100),
        ),
        ("Первые 500", "500 треков из Моей музыки".into(), Some(500)),
        (
            "Всю медиатеку",
            format!("Все {} будут закреплены офлайн", fmt_tracks_count(total)),
            None,
        ),
    ];

    let mut body = div()
        .px(px(24.0))
        .py(px(18.0))
        .flex()
        .flex_col()
        .gap(px(12.0));
    for (idx, (label, sub, limit)) in options.into_iter().enumerate() {
        body = body.child(
            div()
                .py(px(12.0))
                .px(px(14.0))
                .rounded(px(6.0))
                .bg(pal.fill)
                .flex()
                .items_center()
                .justify_between()
                .gap(px(16.0))
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
                                .child(label),
                        )
                        .child(
                            div()
                                .mt(px(2.0))
                                .text_size(px(13.0))
                                .text_color(pal.ink_2)
                                .child(sub),
                        ),
                )
                .child(
                    pill_button(format!("bulkdl-{idx}"), None, "Скачать", false, pal).on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.bulk_download_library(limit, cx);
                        }),
                    ),
                ),
        );
    }

    div()
        .flex()
        .flex_col()
        .pb(px(6.0))
        .child(modal_header("Скачать медиатеку", None, pal, cx))
        .child(body)
        .into_any_element()
}

fn render_playlist_cover_picker(
    app: &VivelyApp,
    editing: Option<&Playlist>,
    size_px: f32,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let custom_cover = app.core.editor_cover_preview.clone();
    let existing_cover = editing.and_then(|pl| app.resolve_artwork(pl.artwork_url(), cx));
    let mut collage: Vec<PathBuf> = Vec::with_capacity(4);
    if custom_cover.is_none() && existing_cover.is_none() {
        for tr in &app.core.editor_draft_tracks {
            if let Some(path) =
                app.resolve_artwork(tr.thumbnail_url().or_else(|| tr.artwork_url()), cx)
                && !collage.contains(&path)
            {
                collage.push(path);
                if collage.len() == 4 {
                    break;
                }
            }
        }
    }

    let has_artwork = custom_cover.is_some() || existing_cover.is_some() || !collage.is_empty();
    let art: AnyElement = if let Some(path) = custom_cover.clone().or(existing_cover) {
        render_cover(Some(path), size_px, false, pal)
            .rounded(px(6.0))
            .into_any_element()
    } else if collage.len() == 4 {
        let half = size_px * 0.5;
        div()
            .size(px(size_px))
            .rounded(px(6.0))
            .overflow_hidden()
            .flex_shrink_0()
            .border_1()
            .border_color(pal.art_ring)
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .h(px(half))
                    .child(img(collage[0].clone()).size(px(half)).object_fit(ObjectFit::Cover))
                    .child(img(collage[1].clone()).size(px(half)).object_fit(ObjectFit::Cover)),
            )
            .child(
                div()
                    .flex()
                    .h(px(half))
                    .child(img(collage[2].clone()).size(px(half)).object_fit(ObjectFit::Cover))
                    .child(img(collage[3].clone()).size(px(half)).object_fit(ObjectFit::Cover)),
            )
            .into_any_element()
    } else {
        render_cover(collage.into_iter().next(), size_px, false, pal)
            .rounded(px(6.0))
            .into_any_element()
    };

    let mut box_el = div()
        .id("ed-pick-cover")
        .relative()
        .size(px(size_px))
        .flex_shrink_0()
        .rounded(px(6.0))
        .shadow(pal.shadow_art())
        .group("ed-cov")
        .child(art)
        .child(
            div()
                .absolute()
                .inset_0()
                .rounded(px(6.0))
                .bg(gpui_kit::rgba(0x00000090))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(4.0))
                .opacity(0.0)
                .group_hover("ed-cov", |s| s.opacity(1.0))
                .child(icon(Icon::Edit, 18.0, gpui_kit::rgb(0xFFFFFF).into()))
                .child(
                    div()
                        .text_size(px(11.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(gpui_kit::rgb(0xFFFFFF))
                        .child(if custom_cover.is_some() {
                            "Заменить"
                        } else {
                            "Обложка"
                        }),
                ),
        );

    if !has_artwork {
        box_el = box_el.child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(px(6.0))
                .flex()
                .justify_center()
                .group_hover("ed-cov", |s| s.opacity(0.0))
                .child(
                    div()
                        .px(px(7.0))
                        .py(px(2.0))
                        .rounded(px(4.0))
                        .bg(gpui_kit::rgba(0x00000066))
                        .text_size(px(10.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(gpui_kit::rgb(0xF2F1EF))
                        .child("Обложка"),
                ),
        );
    }

    if !app.core.editor_saving {
        box_el = box_el
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| this.pick_playlist_cover(cx)));
    }

    if custom_cover.is_some() && !app.core.editor_saving {
        box_el = box_el.child(
            div()
                .id("ed-clear-cover")
                .absolute()
                .top(px(5.0))
                .right(px(5.0))
                .size(px(22.0))
                .rounded_full()
                .bg(gpui_kit::rgba(0x000000B8))
                .hover(|s| s.bg(gpui_kit::rgba(0x000000E6)))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|this, _, _, cx| this.clear_editor_cover(cx)))
                .child(icon(Icon::Close, 12.0, gpui_kit::rgb(0xFFFFFF).into())),
        );
    }

    box_el.into_any_element()
}

fn render_playlist_submit_button(
    label: String,
    saving: bool,
    disabled: bool,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let hover_bg = pal.cta_hover;
    let lead: AnyElement = if saving {
        spinner("ed-save-spin", 16.0, pal.cta_fg)
    } else {
        icon(Icon::Check, 17.0, pal.cta_fg).into_any_element()
    };
    let btn = div()
        .id("ed-save")
        .h(px(40.0))
        .pl(px(14.0))
        .pr(px(18.0))
        .rounded(px(6.0))
        .bg(pal.cta_bg)
        .text_color(pal.cta_fg)
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(lead)
        .child(
            div()
                .text_size(px(14.5))
                .font_weight(FontWeight::BOLD)
                .child(label),
        );
    if saving || disabled {
        btn.opacity(0.5).cursor_default().into_any_element()
    } else {
        btn.cursor_pointer()
            .hover(move |s| s.bg(hover_bg))
            .on_click(cx.listener(|this, _, window, cx| this.save_playlist_editor(window, cx)))
            .into_any_element()
    }
}

fn render_playlist_edit_modal(
    app: &VivelyApp,
    editing: &Playlist,
    max_height: f32,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let top_grid = div()
        .flex()
        .items_start()
        .gap(px(18.0))
        .child(render_playlist_cover_picker(app, Some(editing), 104.0, pal, cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(pal.ink_2)
                                .child("Название"),
                        )
                        .child(
                            div()
                                .h(px(38.0))
                                .px(px(12.0))
                                .rounded(px(6.0))
                                .bg(pal.fill)
                                .flex()
                                .items_center()
                                .child(
                                    Input::new(&app.pl_title_input)
                                        .disabled(app.core.editor_saving)
                                        .appearance(false)
                                        .bordered(false)
                                        .focus_bordered(false),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(pal.ink_2)
                                .child("Описание"),
                        )
                        .child(
                            div()
                                .min_h(px(76.0))
                                .py(px(8.0))
                                .px(px(12.0))
                                .rounded(px(6.0))
                                .bg(pal.fill)
                                .child(
                                    Textarea::new(&app.pl_desc_input)
                                        .h(px(64.0))
                                        .disabled(app.core.editor_saving)
                                        .appearance(false)
                                        .bordered(false),
                                ),
                        ),
                ),
        );

    let priv_row = div()
        .id("pl-priv-row")
        .py(px(12.0))
        .px(px(14.0))
        .rounded(px(6.0))
        .bg(pal.fill)
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .cursor_pointer()
        .hover({
            let h = pal.fill_2;
            move |s| s.bg(h)
        })
        .on_click(cx.listener(|this, _, _, cx| {
            if !this.core.editor_saving {
                this.core.editor_hidden = !this.core.editor_hidden;
                cx.notify();
            }
        }))
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
                        .child("Скрытый плейлист"),
                )
                .child(
                    div()
                        .mt(px(2.0))
                        .text_size(px(13.0))
                        .text_color(pal.ink_2)
                        .child("Не отображается в поиске и рекомендациях ВКонтакте"),
                ),
        )
        .child(toggle_pill("pl-priv-toggle", app.core.editor_hidden, pal));

    let body = div()
        .px(px(24.0))
        .py(px(18.0))
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(top_grid)
        .child(priv_row);

    let title_empty = app.pl_title_input.read(cx).value().trim().is_empty();
    let submit = render_playlist_submit_button(
        if app.core.editor_saving {
            "Сохранение…".into()
        } else {
            "Сохранить".into()
        },
        app.core.editor_saving,
        title_empty,
        pal,
        cx,
    );
    let cancel = modal_cancel("ed-cancel", app.core.editor_saving, pal, cx);

    let mut footer = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .flex_shrink_0()
        .border_t_1()
        .border_color(pal.line)
        .px(px(24.0))
        .py(px(12.0));

    let left_status: AnyElement = if let Some(error) = &app.core.editor_error {
        div()
            .id("ed-save-error")
            .flex_1()
            .min_w_0()
            .text_size(px(13.0))
            .text_color(pal.danger)
            .truncate()
            .child(error.clone())
            .into_any_element()
    } else {
        div().flex_1().min_w_0().into_any_element()
    };

    footer = footer
        .child(left_status)
        .child(div().flex().items_center().gap(px(10.0)).child(cancel).child(submit));

    div()
        .flex()
        .flex_col()
        .child(modal_header("Изменить плейлист", None, pal, cx))
        .child(modal_body("ed-body", max_height, body))
        .child(footer)
        .into_any_element()
}

fn render_playlist_creator_modal(
    app: &VivelyApp,
    wide: bool,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let selected_count = app.core.editor_draft_tracks.len();
    let total_secs: i64 = app
        .core.editor_draft_tracks
        .iter()
        .map(|t| t.duration.max(0))
        .sum();
    let dur_text = fmt_total_duration(total_secs);
    let summary_text = if selected_count == 0 {
        String::new()
    } else if dur_text.is_empty() {
        fmt_tracks_count(selected_count as i64)
    } else {
        format!("{} · {dur_text}", fmt_tracks_count(selected_count as i64))
    };

    let mut header_left = div().flex().items_center().gap(px(10.0)).child(
        div()
            .text_size(px(19.0))
            .font_weight(FontWeight::BOLD)
            .line_height(relative(1.2))
            .text_color(pal.ink)
            .child("Новый плейлист"),
    );
    if !summary_text.is_empty() {
        header_left = header_left.child(
            div()
                .px(px(9.0))
                .py(px(3.0))
                .rounded(px(999.0))
                .bg(pal.fill)
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink_2)
                .child(summary_text.clone()),
        );
    }

    let mut header_right = div().flex().items_center().gap(px(6.0));
    if !wide {
        let on_picker = app.editor_picker_open;
        header_right = header_right.child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .p(px(3.0))
                .rounded(px(7.0))
                .bg(pal.fill)
                .child(
                    div()
                        .id("ed-compact-details")
                        .h(px(28.0))
                        .px(px(10.0))
                        .rounded(px(5.0))
                        .flex()
                        .items_center()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .bg(if !on_picker {
                            pal.ink
                        } else {
                            transparent_black()
                        })
                        .text_color(if !on_picker { pal.ground } else { pal.ink_2 })
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.editor_picker_open = false;
                            cx.notify();
                        }))
                        .child("Детали"),
                )
                .child(
                    div()
                        .id("ed-compact-tracks")
                        .h(px(28.0))
                        .px(px(10.0))
                        .rounded(px(5.0))
                        .flex()
                        .items_center()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .bg(if on_picker {
                            pal.ink
                        } else {
                            transparent_black()
                        })
                        .text_color(if on_picker { pal.ground } else { pal.ink_2 })
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.editor_picker_open = true;
                            cx.notify();
                        }))
                        .child(if selected_count > 0 {
                            format!("Треки · {selected_count}")
                        } else {
                            "Треки".into()
                        }),
                ),
        );
    }
    header_right = header_right.child(
        icon_button("modal-close-x", Icon::Close, 18.0, false, pal)
            .on_click(cx.listener(|this, _, _, cx| this.dismiss_modal(cx))),
    );

    let header = div()
        .h(px(58.0))
        .flex_shrink_0()
        .pl(px(22.0))
        .pr(px(18.0))
        .border_b_1()
        .border_color(pal.line)
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .child(header_left)
        .child(header_right);

    let content: AnyElement = if wide {
        div()
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .overflow_hidden()
            .child(render_creator_left_pane(app, true, pal, cx))
            .child(render_creator_picker_pane(app, pal, cx))
            .into_any_element()
    } else if app.editor_picker_open {
        div()
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .overflow_hidden()
            .child(render_creator_picker_pane(app, pal, cx))
            .into_any_element()
    } else {
        div()
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .overflow_hidden()
            .child(render_creator_left_pane(app, false, pal, cx))
            .into_any_element()
    };

    let title_empty = app.pl_title_input.read(cx).value().trim().is_empty();
    let submit_label = if app.core.editor_saving {
        "Создание…".into()
    } else if selected_count > 0 {
        format!("Создать · {selected_count}")
    } else {
        "Создать".into()
    };
    let submit = render_playlist_submit_button(submit_label, app.core.editor_saving, title_empty, pal, cx);
    let cancel = modal_cancel("ed-cancel", app.core.editor_saving, pal, cx);

    let footer_hint: AnyElement = if let Some(error) = &app.core.editor_error {
        div()
            .id("ed-save-error")
            .flex_1()
            .min_w_0()
            .text_size(px(13.0))
            .text_color(pal.danger)
            .truncate()
            .child(error.clone())
            .into_any_element()
    } else if title_empty {
        div()
            .flex_1()
            .min_w_0()
            .text_size(px(13.0))
            .text_color(pal.ink_3)
            .truncate()
            .child("Введите название, чтобы создать плейлист")
            .into_any_element()
    } else if selected_count == 0 {
        div()
            .flex_1()
            .min_w_0()
            .text_size(px(13.0))
            .text_color(pal.ink_2)
            .truncate()
            .child("Можно создать пустой плейлист и добавить треки позже")
            .into_any_element()
    } else {
        div()
            .flex_1()
            .min_w_0()
            .text_size(px(13.0))
            .text_color(pal.ink_2)
            .truncate()
            .child(format!("Выбрано: {summary_text}"))
            .into_any_element()
    };

    let footer = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .flex_shrink_0()
        .border_t_1()
        .border_color(pal.line)
        .px(px(22.0))
        .py(px(12.0))
        .child(footer_hint)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .flex_shrink_0()
                .child(cancel)
                .child(submit),
        );

    div()
        .flex_1()
        .min_h_0()
        .w_full()
        .flex()
        .flex_col()
        .child(header)
        .child(content)
        .child(footer)
        .into_any_element()
}

fn render_creator_left_pane(
    app: &VivelyApp,
    side_by_side: bool,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let cover_and_meta = div()
        .flex()
        .items_start()
        .gap(px(12.0))
        .child(render_playlist_cover_picker(app, None, 88.0, pal, cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .h(px(38.0))
                        .px(px(11.0))
                        .rounded(px(6.0))
                        .bg(pal.fill)
                        .flex()
                        .items_center()
                        .child(
                            Input::new(&app.pl_title_input)
                                .disabled(app.core.editor_saving)
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false),
                        ),
                )
                .child(
                    div()
                        .id("pl-priv-row")
                        .h(px(42.0))
                        .px(px(10.0))
                        .rounded(px(6.0))
                        .bg(pal.fill)
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.0))
                        .cursor_pointer()
                        .hover({
                            let h = pal.fill_2;
                            move |s| s.bg(h)
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            if !this.core.editor_saving {
                                this.core.editor_hidden = !this.core.editor_hidden;
                                cx.notify();
                            }
                        }))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(7.0))
                                .child(icon(Icon::Lock, 14.0, pal.ink_2))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .text_size(px(12.5))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(pal.ink)
                                                .truncate()
                                                .child("Скрытый"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.5))
                                                .text_color(pal.ink_3)
                                                .truncate()
                                                .child("Скрыт из поиска VK"),
                                        ),
                                ),
                        )
                        .child(toggle_pill("pl-priv-toggle", app.core.editor_hidden, pal)),
                ),
        );

    let desc_box = div()
        .min_h(px(56.0))
        .py(px(6.0))
        .px(px(11.0))
        .rounded(px(6.0))
        .bg(pal.fill)
        .child(
            Textarea::new(&app.pl_desc_input)
                .h(px(44.0))
                .disabled(app.core.editor_saving)
                .appearance(false)
                .bordered(false),
        );

    let identity = div()
        .px(px(18.0))
        .pt(px(16.0))
        .pb(px(14.0))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(cover_and_meta)
        .child(desc_box);

    let draft_count = app.core.editor_draft_tracks.len();
    let mut draft_hdr_left = div().flex().items_center().gap(px(7.0)).child(
        div()
            .text_size(px(13.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(pal.ink_2)
            .child("В плейлисте"),
    );
    if draft_count > 0 {
        draft_hdr_left = draft_hdr_left.child(
            div()
                .px(px(7.0))
                .py(px(1.0))
                .rounded(px(999.0))
                .bg(pal.fill)
                .text_size(px(11.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink)
                .child(draft_count.to_string()),
        );
    }

    let mut draft_hdr = div()
        .px(px(18.0))
        .pt(px(12.0))
        .pb(px(8.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .child(draft_hdr_left);

    if draft_count > 0 && !app.core.editor_saving {
        let danger = pal.danger;
        draft_hdr = draft_hdr.child(
            div()
                .id("ed-draft-clear")
                .px(px(6.0))
                .py(px(2.0))
                .rounded(px(4.0))
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink_3)
                .cursor_pointer()
                .hover(move |s| s.text_color(danger))
                .on_click(cx.listener(|this, _, _, cx| this.clear_editor_draft_tracks(cx)))
                .child("Очистить"),
        );
    } else if !side_by_side {
        draft_hdr = draft_hdr.child(
            pill_button(
                "ed-open-picker",
                Some(Icon::Plus),
                "Добавить треки",
                false,
                pal,
            )
            .h(px(30.0))
            .px(px(10.0))
            .on_click(cx.listener(|this, _, _, cx| {
                this.editor_picker_open = true;
                cx.notify();
            })),
        );
    }

    let draft_body: AnyElement = if draft_count == 0 {
        let mut empty = div()
            .flex_1()
            .min_h_0()
            .px(px(24.0))
            .pb(px(20.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .child(icon(Icon::Album, 26.0, pal.ink_3))
            .child(
                div()
                    .mt(px(4.0))
                    .text_size(px(13.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(pal.ink_2)
                    .child("Треки пока не выбраны"),
            )
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(pal.ink_3)
                    .text_center()
                    .child(if side_by_side {
                        "Нажимайте на треки справа, чтобы собрать плейлист"
                    } else {
                        "Перейдите на вкладку «Треки», чтобы выбрать музыку"
                    }),
            );
        if !side_by_side {
            empty = empty.child(
                div().mt(px(8.0)).child(
                    pill_button(
                        "ed-empty-open-picker",
                        Some(Icon::Plus),
                        "Выбрать треки",
                        false,
                        pal,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.editor_picker_open = true;
                        cx.notify();
                    })),
                ),
            );
        }
        empty.into_any_element()
    } else {
        let rows: Vec<AnyElement> = app
            .core.editor_draft_tracks
            .iter()
            .enumerate()
            .map(|(idx, track)| {
                render_draft_track_row(track, idx, draft_count, app, pal, cx)
            })
            .collect();

        let scroll_list = div()
            .id("ed-draft-scroll")
            .track_scroll(&app.editor_draft_scroll)
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .child(
                div()
                    .px(px(10.0))
                    .pb(px(12.0))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .children(rows),
            );

        div()
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(ScrollBounce::new(
                "ed-draft-bounce",
                &app.editor_draft_scroll,
                scroll_list,
            ))
            .child(
                div().absolute().inset_0().child(
                    Scrollbar::new(&app.editor_draft_scroll)
                        .id("ed-draft-scrollbar")
                        .axis(ScrollbarAxis::Vertical)
                        .viewport_from_layout(),
                ),
            )
            .into_any_element()
    };

    let pane = div()
        .h_full()
        .min_h_0()
        .flex()
        .flex_col()
        .child(identity)
        .child(div().h(px(1.0)).bg(pal.line).flex_shrink_0())
        .child(draft_hdr)
        .child(draft_body);

    if side_by_side {
        pane.w(px(340.0))
            .flex_shrink_0()
            .border_r_1()
            .border_color(pal.line)
            .into_any_element()
    } else {
        pane.flex_1().min_w_0().into_any_element()
    }
}

fn render_draft_track_row(
    track: &Audio,
    idx: usize,
    total: usize,
    app: &VivelyApp,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let id = track.full_id();
    let grp = SharedString::from(format!("ed-draft-grp-{idx}-{id}"));
    let cover = app.resolve_artwork(track.thumbnail_url().or_else(|| track.artwork_url()), cx);
    let track_rm = track.clone();
    let saving = app.core.editor_saving;
    let hover_bg = pal.hover;
    let btn_hover = pal.fill_2;

    let mut actions = div().flex().items_center().gap(px(2.0)).flex_shrink_0();
    if !saving {
        if total > 1 && idx > 0 {
            let g = grp.clone();
            actions = actions.child(
                div()
                    .id(format!("ed-draft-up-{idx}-{id}"))
                    .size(px(22.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .opacity(0.0)
                    .group_hover(g, |s| s.opacity(1.0))
                    .hover(move |s| s.bg(btn_hover))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.move_editor_draft_track(idx, idx - 1, cx);
                    }))
                    .child(icon(Icon::ChevUp, 13.0, pal.ink_2)),
            );
        }
        if total > 1 && idx + 1 < total {
            let g = grp.clone();
            actions = actions.child(
                div()
                    .id(format!("ed-draft-down-{idx}-{id}"))
                    .size(px(22.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .opacity(0.0)
                    .group_hover(g, |s| s.opacity(1.0))
                    .hover(move |s| s.bg(btn_hover))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.move_editor_draft_track(idx, idx + 1, cx);
                    }))
                    .child(icon(Icon::ChevDown, 13.0, pal.ink_2)),
            );
        }
        actions = actions.child(
            div()
                .id(format!("ed-draft-rm-{idx}-{id}"))
                .size(px(22.0))
                .rounded(px(4.0))
                .flex()
                .items_center()
                .justify_center()
                .hover(move |s| s.bg(btn_hover))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_editor_draft_track(track_rm.clone(), cx);
                }))
                .child(icon(Icon::Close, 13.0, pal.ink_3)),
        );
    }

    div()
        .id(format!("ed-draft-track-{idx}-{id}"))
        .group(grp)
        .h(px(46.0))
        .px(px(8.0))
        .rounded(px(6.0))
        .hover(move |s| s.bg(hover_bg))
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .w(px(18.0))
                .flex_shrink_0()
                .text_size(px(12.0))
                .text_color(pal.ink_3)
                .text_right()
                .child(format!("{}", idx + 1)),
        )
        .child(render_cover(cover, 30.0, false, pal))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(pal.ink)
                        .truncate()
                        .child(track.title.clone()),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(pal.ink_2)
                        .truncate()
                        .child(track.artist.clone()),
                ),
        )
        .child(actions)
        .into_any_element()
}

fn render_creator_picker_pane(
    app: &VivelyApp,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let source = app.editor_source;
    let drill = app.core.editor_picker_playlist.is_some();
    let list_collections = source == EditorTrackSource::Playlists && !drill;
    let query_raw = app.editor_picker_query.trim();
    let query_lc = query_raw.to_lowercase();
    let filtering_local = source != EditorTrackSource::Search && !query_lc.is_empty();

    let likes_label = if app.core.library_total > 0 {
        format!("Моя музыка · {}", app.core.library_total)
    } else {
        "Моя музыка".into()
    };

    let tabs = div().flex().items_center().gap(px(6.0)).children(
        [
            ("edsrc-likes", likes_label, EditorTrackSource::Likes),
            ("edsrc-pls", "Плейлисты".into(), EditorTrackSource::Playlists),
            ("edsrc-search", "Поиск VK".into(), EditorTrackSource::Search),
        ]
        .map(|(id, title, target)| {
            pill_button(id, None, title, source == target, pal)
                .h(px(33.0))
                .px(px(12.0))
                .on_click(cx.listener(move |this, _, window, cx| {
                    if this.editor_source != target || this.core.editor_picker_playlist.is_some() {
                        this.set_editor_source(target, window, cx);
                    }
                    if target == EditorTrackSource::Search {
                        this.pl_picker_search_input
                            .update(cx, |input, cx| input.focus(window, cx));
                    }
                }))
        }),
    );

    let mut search_bar = div()
        .h(px(36.0))
        .px(px(12.0))
        .rounded(px(6.0))
        .bg(pal.fill)
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(icon(Icon::Search, 15.0, pal.ink_2))
        .child(
            div().flex_1().min_w_0().child(
                Input::new(&app.pl_picker_search_input)
                    .appearance(false)
                    .bordered(false)
                    .focus_bordered(false),
            ),
        );
    if !app.editor_picker_query.is_empty() {
        search_bar = search_bar.child(
            div()
                .id("ed-picker-clear-q")
                .size(px(22.0))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover({
                    let h = pal.fill_2;
                    move |s| s.bg(h)
                })
                .on_click(cx.listener(|this, _, window, cx| {
                    this.clear_editor_picker_query(window, cx);
                }))
                .child(icon(Icon::Close, 13.0, pal.ink_2)),
        );
    }

    let mut top_controls = div()
        .px(px(18.0))
        .pt(px(14.0))
        .pb(px(10.0))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(tabs);

    let draft_ids: HashSet<(i64, i64)> = app
        .core.editor_draft_tracks
        .iter()
        .map(|t| (t.owner_id, t.id))
        .collect();

    if let Some(playlist) = &app.core.editor_picker_playlist {
        let cov = app.resolve_artwork(playlist.artwork_url(), cx);
        let pl_tracks = &app.core.editor_picker_tracks;
        let all_selected = !pl_tracks.is_empty()
            && pl_tracks
                .iter()
                .all(|t| draft_ids.contains(&(t.owner_id, t.id)));

        let mut crumb = div()
            .px(px(10.0))
            .py(px(7.0))
            .rounded(px(7.0))
            .bg(pal.fill)
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                icon_button("ed-picker-back", Icon::ChevLeft, 17.0, false, pal).on_click(
                    cx.listener(|this, _, window, cx| this.editor_picker_back(window, cx)),
                ),
            )
            .child(render_cover(cov, 28.0, false, pal))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(13.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(pal.ink)
                            .truncate()
                            .child(playlist.title.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(pal.ink_2)
                            .child(fmt_tracks_count(playlist.count.max(pl_tracks.len() as i64))),
                    ),
            );

        if !pl_tracks.is_empty() && !app.core.editor_saving {
            let bulk_tracks = pl_tracks.clone();
            crumb = crumb.child(
                pill_button(
                    "ed-picker-bulk-pl",
                    Some(if all_selected {
                        Icon::Close
                    } else {
                        Icon::Plus
                    }),
                    if all_selected {
                        "Убрать все"
                    } else {
                        "Добавить все"
                    },
                    false,
                    pal,
                )
                .h(px(30.0))
                .px(px(10.0))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if all_selected {
                        this.remove_editor_tracks_bulk(&bulk_tracks, cx);
                    } else {
                        this.add_editor_tracks_bulk(bulk_tracks.clone(), cx);
                    }
                })),
            );
        }
        top_controls = top_controls.child(crumb);
    }

    top_controls = top_controls.child(search_bar);

    let filtered_indices: Option<Vec<usize>> = if !filtering_local {
        None
    } else if list_collections {
        Some(
            app.core.editor_picker_playlists
                .iter()
                .enumerate()
                .filter(|(_, pl)| pl.title.to_lowercase().contains(&query_lc))
                .map(|(i, _)| i)
                .collect(),
        )
    } else {
        Some(
            editor_tracks(app)
                .iter()
                .enumerate()
                .filter(|(_, t)| {
                    t.title.to_lowercase().contains(&query_lc)
                        || t.artist.to_lowercase().contains(&query_lc)
                })
                .map(|(i, _)| i)
                .collect(),
        )
    };

    let raw_count = if list_collections {
        app.core.editor_picker_playlists.len()
    } else {
        editor_tracks(app).len()
    };
    let count = filtered_indices
        .as_ref()
        .map_or(raw_count, Vec::len);

    let state = match source {
        EditorTrackSource::Likes => &app.core.library_state,
        EditorTrackSource::Search => &app.core.editor_search_state,
        EditorTrackSource::Playlists => &app.core.editor_picker_state,
    };
    let retry_more = source == EditorTrackSource::Likes && app.core.library_more_failed;
    let can_page = !filtering_local
        && match source {
            EditorTrackSource::Likes => app.core.library_can_more && !app.core.library_loading_more,
            EditorTrackSource::Search => false,
            EditorTrackSource::Playlists => app.core.editor_picker_more,
        }
        && matches!(state, LoadState::Loaded)
        && !retry_more;

    let list_area: AnyElement = if count > 0 {
        let generation = app.core.editor_picker_generation;
        let handle = app.editor_picker_scroll.clone();
        let view = cx.weak_entity();
        let pal_copy = *pal;
        let list = uniform_list("ed-picker-rows", count, move |range, window, cx| {
            let Some(view) = view.upgrade() else {
                return Vec::new();
            };
            if can_page && range.end.saturating_add(10) >= count {
                let view = view.downgrade();
                window.defer(cx, move |_, cx| {
                    let _ = view.update(cx, |app, cx| {
                        if app.core.editor_picker_generation != generation {
                            return;
                        }
                        if source == EditorTrackSource::Likes {
                            if !app.core.library_more_failed {
                                app.load_more_library(cx);
                            }
                        } else if !matches!(app.core.editor_picker_state, LoadState::Failed(_)) {
                            app.load_editor_playlist_page(cx);
                        }
                    });
                });
            }
            view.update(cx, |app, cx| {
                let selected_set: HashSet<(i64, i64)> = app
                    .core.editor_draft_tracks
                    .iter()
                    .map(|t| (t.owner_id, t.id))
                    .collect();
                let existing_set: HashSet<(i64, i64)> = match &app.router.modal {
                    Some(ActiveModal::AddTracksToPlaylist { playlist }) => app
                        .core.playlist_details
                        .get(&playlist.stable_id())
                        .map(|d| d.tracks.iter().map(|t| (t.owner_id, t.id)).collect())
                        .unwrap_or_default(),
                    _ => HashSet::new(),
                };

                if list_collections {
                    return range
                        .filter_map(|row_idx| {
                            let real_idx = filtered_indices
                                .as_ref()
                                .map_or(row_idx, |v| v[row_idx]);
                            app.core.editor_picker_playlists.get(real_idx)
                        })
                        .map(|playlist| render_picker_playlist_row(playlist, app, &pal_copy, cx))
                        .collect();
                }

                range
                    .filter_map(|row_idx| {
                        let real_idx = filtered_indices
                            .as_ref()
                            .map_or(row_idx, |v| v[row_idx]);
                        editor_tracks(app).get(real_idx)
                    })
                    .map(|track| {
                        let key = (track.owner_id, track.id);
                        let selected = selected_set.contains(&key);
                        let already_in_playlist = existing_set.contains(&key);
                        render_picker_track_row(
                            track,
                            selected,
                            already_in_playlist,
                            app,
                            &pal_copy,
                            cx,
                        )
                    })
                    .collect()
            })
        })
        .track_scroll(&handle)
        .size_full();

        div()
            .relative()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(ScrollBounce::new("ed-picker-bounce", &handle, list))
            .child(
                div().absolute().inset_0().child(
                    Scrollbar::new(&handle)
                        .id("ed-picker-scrollbar")
                        .axis(ScrollbarAxis::Vertical)
                        .viewport_from_layout(),
                ),
            )
            .into_any_element()
    } else {
        render_picker_empty_state(app, source, list_collections, state, query_raw, pal, cx)
    };

    let bottom_strip = render_picker_bottom_strip(app, source, filtering_local, raw_count, pal, cx);

    let mut right_pane = div()
        .flex_1()
        .min_w_0()
        .h_full()
        .min_h_0()
        .flex()
        .flex_col()
        .child(top_controls)
        .child(list_area);
    if let Some(strip) = bottom_strip {
        right_pane = right_pane.child(strip);
    }
    right_pane.into_any_element()
}

fn render_picker_empty_state(
    app: &VivelyApp,
    source: EditorTrackSource,
    list_collections: bool,
    state: &LoadState,
    query_raw: &str,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let mut box_el = div()
        .flex_1()
        .min_h_0()
        .px(px(28.0))
        .pb(px(20.0))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(8.0));

    match state {
        LoadState::Loading => {
            box_el = box_el
                .child(spinner("ed-picker-loading", 24.0, pal.hi))
                .child(
                    div()
                        .mt(px(4.0))
                        .text_size(px(13.5))
                        .text_color(pal.ink_2)
                        .child("Загрузка…"),
                );
        }
        LoadState::Failed(err) => {
            box_el = box_el
                .child(
                    div()
                        .text_size(px(13.5))
                        .text_color(pal.danger)
                        .text_center()
                        .child(err.clone()),
                )
                .child(
                    pill_button("ed-picker-retry", None, "Повторить", false, pal).on_click(
                        cx.listener(move |this, _, _, cx| match source {
                            EditorTrackSource::Likes => this.load_library(true, cx),
                            EditorTrackSource::Playlists => this.load_editor_playlist_page(cx),
                            EditorTrackSource::Search => {
                                let q = this.editor_picker_query.clone();
                                this.search_editor_tracks(q, true, cx);
                            }
                        }),
                    ),
                );
        }
        LoadState::Idle if source == EditorTrackSource::Search => {
            box_el = box_el
                .child(icon(Icon::Search, 26.0, pal.ink_3))
                .child(
                    div()
                        .mt(px(2.0))
                        .text_size(px(14.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(pal.ink_2)
                        .child("Поиск по каталогу ВКонтакте"),
                )
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(pal.ink_3)
                        .text_center()
                        .child("Введите название трека или имя исполнителя"),
                );
        }
        _ => {
            let filtering = source != EditorTrackSource::Search && !query_raw.is_empty();
            let title = if filtering {
                "Совпадений не найдено"
            } else if list_collections {
                "Нет плейлистов"
            } else if source == EditorTrackSource::Search {
                "Ничего не найдено"
            } else {
                "Нет треков"
            };
            box_el = box_el.child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(pal.ink_2)
                    .child(title),
            );
            if filtering {
                let mut actions = div()
                    .mt(px(4.0))
                    .flex()
                    .flex_wrap()
                    .justify_center()
                    .gap(px(8.0));
                if source == EditorTrackSource::Likes && app.core.library_can_more {
                    actions = actions.child(
                        pill_button(
                            "ed-picker-load-more-empty",
                            None,
                            if app.core.library_loading_more {
                                "Загрузка…"
                            } else {
                                "Загрузить ещё треки"
                            },
                            false,
                            pal,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.load_more_library(cx))),
                    );
                }
                actions = actions.child(
                    pill_button(
                        "ed-picker-fallback-vk",
                        Some(Icon::Search),
                        format!("Искать «{query_raw}» в VK"),
                        false,
                        pal,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.search_editor_query_in_vk(window, cx);
                    })),
                );
                box_el = box_el.child(actions);
            }
        }
    }

    box_el.into_any_element()
}

fn render_picker_bottom_strip(
    app: &VivelyApp,
    source: EditorTrackSource,
    filtering_local: bool,
    raw_count: usize,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> Option<AnyElement> {
    if source == EditorTrackSource::Likes && app.core.library_loading_more && raw_count > 0 {
        return Some(
            div()
                .flex_shrink_0()
                .px(px(18.0))
                .py(px(8.0))
                .border_t_1()
                .border_color(pal.line)
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(spinner("ed-picker-more-spin", 14.0, pal.hi))
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(pal.ink_2)
                        .child("Загрузка ещё треков…"),
                )
                .into_any_element(),
        );
    }

    if source == EditorTrackSource::Likes && app.core.library_more_failed {
        return Some(
            div()
                .flex_shrink_0()
                .px(px(18.0))
                .py(px(8.0))
                .border_t_1()
                .border_color(pal.line)
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(pal.danger)
                        .child("Не удалось подгрузить треки"),
                )
                .child(
                    pill_button("ed-picker-more-retry", None, "Повторить", false, pal)
                        .h(px(28.0))
                        .px(px(10.0))
                        .on_click(cx.listener(|this, _, _, cx| this.load_more_library(cx))),
                )
                .into_any_element(),
        );
    }

    if source == EditorTrackSource::Likes
        && filtering_local
        && app.core.library_can_more
        && raw_count > 0
    {
        let total = app.core.library_total.max(raw_count as i64);
        return Some(
            div()
                .flex_shrink_0()
                .px(px(18.0))
                .py(px(8.0))
                .border_t_1()
                .border_color(pal.line)
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(pal.ink_2)
                        .truncate()
                        .child(format!("Поиск среди загруженных {raw_count} из {total}")),
                )
                .child(
                    pill_button("ed-picker-load-more-bar", None, "Загрузить ещё", false, pal)
                        .h(px(28.0))
                        .px(px(10.0))
                        .on_click(cx.listener(|this, _, _, cx| this.load_more_library(cx))),
                )
                .into_any_element(),
        );
    }

    None
}

fn render_picker_playlist_row(
    playlist: &Playlist,
    app: &VivelyApp,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let target = playlist.clone();
    let cover = app.resolve_artwork(playlist.artwork_url(), cx);
    let hover_bg = pal.hover;

    div()
        .h(px(56.0))
        .pb(px(2.0))
        .px(px(10.0))
        .child(
            div()
                .id(format!("ed-source-playlist-{}", playlist.full_id()))
                .h_full()
                .px(px(10.0))
                .rounded(px(7.0))
                .flex()
                .items_center()
                .gap(px(11.0))
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.pick_editor_playlist(target.clone(), window, cx);
                }))
                .child(render_cover(cover, 38.0, false, pal))
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
                                .text_color(pal.ink)
                                .truncate()
                                .child(playlist.title.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(pal.ink_2)
                                .truncate()
                                .child(fmt_tracks_count(playlist.count)),
                        ),
                )
                .child(icon(Icon::ChevRight, 16.0, pal.ink_3)),
        )
        .into_any_element()
}

fn render_picker_track_row(
    track: &Audio,
    selected: bool,
    already_in_playlist: bool,
    app: &VivelyApp,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let id = track.full_id();
    let grp = SharedString::from(format!("ed-pick-grp-{id}"));
    let cover = app.resolve_artwork(track.thumbnail_url().or_else(|| track.artwork_url()), cx);
    let track_action = track.clone();
    let row_bg = if selected {
        pal.hi.opacity(0.11)
    } else {
        transparent_black()
    };
    let row_border = if selected {
        pal.hi.opacity(0.28)
    } else {
        transparent_black()
    };
    let row_hover = if selected {
        pal.hi.opacity(0.17)
    } else {
        pal.hover
    };
    let badge_hover = pal.fill_2;

    let badge = if selected {
        div()
            .size(px(26.0))
            .rounded_full()
            .flex_shrink_0()
            .bg(pal.hi)
            .flex()
            .items_center()
            .justify_center()
            .child(icon(Icon::Check, 14.0, pal.cta_fg))
    } else {
        div()
            .size(px(26.0))
            .rounded_full()
            .flex_shrink_0()
            .bg(pal.fill)
            .group_hover(grp.clone(), move |s| s.bg(badge_hover))
            .flex()
            .items_center()
            .justify_center()
            .child(icon(Icon::Plus, 14.0, pal.ink_2))
    };

    let row = div()
        .id(format!("ed-picker-track-{id}"))
        .group(grp)
        .h_full()
        .px(px(10.0))
        .rounded(px(7.0))
        .bg(row_bg)
        .border_1()
        .border_color(row_border)
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(render_cover(cover, 36.0, false, pal))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(13.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if selected { pal.hi } else { pal.ink })
                        .truncate()
                        .child(track.title.clone()),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(pal.ink_2)
                        .truncate()
                        .child(track.artist.clone()),
                ),
        )
        .children(already_in_playlist.then(|| {
            div()
                .flex_shrink_0()
                .px(px(7.0))
                .py(px(2.0))
                .rounded(px(4.0))
                .bg(pal.fill)
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink_3)
                .child("В плейлисте")
        }))
        .child(
            div()
                .flex_shrink_0()
                .text_size(px(12.0))
                .text_color(pal.ink_3)
                .child(fmt_duration(track.duration)),
        )
        .child(badge);

    let row = if app.core.editor_saving {
        row
    } else {
        row.cursor_pointer()
            .hover(move |s| s.bg(row_hover))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_editor_draft_track(track_action.clone(), cx);
            }))
    };

    div()
        .h(px(52.0))
        .pb(px(2.0))
        .px(px(10.0))
        .child(row)
        .into_any_element()
}

fn modal_body(id: &'static str, max_height: f32, body: impl IntoElement) -> impl IntoElement {
    div()
        .id(id)
        .max_h(px((max_height - 132.0).max(0.0)))
        .overflow_y_scroll()
        .child(body)
}

fn modal_footer(pal: &Palette) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .justify_end()
        .gap(px(10.0))
        .flex_shrink_0()
        .border_t_1()
        .border_color(pal.line)
        .px(px(24.0))
        .py(px(12.0))
}

fn modal_cancel(
    id: &'static str,
    disabled: bool,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let button = pill_button(id, None, "Отмена", false, pal)
        .h(px(40.0))
        .min_w(px(96.0))
        .justify_center();
    if disabled {
        button.opacity(0.5).cursor_default()
    } else {
        button.on_click(cx.listener(|this, _, _, cx| this.dismiss_modal(cx)))
    }
}

fn editor_tracks(app: &VivelyApp) -> &[Audio] {
    match app.editor_source {
        EditorTrackSource::Likes => &app.core.library_tracks,
        EditorTrackSource::Search => &app.core.editor_search_tracks,
        EditorTrackSource::Playlists => &app.core.editor_picker_tracks,
    }
}
fn render_add_tracks_to_playlist_modal(
    app: &VivelyApp,
    playlist: &Playlist,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let selected_count = app.core.editor_draft_tracks.len();
    let pl_cov = app.resolve_artwork(playlist.artwork_url(), cx);

    let mut header_left = div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(render_cover(pl_cov, 34.0, false, pal))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(17.0))
                        .font_weight(FontWeight::BOLD)
                        .line_height(relative(1.2))
                        .text_color(pal.ink)
                        .child("Добавить треки"),
                )
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(pal.ink_2)
                        .truncate()
                        .child(format!("в «{}»", playlist.title)),
                ),
        );
    if selected_count > 0 {
        header_left = header_left.child(
            div()
                .px(px(9.0))
                .py(px(3.0))
                .rounded(px(999.0))
                .bg(pal.fill)
                .flex_shrink_0()
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink)
                .child(fmt_tracks_count(selected_count as i64)),
        );
    }

    let header = div()
        .h(px(62.0))
        .flex_shrink_0()
        .pl(px(20.0))
        .pr(px(16.0))
        .border_b_1()
        .border_color(pal.line)
        .flex()
        .items_center()
        .justify_between()
        .gap(px(10.0))
        .child(header_left)
        .child(
            icon_button("modal-close-x", Icon::Close, 18.0, false, pal)
                .on_click(cx.listener(|this, _, _, cx| this.dismiss_modal(cx))),
        );

    let left_status: AnyElement = if let Some(err) = &app.core.editor_error {
        div()
            .id("addtr-error")
            .flex_1()
            .min_w_0()
            .text_size(px(13.0))
            .text_color(pal.danger)
            .truncate()
            .child(err.clone())
            .into_any_element()
    } else if selected_count > 0 && !app.core.editor_saving {
        let danger = pal.danger;
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .child(
                div()
                    .id("addtr-clear-sel")
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .text_size(px(13.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(pal.ink_2)
                    .cursor_pointer()
                    .hover(move |s| s.text_color(danger))
                    .on_click(cx.listener(|this, _, _, cx| this.clear_editor_draft_tracks(cx)))
                    .child("Сбросить выбор"),
            )
            .into_any_element()
    } else {
        div()
            .flex_1()
            .min_w_0()
            .text_size(px(13.0))
            .text_color(pal.ink_3)
            .truncate()
            .child("Выберите треки, чтобы добавить их в плейлист")
            .into_any_element()
    };

    let hover_bg = pal.cta_hover;
    let lead: AnyElement = if app.core.editor_saving {
        spinner("addtr-save-spin", 16.0, pal.cta_fg)
    } else {
        icon(Icon::Plus, 17.0, pal.cta_fg).into_any_element()
    };
    let submit_label = if app.core.editor_saving {
        "Добавление…".into()
    } else if selected_count > 0 {
        format!("Добавить · {selected_count}")
    } else {
        "Добавить".into()
    };
    let submit_base = div()
        .id("addtr-submit")
        .h(px(40.0))
        .pl(px(14.0))
        .pr(px(18.0))
        .rounded(px(6.0))
        .bg(pal.cta_bg)
        .text_color(pal.cta_fg)
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(lead)
        .child(
            div()
                .text_size(px(14.5))
                .font_weight(FontWeight::BOLD)
                .child(submit_label),
        );
    let submit: AnyElement = if app.core.editor_saving || selected_count == 0 {
        submit_base.opacity(0.5).cursor_default().into_any_element()
    } else {
        submit_base
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg))
            .on_click(cx.listener(|this, _, _, cx| this.submit_add_tracks_to_playlist(cx)))
            .into_any_element()
    };

    let cancel = modal_cancel("addtr-cancel", app.core.editor_saving, pal, cx);
    let footer = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .flex_shrink_0()
        .border_t_1()
        .border_color(pal.line)
        .px(px(20.0))
        .py(px(12.0))
        .child(left_status)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .flex_shrink_0()
                .child(cancel)
                .child(submit),
        );

    div()
        .flex_1()
        .min_h_0()
        .w_full()
        .flex()
        .flex_col()
        .child(header)
        .child(render_creator_picker_pane(app, pal, cx))
        .child(footer)
        .into_any_element()
}

fn render_confirm_delete_playlist_modal(
    app: &VivelyApp,
    playlist: &Playlist,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let cov = app.resolve_artwork(playlist.artwork_url(), cx);
    let pl_del = playlist.clone();
    let danger_bg = pal.danger;
    let danger_hover = pal.danger.opacity(0.86);

    let body = div()
        .px(px(24.0))
        .pt(px(8.0))
        .pb(px(20.0))
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(
            div()
                .p(px(12.0))
                .rounded(px(8.0))
                .bg(pal.fill)
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(render_cover(cov, 48.0, false, pal))
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
                                .truncate()
                                .child(playlist.title.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(pal.ink_2)
                                .child(fmt_tracks_count(playlist.count)),
                        ),
                ),
        )
        .child(
            div()
                .text_size(px(13.5))
                .line_height(relative(1.45))
                .text_color(pal.ink_2)
                .child("Плейлист будет удалён из ВКонтакте без возможности восстановления."),
        );

    let confirm_btn = div()
        .id("confirm-del-pl-btn")
        .h(px(40.0))
        .px(px(16.0))
        .rounded(px(6.0))
        .bg(danger_bg)
        .text_color(gpui_kit::rgb(0xFFFFFF))
        .flex()
        .items_center()
        .gap(px(7.0))
        .cursor_pointer()
        .hover(move |s| s.bg(danger_hover))
        .on_click(cx.listener(move |this, _, window, cx| {
            this.delete_own_playlist(pl_del.clone(), window, cx);
        }))
        .child(icon(Icon::Trash, 16.0, gpui_kit::rgb(0xFFFFFF).into()))
        .child(
            div()
                .text_size(px(14.5))
                .font_weight(FontWeight::BOLD)
                .child("Удалить"),
        );

    div()
        .flex()
        .flex_col()
        .child(modal_header("Удалить плейлист?", None, pal, cx))
        .child(body)
        .child(
            modal_footer(pal)
                .child(modal_cancel("confirm-del-pl-cancel", false, pal, cx))
                .child(confirm_btn),
        )
        .into_any_element()
}

fn render_add_to_playlist_modal(
    app: &VivelyApp,
    track: &Audio,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let uid = app.signed_in_user_id().unwrap_or(0);
    let q_lc = app.add_pl_query.trim().to_lowercase();
    let own_pls: Vec<Playlist> = app
        .core.playlists
        .iter()
        .filter(|p| p.owner_id == uid || p.is_editable())
        .cloned()
        .collect();
    let filtered_pls: Vec<Playlist> = own_pls
        .iter()
        .filter(|p| q_lc.is_empty() || p.title.to_lowercase().contains(&q_lc))
        .cloned()
        .collect();

    let seed_tr = track.clone();
    let new_pl_btn = pill_button(
        "addpl-new-shortcut",
        Some(Icon::Plus),
        "Новый",
        false,
        pal,
    )
    .h(px(32.0))
    .px(px(11.0))
    .on_click(cx.listener(move |this, _, window, cx| {
        this.open_playlist_editor(None, Some(seed_tr.clone()), window, cx);
    }))
    .into_any_element();

    let tr_cov = app.resolve_artwork(track.thumbnail_url().or_else(|| track.artwork_url()), cx);
    let mut top = div()
        .px(px(20.0))
        .pt(px(12.0))
        .pb(px(10.0))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(
            div()
                .py(px(9.0))
                .px(px(11.0))
                .rounded(px(7.0))
                .bg(pal.fill)
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(render_cover(tr_cov, 40.0, false, pal))
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
                                .text_color(pal.ink)
                                .truncate()
                                .child(track.title.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(12.5))
                                .text_color(pal.ink_2)
                                .truncate()
                                .child(track.artist.clone()),
                        ),
                ),
        );

    if !own_pls.is_empty() {
        let mut search_box = div()
            .h(px(36.0))
            .px(px(12.0))
            .rounded(px(6.0))
            .bg(pal.fill)
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(icon(Icon::Search, 15.0, pal.ink_2))
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&app.add_pl_filter_input)
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false),
                ),
            );
        if !app.add_pl_query.is_empty() {
            let h = pal.fill_2;
            search_box = search_box.child(
                div()
                    .id("addpl-clear-q")
                    .size(px(22.0))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(move |s| s.bg(h))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.clear_add_pl_filter(window, cx);
                    }))
                    .child(icon(Icon::Close, 13.0, pal.ink_2)),
            );
        }
        top = top.child(search_box);
    }

    let list_body: AnyElement = if own_pls.is_empty() {
        let seed_empty = track.clone();
        div()
            .flex_1()
            .min_h_0()
            .px(px(24.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(10.0))
            .child(
                div()
                    .text_size(px(15.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(pal.ink)
                    .child("Нет своих плейлистов"),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(pal.ink_2)
                    .text_center()
                    .child("Создайте плейлист, чтобы добавлять в него треки."),
            )
            .child(
                primary_play_pill("addpl-create-first", Icon::Plus, "Создать плейлист", pal)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_playlist_editor(None, Some(seed_empty.clone()), window, cx);
                    })),
            )
            .into_any_element()
    } else if filtered_pls.is_empty() {
        div()
            .flex_1()
            .min_h_0()
            .px(px(24.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(pal.ink_2)
                    .child("Плейлисты не найдены"),
            )
            .into_any_element()
    } else {
        let rows: Vec<AnyElement> = filtered_pls
            .into_iter()
            .enumerate()
            .map(|(idx, pl)| {
                let cov = app.resolve_artwork(pl.artwork_url(), cx);
                let tr_put = track.clone();
                let pl_put = pl.clone();
                let fid = pl.full_id();
                let added = app.core.add_pl_added.contains(&fid);
                let busy = app.core.add_pl_busy.contains(&fid);
                let row_bg = if added {
                    pal.hi.opacity(0.11)
                } else {
                    transparent_black()
                };
                let row_hover = if added {
                    pal.hi.opacity(0.16)
                } else {
                    pal.hover
                };

                let right_ctl: AnyElement = if busy {
                    div()
                        .size(px(30.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(spinner(format!("put-pl-spin-{idx}"), 16.0, pal.hi))
                        .into_any_element()
                } else if added {
                    div()
                        .h(px(30.0))
                        .px(px(10.0))
                        .rounded(px(6.0))
                        .bg(pal.hi)
                        .text_color(pal.cta_fg)
                        .flex()
                        .items_center()
                        .gap(px(5.0))
                        .text_size(px(12.5))
                        .font_weight(FontWeight::BOLD)
                        .child(icon(Icon::Check, 14.0, pal.cta_fg))
                        .child("Добавлено")
                        .into_any_element()
                } else {
                    pill_button(format!("put-pl-{idx}"), Some(Icon::Plus), "Добавить", false, pal)
                        .h(px(30.0))
                        .px(px(10.0))
                        .into_any_element()
                };

                let mut row = div()
                    .id(format!("addpl-row-{idx}-{fid}"))
                    .py(px(8.0))
                    .px(px(10.0))
                    .rounded(px(7.0))
                    .bg(row_bg)
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .child(render_cover(cov, 38.0, false, pal))
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
                                            .text_color(if added { pal.hi } else { pal.ink })
                                            .truncate()
                                            .child(pl.title.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(pal.ink_2)
                                            .child(fmt_tracks_count(pl.count)),
                                    ),
                            ),
                    )
                    .child(right_ctl);

                if !busy && !added {
                    row = row
                        .cursor_pointer()
                        .hover(move |s| s.bg(row_hover))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.add_track_to_playlist(tr_put.clone(), pl_put.clone(), cx);
                        }));
                }
                row.into_any_element()
            })
            .collect();

        let scroll_area = div()
            .id("addpl-list-scroll")
            .track_scroll(&app.add_pl_scroll)
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .child(
                div()
                    .px(px(12.0))
                    .pb(px(12.0))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .children(rows),
            );

        div()
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(ScrollBounce::new(
                "addpl-list-bounce",
                &app.add_pl_scroll,
                scroll_area,
            ))
            .child(
                div().absolute().inset_0().child(
                    Scrollbar::new(&app.add_pl_scroll)
                        .id("addpl-list-scrollbar")
                        .axis(ScrollbarAxis::Vertical)
                        .viewport_from_layout(),
                ),
            )
            .into_any_element()
    };

    let added_cnt = app.core.add_pl_added.len();
    let footer = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(10.0))
        .flex_shrink_0()
        .border_t_1()
        .border_color(pal.line)
        .px(px(20.0))
        .py(px(12.0))
        .child(
            div()
                .text_size(px(13.0))
                .text_color(pal.ink_2)
                .child(if added_cnt > 0 {
                    format!("Добавлено в плейлистов: {added_cnt}")
                } else {
                    "Можно выбрать несколько плейлистов".into()
                }),
        )
        .child(
            primary_play_pill("addpl-done-btn", Icon::Check, "Готово", pal)
                .h(px(38.0))
                .on_click(cx.listener(|this, _, _, cx| this.dismiss_modal(cx))),
        );

    div()
        .flex_1()
        .min_h_0()
        .w_full()
        .flex()
        .flex_col()
        .child(modal_header(
            "Добавить в плейлист",
            Some(new_pl_btn),
            pal,
            cx,
        ))
        .child(div().h(px(1.0)).bg(pal.line).flex_shrink_0())
        .child(top)
        .child(list_body)
        .child(footer)
        .into_any_element()
}

fn render_audio_upload_modal(
    app: &VivelyApp,
    max_height: f32,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let mut body = div()
        .px(px(24.0))
        .py(px(18.0))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(
            div()
                .text_size(px(13.5))
                .text_color(pal.ink_2)
                .child("Проверьте исполнителя и название перед отправкой MP3 в Мою музыку."),
        );

    for (item, inputs) in app.core.upload_items.iter().zip(&app.upload_inputs) {
        let status_label = match &item.status {
            UploadItemStatus::Pending => ("Готов к отправке".to_owned(), pal.ink_2),
            UploadItemStatus::Uploading => ("Загрузка...".to_owned(), pal.hi),
            UploadItemStatus::Done => ("Загружено".to_owned(), pal.live_dot),
            UploadItemStatus::Failed(err) => (format!("Ошибка: {err}"), pal.danger),
        };
        let editable = matches!(
            item.status,
            UploadItemStatus::Pending | UploadItemStatus::Failed(_)
        ) && !app.core.upload_in_flight;

        body = body.child(
            div()
                .p(px(12.0))
                .rounded(px(6.0))
                .bg(pal.fill)
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(pal.ink_2)
                                .truncate()
                                .child(item.file_name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(status_label.1)
                                .child(status_label.0),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(10.0))
                        .child(
                            div()
                                .flex_1()
                                .h(px(36.0))
                                .px(px(10.0))
                                .rounded(px(6.0))
                                .bg(pal.raised)
                                .border_1()
                                .border_color(pal.line)
                                .flex()
                                .items_center()
                                .child(
                                    Input::new(&inputs.artist)
                                        .disabled(!editable)
                                        .appearance(false)
                                        .bordered(false)
                                        .focus_bordered(false),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .h(px(36.0))
                                .px(px(10.0))
                                .rounded(px(6.0))
                                .bg(pal.raised)
                                .border_1()
                                .border_color(pal.line)
                                .flex()
                                .items_center()
                                .child(
                                    Input::new(&inputs.title)
                                        .disabled(!editable)
                                        .appearance(false)
                                        .bordered(false)
                                        .focus_bordered(false),
                                ),
                        ),
                ),
        );
    }

    let upload = primary_play_pill(
        "upload-confirm",
        Icon::Upload,
        if app.core.upload_in_flight {
            "Загрузка…"
        } else {
            "Загрузить"
        },
        pal,
    )
    .h(px(42.0));
    let upload = if app.core.upload_in_flight || app.core.upload_items.is_empty() {
        upload.opacity(0.5).cursor_default()
    } else {
        upload.on_click(cx.listener(|this, _, _, cx| this.confirm_audio_upload(cx)))
    };

    div()
        .flex()
        .flex_col()
        .child(modal_header("Загрузка аудио", None, pal, cx))
        .child(modal_body("upload-body", max_height, body))
        .child(
            modal_footer(pal)
                .child(modal_cancel("upload-cancel", app.core.upload_in_flight, pal, cx))
                .child(upload),
        )
        .into_any_element()
}

fn render_share_file_modal(
    _app: &VivelyApp,
    track: &Audio,
    state: &ExportState,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let heading = match state {
        ExportState::Exporting => "Сборка MP3...",
        ExportState::Ready(_) => "Файл готов",
        ExportState::Failed(_) => "Ошибка экспорта",
    };

    let body: AnyElement = match state {
        ExportState::Exporting => div()
            .py(px(28.0))
            .px(px(24.0))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(10.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(pal.ink)
                    .child(format!("{} - {}.mp3", track.artist, track.title)),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(pal.ink_2)
                    .child("Загрузка сегментов HLS, расшифровка AES-128 и запись ID3v2.4..."),
            )
            .into_any_element(),
        ExportState::Ready(path) => {
            let p_save = path.clone();
            let p_rev = path.clone();
            div()
                .py(px(28.0))
                .px(px(24.0))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .size(px(52.0))
                        .rounded_full()
                        .bg(pal.fill_2)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon(Icon::Check, 26.0, pal.ink)),
                )
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(pal.ink)
                        .child(format!("{} - {}.mp3", track.artist, track.title)),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(pal.ink_2)
                        .child("Оригинальный поток 320 кбит/с с тегами ID3v2.4 и обложкой"),
                )
                .child(
                    div()
                        .mt(px(12.0))
                        .flex()
                        .flex_wrap()
                        .justify_center()
                        .gap(px(10.0))
                        .child(
                            primary_play_pill("share-save-as", Icon::Share, "Сохранить как…", pal)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.save_exported_mp3_as(p_save.clone(), cx);
                                })),
                        )
                        .child(
                            pill_button(
                                "share-reveal",
                                Some(Icon::Folder),
                                "Показать в Finder",
                                false,
                                pal,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.reveal_exported_mp3(&p_rev, cx);
                                },
                            )),
                        )
                        .child(
                            pill_button("share-done", None, "Готово", false, pal).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.router.modal = None;
                                    cx.notify();
                                }),
                            ),
                        ),
                )
                .into_any_element()
        }
        ExportState::Failed(err) => div()
            .p(px(24.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(pal.danger)
                    .child(format!("Не удалось собрать MP3: {err}")),
            )
            .child(div().flex().justify_end().child(
                pill_button("share-err-close", None, "Закрыть", false, pal).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.router.modal = None;
                        cx.notify();
                    },
                )),
            ))
            .into_any_element(),
    };

    div()
        .flex()
        .flex_col()
        .child(modal_header(heading, None, pal, cx))
        .child(body)
        .into_any_element()
}

fn render_settings_modal(
    app: &VivelyApp,
    tab: SettingsTab,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let tabs = div()
        .px(px(24.0))
        .pb(px(14.0))
        .border_b_1()
        .border_color(pal.line)
        .flex()
        .gap(px(6.0))
        .child(
            mood_chip("stab-gen", "Основные", tab == SettingsTab::General, pal).on_click(
                cx.listener(|this, _, _, cx| {
                    this.router.modal = Some(ActiveModal::Settings {
                        tab: SettingsTab::General,
                    });
                    cx.notify();
                }),
            ),
        )
        .child(
            mood_chip("stab-dl", "Загрузки", tab == SettingsTab::Downloads, pal).on_click(
                cx.listener(|this, _, _, cx| {
                    this.router.modal = Some(ActiveModal::Settings {
                        tab: SettingsTab::Downloads,
                    });
                    cx.notify();
                }),
            ),
        )
        .child(
            mood_chip("stab-acct", "Аккаунт", tab == SettingsTab::Account, pal).on_click(
                cx.listener(|this, _, _, cx| {
                    this.router.modal = Some(ActiveModal::Settings {
                        tab: SettingsTab::Account,
                    });
                    cx.notify();
                }),
            ),
        )
        .child(
            mood_chip("stab-about", "О приложении", tab == SettingsTab::About, pal).on_click(
                cx.listener(|this, _, _, cx| {
                    this.router.modal = Some(ActiveModal::Settings {
                        tab: SettingsTab::About,
                    });
                    cx.notify();
                }),
            ),
        );

    let body = match tab {
        SettingsTab::General => {
            let xfade = app.core.settings.crossfade_duration;
            let xfade_label = if xfade > 0.0 {
                format!("Кроссфейд ({:.0} с)", xfade)
            } else {
                "Кроссфейд (Выкл)".into()
            };
            let xfade_pct = (xfade / 12.0).clamp(0.0, 1.0) as f32;
            let view = cx.entity();

            div()
                .px(px(24.0))
                .py(px(18.0))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(pref_card(
                    "Тема",
                    "Оформление окна приложения",
                    div()
                        .flex()
                        .gap(px(4.0))
                        .child(
                            mood_chip(
                                "theme-dark",
                                "Тёмная",
                                app.core.settings.appearance == Appearance::Dark,
                                pal,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_appearance(Appearance::Dark, cx);
                            })),
                        )
                        .child(
                            mood_chip(
                                "theme-light",
                                "Светлая",
                                app.core.settings.appearance == Appearance::Light,
                                pal,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_appearance(Appearance::Light, cx);
                            })),
                        )
                        .child(
                            mood_chip(
                                "theme-sys",
                                "Авто",
                                app.core.settings.appearance == Appearance::System,
                                pal,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_appearance(Appearance::System, cx);
                            })),
                        )
                        .into_any_element(),
                    pal,
                ))
                .child(pref_card(
                    &xfade_label,
                    "Плавно смешивает конец трека с началом следующего (0..12 с)",
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(
                            div()
                                .id("xfade-slider")
                                .relative()
                                .w(px(120.0))
                                .h(px(18.0))
                                .flex()
                                .items_center()
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                                        if let Some(b) = this.last_xfade_bounds {
                                            let w = f32::from(b.size.width).max(1.0);
                                            let x =
                                                f32::from(ev.position.x - b.origin.x).clamp(0.0, w);
                                            let secs = ((x / w) * 12.0).round() as f64;
                                            this.set_crossfade_secs(secs, cx);
                                        }
                                    }),
                                )
                                .child(
                                    canvas(
                                        move |bounds, _, cx| {
                                            view.update(cx, |this, _| {
                                                this.last_xfade_bounds = Some(bounds);
                                            });
                                        },
                                        |_, _, _, _| {},
                                    )
                                    .absolute()
                                    .inset_0(),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .h(px(4.0))
                                        .rounded(px(2.0))
                                        .bg(pal.fill_2)
                                        .overflow_hidden()
                                        .child(div().h_full().w(relative(xfade_pct)).bg(pal.hi)),
                                ),
                        )
                        .child(toggle_pill("xfade-toggle", xfade > 0.0, pal).on_click(
                            cx.listener(|this, _, _, cx| this.toggle_crossfade_default(cx)),
                        ))
                        .into_any_element(),
                    pal,
                ))
                .child(pref_card(
                    "Внешние источники текстов",
                    "Загружать синхронизированные и обычные тексты из Musixmatch и Genius",
                    toggle_pill("genius-toggle", app.core.settings.genius_fallback_enabled, pal)
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_genius_fallback(cx)))
                        .into_any_element(),
                    pal,
                ))
        }
        SettingsTab::Downloads => {
            let mut lim_chips = div().flex().flex_wrap().gap(px(6.0));
            for (idx, lim) in CACHE_LIMIT_OPTIONS.into_iter().enumerate() {
                let sel = app.core.settings.cache_limit_bytes == lim;
                let lbl = if lim == UNLIMITED_CACHE {
                    "Без ограничений".to_owned()
                } else {
                    format!("{} ГБ", lim / (1024 * 1024 * 1024))
                };
                lim_chips = lim_chips.child(
                    mood_chip(format!("cache-lim-{idx}"), lbl, sel, pal).on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.set_cache_limit(lim, cx);
                        },
                    )),
                );
            }

            let denom = if app.core.settings.cache_limit_bytes > 0 {
                app.core.settings
                    .cache_limit_bytes
                    .max(app.core.cache_usage.total)
                    .max(1) as f32
            } else {
                (app.core.cache_usage.total.max(1024 * 1024 * 1024)) as f32
            };
            let pin_pct = (app.core.cache_usage.pinned as f32 / denom).clamp(0.0, 1.0);
            let auto_bytes = app.core.cache_usage.total.saturating_sub(app.core.cache_usage.pinned);
            let auto_pct = (auto_bytes as f32 / denom).clamp(0.0, 1.0 - pin_pct);

            div()
                .px(px(24.0))
                .py(px(18.0))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(pref_card(
                    "Автозагрузка лайков",
                    "Треки из Моей музыки сохраняются при прослушивании",
                    toggle_pill("autocache-toggle", app.core.settings.auto_cache_enabled, pal)
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_auto_cache(cx)))
                        .into_any_element(),
                    pal,
                ))
                .child(pref_card(
                    "Только по Wi-Fi",
                    "Не тратить мобильный трафик на автозагрузку",
                    toggle_pill("wifi-toggle", app.core.settings.downloads_wifi_only, pal)
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_wifi_only(cx)))
                        .into_any_element(),
                    pal,
                ))
                .child(
                    div()
                        .py(px(12.0))
                        .px(px(14.0))
                        .rounded(px(6.0))
                        .bg(pal.fill)
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .child(
                                    div()
                                        .text_size(px(14.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(pal.ink)
                                        .child("Лимит кэша"),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(pal.ink_2)
                                        .child("Скачанное вручную не удаляется лимитом"),
                                ),
                        )
                        .child(lim_chips),
                )
                .child(
                    div()
                        .py(px(12.0))
                        .px(px(14.0))
                        .rounded(px(6.0))
                        .bg(pal.fill)
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .child(
                                    div()
                                        .text_size(px(14.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(pal.ink)
                                        .child("Хранилище"),
                                )
                                .child(div().text_size(px(13.0)).text_color(pal.ink_2).child(
                                    format!(
                                        "{}, из них закреплено {}",
                                        fmt_gb(app.core.cache_usage.total),
                                        fmt_gb(app.core.cache_usage.pinned)
                                    ),
                                )),
                        )
                        .child(
                            div()
                                .mt(px(10.0))
                                .mb(px(8.0))
                                .h(px(8.0))
                                .bg(pal.fill_2)
                                .flex()
                                .overflow_hidden()
                                .child(div().h_full().w(relative(pin_pct)).bg(pal.hi))
                                .child(div().h_full().w(relative(auto_pct)).bg(pal.ink_3)),
                        )
                        .child(
                            div()
                                .mt(px(6.0))
                                .flex()
                                .gap(px(8.0))
                                .child(
                                    pill_button(
                                        "clear-autocache",
                                        None,
                                        "Очистить автокэш",
                                        false,
                                        pal,
                                    )
                                    .on_click(
                                        cx.listener(|this, _, _, cx| {
                                            this.clear_auto_cache_only(cx)
                                        }),
                                    ),
                                )
                                .child(
                                    pill_button(
                                        "clear-all-dl",
                                        None,
                                        "Удалить все загрузки",
                                        false,
                                        pal,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| this.clear_all_downloads_action(cx),
                                    )),
                                ),
                        ),
                )
        }
        SettingsTab::Account => {
            let user = app.core.session.as_ref().and_then(|s| s.user.as_ref());
            let full_name = user
                .map(|u| u.full_name())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "Аккаунт VK".into());
            let handle = user
                .and_then(|u| u.screen_name.as_ref())
                .map(|s| format!("@{s}"))
                .unwrap_or_else(|| "ВКонтакте".into());
            let initials = user
                .map(|u| make_initials(&u.first_name, &u.last_name))
                .unwrap_or_else(|| "VK".into());
            let av_path = app.resolve_artwork(user.and_then(|u| u.photo_200.as_ref()), cx);
            let uid = app.signed_in_user_id().unwrap_or(1);

            div()
                .px(px(24.0))
                .py(px(18.0))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(
                    div()
                        .py(px(12.0))
                        .px(px(14.0))
                        .rounded(px(6.0))
                        .bg(pal.fill)
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(12.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(12.0))
                                .child(render_avatar(av_path, uid, &initials, 44.0, pal.art_ring))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .text_size(px(14.5))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(pal.ink)
                                                .child(full_name),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(13.0))
                                                .text_color(pal.ink_2)
                                                .child(handle),
                                        ),
                                ),
                        )
                        .child(
                            pill_button("acct-sign-out", None, "Выйти", false, pal).on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.sign_out(window, cx);
                                }),
                            ),
                        ),
                )
                .child(pref_card(
                    "Транслировать в статус",
                    "Играющий трек виден в вашем статусе ВКонтакте",
                    toggle_pill("bcast-toggle", app.core.settings.broadcast_enabled, pal)
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_broadcast_setting(cx)))
                        .into_any_element(),
                    pal,
                ))
                .child(pref_card(
                    "Учитывать прослушивания",
                    "Попадают в Недавно прослушанное и влияют на рекомендации",
                    toggle_pill(
                        "report-toggle",
                        app.core.settings.playback_reporting_enabled,
                        pal,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_reporting_setting(cx)))
                    .into_any_element(),
                    pal,
                ))
        }
        SettingsTab::About => {
            div()
                .px(px(24.0))
                .py(px(18.0))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(
                    div()
                        .py(px(12.0))
                        .px(px(14.0))
                        .rounded(px(6.0))
                        .bg(pal.fill)
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(
                            div()
                                .w(px(40.0))
                                .h(px(40.0))
                                .rounded(px(10.0))
                                .bg(pal.ink)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(icon(Icon::Logo, 22.0, pal.ground)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_size(px(14.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(pal.ink)
                                        .child("Vively"),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(pal.ink_2)
                                        .child(format!("Версия {}", env!("CARGO_PKG_VERSION"))),
                                ),
                        ),
                )
                .child(pref_card(
                    "Разработчик",
                    "rfc9849",
                    pill_button("about-dev-github", None, "GitHub", false, pal)
                        .on_click(|_, _, cx| cx.open_url("https://github.com/rfc9849"))
                        .into_any_element(),
                    pal,
                ))
                .child(pref_card(
                    "Исходный код",
                    "github.com/rfc9849/Vively",
                    pill_button("about-repo", None, "Открыть", false, pal)
                        .on_click(|_, _, cx| cx.open_url("https://github.com/rfc9849/Vively"))
                        .into_any_element(),
                    pal,
                ))
                .child(pref_card(
                    "Лицензия",
                    "MIT License",
                    pill_button("about-license", None, "Открыть", false, pal)
                        .on_click(|_, _, cx| {
                            cx.open_url("https://github.com/rfc9849/Vively/blob/main/LICENSE")
                        })
                        .into_any_element(),
                    pal,
                ))
        }
    };

    let foot = div()
        .pt(px(12.0))
        .px(px(24.0))
        .pb(px(20.0))
        .flex()
        .justify_end()
        .child(
            primary_play_pill("settings-done", Icon::Check, "Готово", pal).on_click(cx.listener(
                |this, _, _, cx| {
                    this.router.modal = None;
                    cx.notify();
                },
            )),
        );

    div()
        .flex()
        .flex_col()
        .child(modal_header("Настройки", None, pal, cx))
        .child(tabs)
        .child(body)
        .child(foot)
        .into_any_element()
}

fn pref_card(title: &str, sub: &str, right: AnyElement, pal: &Palette) -> AnyElement {
    div()
        .py(px(12.0))
        .px(px(14.0))
        .rounded(px(6.0))
        .bg(pal.fill)
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.0))
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
                        .child(title.to_owned()),
                )
                .child(
                    div()
                        .mt(px(2.0))
                        .text_size(px(13.0))
                        .text_color(pal.ink_2)
                        .child(sub.to_owned()),
                ),
        )
        .child(right)
        .into_any_element()
}
