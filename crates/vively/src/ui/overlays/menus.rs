use gpui_kit::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Pixels, Point, StatefulInteractiveElement as _, Styled as _, div, px,
};
use vively_core::{
    models::{Audio, Playlist},
    playback::PlaybackOrigin,
};

use crate::{
    design::{
        colors::Palette,
        icons::{Icon, icon},
    },
    state::{
        app_state::VivelyApp,
        router::ContextMenuKind,
    },
};

pub fn render_context_menu(
    app: &VivelyApp,
    viewport_w: f32,
    viewport_h: f32,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let Some(menu) = app.router.context_menu.clone() else {
        return div().into_any_element();
    };

    match menu {
        ContextMenuKind::Track {
            track,
            position,
            editable_playlist,
        } => render_track_menu(
            app,
            track,
            position,
            editable_playlist,
            viewport_w,
            viewport_h,
            pal,
            cx,
        ),
        ContextMenuKind::Playlist { playlist, position } => {
            render_playlist_menu(app, playlist, position, viewport_w, viewport_h, pal, cx)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_track_menu(
    app: &VivelyApp,
    track: Audio,
    pos: Point<Pixels>,
    editable_pl: Option<Playlist>,
    viewport_w: f32,
    viewport_h: f32,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let liked = app.is_liked(&track);
    let pending = app.is_download_pending(&track);
    let downloaded = app.is_downloaded(&track);
    let playable = track.content_restricted.unwrap_or(0) == 0;

    let mw = 252.0_f32;
    let est_h = 420.0_f32;
    let x = f32::from(pos.x).clamp(12.0, (viewport_w - mw - 12.0).max(12.0));
    let raw_y = f32::from(pos.y);
    let y = if raw_y + est_h > viewport_h - 12.0 {
        (raw_y - est_h - 8.0).max(12.0)
    } else {
        raw_y.max(12.0)
    };

    let tr_like = track.clone();
    let tr_next = track.clone();
    let tr_mix = track.clone();
    let fill_2 = pal.fill_2;

    let quick_group = div()
        .pb(px(5.0))
        .mb(px(4.0))
        .border_b_1()
        .border_color(pal.line)
        .flex()
        .gap(px(4.0))
        .child(
            div()
                .id("cm-q-like")
                .flex_1()
                .py(px(8.0))
                .px(px(4.0))
                .rounded(px(6.0))
                .bg(pal.fill)
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(4.0))
                .cursor_pointer()
                .hover(move |s| s.bg(fill_2))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.router.context_menu = None;
                    this.toggle_like_track(tr_like.clone(), cx);
                }))
                .child(icon(
                    if liked { Icon::HeartSlash } else { Icon::Heart },
                    17.0,
                    pal.ink,
                ))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(pal.ink)
                        .child(if liked {
                            "Убрать"
                        } else {
                            "Нравится"
                        }),
                ),
        )
        .child(
            div()
                .id("cm-q-next")
                .flex_1()
                .py(px(8.0))
                .px(px(4.0))
                .rounded(px(6.0))
                .bg(pal.fill)
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(4.0))
                .cursor_pointer()
                .hover(move |s| s.bg(fill_2))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.router.context_menu = None;
                    this.play_next_track(tr_next.clone(), cx);
                }))
                .child(icon(Icon::Queue, 17.0, pal.ink))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(pal.ink)
                        .child("Далее"),
                ),
        )
        .child(
            div()
                .id("cm-q-mix")
                .flex_1()
                .py(px(8.0))
                .px(px(4.0))
                .rounded(px(6.0))
                .bg(pal.fill)
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(4.0))
                .cursor_pointer()
                .hover(move |s| s.bg(fill_2))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.router.context_menu = None;
                    this.start_track_mix(tr_mix.clone(), cx);
                }))
                .child(icon(Icon::Infinity, 17.0, pal.ink))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(pal.ink)
                        .child("Микс"),
                ),
        );

    let mut menu_col = div()
        .w(px(mw))
        .p(px(5.0))
        .rounded(px(10.0))
        .bg(pal.raised)
        .border_1()
        .border_color(pal.line)
        .shadow(pal.shadow_pop())
        .flex()
        .flex_col()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(quick_group);

    let tr_dislike = track.clone();
    let disliked = app.is_disliked(&track);
    menu_col = menu_col.child(
        cm_row(
            "cm-dislike",
            Icon::Close,
            if disliked {
                "Снять «Не нравится»"
            } else {
                "Не нравится"
            },
            None,
            false,
            pal,
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.router.context_menu = None;
            this.toggle_dislike_track(tr_dislike.clone(), cx);
        })),
    );

    let tr_addpl = track.clone();
    menu_col = menu_col.child(
        cm_row(
            "cm-addpl",
            Icon::Plus,
            "Добавить в плейлист",
            None,
            false,
            pal,
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            this.router.context_menu = None;
            this.open_add_to_playlist_modal(tr_addpl.clone(), window, cx);
        })),
    );

    if playable {
        if pending {
            let fid = track.full_id();
            menu_col = menu_col.child(
                cm_row(
                    "cm-cancel-dl",
                    Icon::Close,
                    "Отменить загрузку",
                    None,
                    false,
                    pal,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.router.context_menu = None;
                    this.remove_downloaded_track(&fid, cx);
                })),
            );
        } else if downloaded {
            let fid = track.full_id();
            menu_col = menu_col.child(
                cm_row("cm-rm-dl", Icon::Trash, "Удалить загрузку", None, true, pal).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.router.context_menu = None;
                        this.remove_downloaded_track(&fid, cx);
                    }),
                ),
            );
        } else {
            let tr_dl = track.clone();
            menu_col = menu_col.child(
                cm_row("cm-dl", Icon::Download, "Скачать", None, false, pal).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.router.context_menu = None;
                        this.download_single_track(tr_dl.clone(), cx);
                    },
                )),
            );
        }

        let tr_mp3 = track.clone();
        menu_col = menu_col.child(
            cm_row("cm-mp3", Icon::Share, "Поделиться файлом", None, false, pal).on_click(
                cx.listener(move |this, _, _, cx| {
                    this.router.context_menu = None;
                    this.export_track_mp3(tr_mp3.clone(), cx);
                }),
            ),
        );
    }

    let album_opt = track
        .album
        .clone()
        .filter(|a| a.id.unwrap_or(0) != 0 && a.title.as_deref().is_some_and(|t| !t.is_empty()));
    let artist_opt = track
        .main_artists
        .as_ref()
        .and_then(|a| a.first())
        .map(|a| (a.id.clone(), a.name.clone()));

    if album_opt.is_some() || artist_opt.is_some() {
        menu_col = menu_col.child(div().h(px(1.0)).my(px(4.0)).mx(px(6.0)).bg(pal.line));
        if let Some(alb) = album_opt {
            let alb_title = alb.title.clone();
            menu_col = menu_col.child(
                cm_row(
                    "cm-goto-album",
                    Icon::Album,
                    "Перейти к альбому",
                    alb_title.as_deref(),
                    false,
                    pal,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.router.context_menu = None;
                    this.open_album(&alb, window, cx);
                })),
            );
        }
        if let Some((aid, artist_name)) = artist_opt {
            let artist_name = if artist_name.trim().is_empty() {
                track.artist.clone()
            } else {
                artist_name
            };
            menu_col = menu_col.child(
                cm_row(
                    "cm-goto-artist",
                    Icon::Mic,
                    "Перейти к исполнителю",
                    Some(&track.artist),
                    false,
                    pal,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.router.context_menu = None;
                    this.open_artist(aid.clone(), &artist_name, window, cx);
                })),
            );
        }
    }

    if let Some(pl) = editable_pl {
        let position = app.core.playlist_details.get(&pl.stable_id()).and_then(|d| {
            d.tracks
                .iter()
                .position(|t| (t.owner_id, t.id) == (track.owner_id, track.id))
                .map(|i| (i, d.tracks.len()))
        });
        menu_col = menu_col.child(div().h(px(1.0)).my(px(4.0)).mx(px(6.0)).bg(pal.line));
        if let Some((index, len)) = position {
            if index > 0 {
                let tr_up = track.clone();
                let pl_up = pl.clone();
                menu_col = menu_col.child(
                    cm_row(
                        "cm-move-up",
                        Icon::ChevUp,
                        "Переместить выше",
                        None,
                        false,
                        pal,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.router.context_menu = None;
                        this.move_track_in_playlist(tr_up.clone(), pl_up.clone(), true, cx);
                    })),
                );
            }
            if index + 1 < len {
                let tr_down = track.clone();
                let pl_down = pl.clone();
                menu_col = menu_col.child(
                    cm_row(
                        "cm-move-down",
                        Icon::ChevDown,
                        "Переместить ниже",
                        None,
                        false,
                        pal,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.router.context_menu = None;
                        this.move_track_in_playlist(tr_down.clone(), pl_down.clone(), false, cx);
                    })),
                );
            }
        }
        let tr_rm = track;
        menu_col = menu_col.child(
            cm_row(
                "cm-rm-from-pl",
                Icon::Trash,
                "Удалить из плейлиста",
                None,
                true,
                pal,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.router.context_menu = None;
                this.remove_track_from_playlist(tr_rm.clone(), pl.clone(), cx);
            })),
        );
    }

    div()
        .id("cm-layer")
        .absolute()
        .inset_0()
        .occlude()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.router.context_menu = None;
                cx.notify();
            }),
        )
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(|this, _, _, cx| {
                this.router.context_menu = None;
                cx.notify();
            }),
        )
        .child(div().absolute().left(px(x)).top(px(y)).child(menu_col))
        .into_any_element()
}

fn render_playlist_menu(
    app: &VivelyApp,
    playlist: Playlist,
    pos: Point<Pixels>,
    viewport_w: f32,
    viewport_h: f32,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let mw = 240.0_f32;
    let est_h = 200.0_f32;
    let x = f32::from(pos.x).clamp(12.0, (viewport_w - mw - 12.0).max(12.0));
    let raw_y = f32::from(pos.y);
    let y = if raw_y + est_h > viewport_h - 12.0 {
        (raw_y - est_h - 8.0).max(12.0)
    } else {
        raw_y.max(12.0)
    };

    let pl_play = playlist.clone();
    let pl_open = playlist.clone();
    let can_copy = playlist
        .permissions
        .as_ref()
        .and_then(|p| p.save_as_copy)
        .unwrap_or(false)
        .then(|| playlist.clone());
    let mut menu_col = div()
        .w(px(mw))
        .p(px(5.0))
        .rounded(px(10.0))
        .bg(pal.raised)
        .border_1()
        .border_color(pal.line)
        .shadow(pal.shadow_pop())
        .flex()
        .flex_col()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            cm_row("plcm-play", Icon::Play, "Слушать", None, false, pal).on_click(cx.listener(
                move |this, _, _, cx| {
                    this.router.context_menu = None;
                    this.play_playlist_immediately(pl_play.clone(), false, cx);
                },
            )),
        )
        .child(
            cm_row("plcm-open", Icon::Album, "Открыть", None, false, pal).on_click(cx.listener(
                move |this, _, window, cx| {
                    this.router.context_menu = None;
                    this.open_playlist(pl_open.clone(), window, cx);
                },
            )),
        );

    if playlist.is_editable() {
        let pl_ed = playlist.clone();
        let pl_del = playlist;
        menu_col = menu_col
            .child(div().h(px(1.0)).my(px(4.0)).mx(px(6.0)).bg(pal.line))
            .child(
                cm_row("plcm-edit", Icon::Edit, "Редактировать", None, false, pal).on_click(
                    cx.listener(move |this, _, window, cx| {
                        this.router.context_menu = None;
                        this.open_playlist_editor(Some(pl_ed.clone()), None, window, cx);
                    }),
                ),
            )
            .child(
                cm_row("plcm-del", Icon::Trash, "Удалить плейлист", None, true, pal).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.router.context_menu = None;
                        this.request_delete_playlist(pl_del.clone(), cx);
                    }),
                ),
            );
    } else {
        let is_fol = playlist.is_following.unwrap_or(false)
            || app
                .core.playlists
                .iter()
                .any(|p| p.full_id() == playlist.full_id());
        let pl_fol = playlist;
        menu_col = menu_col
            .child(div().h(px(1.0)).my(px(4.0)).mx(px(6.0)).bg(pal.line))
            .child(
                cm_row(
                    "plcm-fol",
                    if is_fol { Icon::Trash } else { Icon::Plus },
                    if is_fol {
                        "Убрать из медиатеки"
                    } else {
                        "Добавить в медиатеку"
                    },
                    None,
                    is_fol,
                    pal,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.router.context_menu = None;
                    this.toggle_follow_playlist(pl_fol.clone(), cx);
                })),
            );
    }

    if let Some(pl_copy) = can_copy {
        menu_col = menu_col.child(
            cm_row(
                "plcm-copy",
                Icon::Folder,
                "Сохранить копию",
                None,
                false,
                pal,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.router.context_menu = None;
                this.save_playlist_copy(pl_copy.clone(), cx);
            })),
        );
    }

    let _ = PlaybackOrigin::Other;
    div()
        .id("plcm-layer")
        .absolute()
        .inset_0()
        .occlude()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.router.context_menu = None;
                cx.notify();
            }),
        )
        .child(div().absolute().left(px(x)).top(px(y)).child(menu_col))
        .into_any_element()
}

fn cm_row(
    id: &'static str,
    ic: Icon,
    title: &'static str,
    sub: Option<&str>,
    danger: bool,
    pal: &Palette,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let fg = if danger { pal.danger } else { pal.ink };
    let ic_fg = if danger { pal.danger } else { pal.ink_2 };
    let fill_bg = pal.fill;

    let mut text_col = div().flex_1().min_w_0().flex().flex_col().child(
        div()
            .text_size(px(14.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(fg)
            .child(title),
    );
    if let Some(s) = sub.filter(|s| !s.is_empty()) {
        text_col = text_col.child(
            div()
                .text_size(px(12.5))
                .text_color(pal.ink_2)
                .truncate()
                .child(s.to_owned()),
        );
    }

    div()
        .id(id)
        .w_full()
        .min_h(px(34.0))
        .py(px(6.0))
        .px(px(10.0))
        .rounded(px(6.0))
        .flex()
        .items_center()
        .gap(px(11.0))
        .cursor_pointer()
        .hover(move |s| s.bg(fill_bg))
        .child(icon(ic, 17.0, ic_fg))
        .child(text_col)
}
