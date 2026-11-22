pub mod components;
pub mod full_player;
pub mod overlays;
pub mod pages;
pub mod player_dock;
pub mod side_panel;
pub mod sidebar;
pub mod top_bar;

use gpui_kit::{
    AnyElement, App, Context, FontWeight, InteractiveElement as _, IntoElement, KeyBinding,
    KeyDownEvent, Menu, MenuItem, ParentElement as _, Render, StatefulInteractiveElement as _,
    Styled as _, Window, actions,
    base::ScrollBounce,
    component::{
        WindowExt as _,
        scroll::{Scrollbar, ScrollbarAxis},
    },
    div, image_cache, px,
};

use crate::state::{
    app_state::VivelyApp,
    router::{ActiveModal, PanelTab, Route, SettingsTab},
};

use self::{
    full_player::render_full_player,
    overlays::{
        auth::render_signed_out_screen, captcha::render_captcha_overlay,
        menus::render_context_menu, modals::render_modal_layer,
    },
    pages::{
        artist::render_artist_page,
        collection::render_collection_page,
        downloads::render_downloads_page,
        friends::{render_friend_music_page, render_friends_page},
        home::render_home_page,
        library::render_library_page,
        search::render_search_page,
        section::render_section_page,
    },
    player_dock::render_player_dock,
    side_panel::render_side_panel,
    sidebar::render_left_rail,
    top_bar::render_top_bar,
};

actions!(
    vively,
    [
        FocusSearch,
        RefreshPage,
        ToggleFullPlayer,
        ToggleQueuePanel,
        ToggleLyricsPanel,
        NewPlaylist,
        NextTrack,
        PrevTrack,
        VolumeUp,
        VolumeDown,
        OpenSettings,
        QuitApp,
    ]
);

pub fn install_shortcuts_and_menus(cx: &mut App) {
    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-f", FocusSearch, Some("VivelyRoot")),
        KeyBinding::new("cmd-n", NewPlaylist, Some("VivelyRoot")),
        KeyBinding::new("cmd-r", RefreshPage, Some("VivelyRoot")),
        KeyBinding::new("cmd-shift-p", ToggleFullPlayer, Some("VivelyRoot")),
        KeyBinding::new("alt-cmd-u", ToggleQueuePanel, Some("VivelyRoot")),
        KeyBinding::new("alt-cmd-l", ToggleLyricsPanel, Some("VivelyRoot")),
        KeyBinding::new("cmd-right", NextTrack, Some("VivelyRoot")),
        KeyBinding::new("cmd-left", PrevTrack, Some("VivelyRoot")),
        KeyBinding::new("cmd-up", VolumeUp, Some("VivelyRoot")),
        KeyBinding::new("cmd-down", VolumeDown, Some("VivelyRoot")),
        KeyBinding::new("cmd-,", OpenSettings, Some("VivelyRoot")),
        KeyBinding::new("cmd-q", QuitApp, None),
    ]);

    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([
        KeyBinding::new("ctrl-f", FocusSearch, Some("VivelyRoot")),
        KeyBinding::new("ctrl-n", NewPlaylist, Some("VivelyRoot")),
        KeyBinding::new("ctrl-r", RefreshPage, Some("VivelyRoot")),
        KeyBinding::new("ctrl-shift-p", ToggleFullPlayer, Some("VivelyRoot")),
        KeyBinding::new("alt-ctrl-u", ToggleQueuePanel, Some("VivelyRoot")),
        KeyBinding::new("alt-ctrl-l", ToggleLyricsPanel, Some("VivelyRoot")),
        KeyBinding::new("ctrl-right", NextTrack, Some("VivelyRoot")),
        KeyBinding::new("ctrl-left", PrevTrack, Some("VivelyRoot")),
        KeyBinding::new("ctrl-up", VolumeUp, Some("VivelyRoot")),
        KeyBinding::new("ctrl-down", VolumeDown, Some("VivelyRoot")),
        KeyBinding::new("ctrl-,", OpenSettings, Some("VivelyRoot")),
        KeyBinding::new("ctrl-q", QuitApp, None),
    ]);

    cx.set_menus(vec![
        Menu {
            name: "Vively".into(),
            disabled: false,
            items: vec![
                MenuItem::action("Настройки…", OpenSettings),
                MenuItem::separator(),
                MenuItem::action("Выйти из Vively", QuitApp),
            ],
        },
        Menu {
            name: "Вид".into(),
            disabled: false,
            items: vec![
                MenuItem::action("Найти", FocusSearch),
                MenuItem::action("Новый плейлист", NewPlaylist),
                MenuItem::action("Обновить", RefreshPage),
                MenuItem::separator(),
                MenuItem::action("Полноэкранный плеер", ToggleFullPlayer),
                MenuItem::action("Очередь", ToggleQueuePanel),
                MenuItem::action("Текст песни", ToggleLyricsPanel),
            ],
        },
        Menu {
            name: "Воспроизведение".into(),
            disabled: false,
            items: vec![
                MenuItem::action("Следующий трек", NextTrack),
                MenuItem::action("Предыдущий трек", PrevTrack),
                MenuItem::separator(),
                MenuItem::action("Громче", VolumeUp),
                MenuItem::action("Тише", VolumeDown),
            ],
        },
    ]);
}

impl Render for VivelyApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = self.sync_theme(Some(window), cx);
        let vp = window.viewport_size();
        let vw = f32::from(vp.width).max(640.0);
        let vh = f32::from(vp.height).max(480.0);
        let is_fullscreen = window.is_fullscreen();
        let images = self.image_cache.clone();
        images.update(cx, |cache, cx| cache.begin_frame(window, cx));
        self.sync_lyrics_scroll(vw, window, cx);

        let main_view: AnyElement = if self.core.session.is_none() {
            render_signed_out_screen(self, &pal, cx)
        } else {
            let rail_w = if vw <= 1180.0 { 216.0 } else { 236.0 };
            let show_panel = self.router.panel_open && vw > 1180.0;
            let panel_w = if show_panel { 340.0 } else { 0.0 };
            let center_w = (vw - rail_w - panel_w).max(320.0);

            let route = self.router.current().clone();
            let canvas_body = match &route {
                Route::Home => render_home_page(self, center_w, &pal, cx),
                Route::Search => render_search_page(self, center_w, &pal, cx),
                Route::Library => render_library_page(self, center_w, &pal, cx),
                Route::Collection(cid) => render_collection_page(self, cid, false, &pal, cx),
                Route::DownloadedCol(cid) => render_collection_page(self, cid, true, &pal, cx),
                Route::Artist(aid) => render_artist_page(self, aid, center_w, &pal, cx),
                Route::Friends => render_friends_page(self, &pal, cx),
                Route::FriendMusic(fid) => render_friend_music_page(self, *fid, center_w, &pal, cx),
                Route::Downloads => render_downloads_page(self, &pal, cx),
                Route::Section(target) => render_section_page(self, target, center_w, &pal, cx),
            };

            let center_island = div()
                .relative()
                .flex_1()
                .min_w_0()
                .h_full()
                .min_h_0()
                .bg(pal.ground)
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(render_top_bar(self, &pal, window, cx))
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .child(ScrollBounce::new(
                            "main-canvas-bounce",
                            &self.canvas_scroll,
                            div()
                                .id("main-canvas-scroll")
                                .track_scroll(&self.canvas_scroll)
                                .size_full()
                                .flex()
                                .flex_col()
                                .overflow_y_scroll()
                                .child(
                                    div()
                                        .flex_none()
                                        .min_h_full()
                                        .pb(px(108.0))
                                        .child(canvas_body),
                                ),
                        ))
                        .child(
                            div().absolute().inset_0().child(
                                Scrollbar::new(&self.canvas_scroll)
                                    .id("main-canvas-scrollbar")
                                    .axis(ScrollbarAxis::Vertical)
                                    .viewport_from_layout(),
                            ),
                        ),
                )
                .child(render_player_dock(self, center_w, &pal, cx));

            let mut cols = div()
                .size_full()
                .min_h_0()
                .flex()
                .child(render_left_rail(self, rail_w, is_fullscreen, &pal, cx))
                .child(center_island);

            if show_panel {
                cols = cols.child(render_side_panel(self, vh, &pal, cx));
            }
            cols.into_any_element()
        };

        let toast_host = render_toasts(self, &pal);

        let root = div()
            .id("vively-root")
            .key_context("VivelyRoot")
            .track_focus(&self.focus_handle)
            .size_full()
            .relative()
            .bg(pal.ground)
            .text_color(pal.ink)
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                if this.core.session.is_some() {
                    this.navigate_to(Route::Search, window, cx);
                    this.top_search_input
                        .update(cx, |s, cx| s.focus(window, cx));
                }
            }))
            .on_action(cx.listener(|this, _: &RefreshPage, _, cx| {
                if this.core.session.is_some() {
                    this.refresh_current_route(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleFullPlayer, _, cx| {
                if this.core.queue.current().is_some() {
                    this.router.stage_open = !this.router.stage_open;
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleQueuePanel, _, cx| {
                this.router.toggle_panel(PanelTab::Queue);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleLyricsPanel, _, cx| {
                this.router.toggle_panel(PanelTab::Lyrics);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NewPlaylist, window, cx| {
                if this.core.session.is_some() && !this.modal_busy() {
                    this.router.stage_open = false;
                    this.open_playlist_editor(None, None, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &NextTrack, _, cx| {
                this.next_track(cx);
            }))
            .on_action(cx.listener(|this, _: &PrevTrack, _, cx| {
                this.prev_track(cx);
            }))
            .on_action(cx.listener(|this, _: &VolumeUp, _, cx| {
                let next = (this.core.audio_engine.volume() + 0.1).min(1.0);
                this.set_volume(next, cx);
            }))
            .on_action(cx.listener(|this, _: &VolumeDown, _, cx| {
                let next = (this.core.audio_engine.volume() - 0.1).max(0.0);
                this.set_volume(next, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, _, cx| {
                if this.core.session.is_some() {
                    this.router.modal = Some(ActiveModal::Settings {
                        tab: SettingsTab::General,
                    });
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|_, _: &QuitApp, _, cx| {
                cx.quit();
            }))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                let ks = &ev.keystroke;
                let has_cmd_ctrl_alt =
                    ks.modifiers.platform || ks.modifiers.control || ks.modifiers.alt;
                if has_cmd_ctrl_alt {
                    return;
                }
                if !ks.modifiers.shift && ks.key == "escape" {
                    if this.active_captcha.is_some() {
                        this.finish_captcha(None, cx);
                        return;
                    }
                    if this.router.context_menu.is_some() {
                        this.router.context_menu = None;
                        cx.notify();
                        return;
                    }
                    if this.router.modal.is_some() {
                        if matches!(
                            this.router.modal,
                            Some(
                                ActiveModal::PlaylistEditor { editing: None }
                                    | ActiveModal::AddTracksToPlaylist { .. }
                            )
                        ) && (this.core.editor_picker_playlist.is_some()
                            || (matches!(
                                this.router.modal,
                                Some(ActiveModal::PlaylistEditor { editing: None })
                            ) && this.editor_picker_open))
                        {
                            this.editor_picker_back(window, cx);
                        } else {
                            this.dismiss_modal(cx);
                        }
                        return;
                    }
                    if this.router.stage_open {
                        this.router.stage_open = false;
                        cx.notify();
                    }
                    return;
                }

                if window.focused_input(cx).is_some()
                    || this.active_captcha.is_some()
                    || this.router.modal.is_some()
                {
                    return;
                }

                let step = if ks.modifiers.shift { 15.0 } else { 5.0 };
                match ks.key.as_str() {
                    "space" if !ks.modifiers.shift && this.core.queue.current().is_some() => {
                        this.toggle_play_pause(cx);
                    }
                    "left" if this.core.queue.current().is_some() => {
                        this.seek_relative(-step, cx);
                    }
                    "right" if this.core.queue.current().is_some() => {
                        this.seek_relative(step, cx);
                    }
                    "m" if !ks.modifiers.shift => {
                        this.toggle_mute(cx);
                    }
                    "l" if !ks.modifiers.shift => {
                        if let Some(cur) = this.core.queue.current().cloned() {
                            this.toggle_like_track(cur, cx);
                        }
                    }
                    "s" if !ks.modifiers.shift && this.core.queue.current().is_some() => {
                        this.toggle_shuffle(cx);
                    }
                    "r" if !ks.modifiers.shift && this.core.queue.current().is_some() => {
                        this.cycle_repeat(cx);
                    }
                    _ => {}
                }
            }))
            .child(main_view)
            .child(render_full_player(self, vw, vh, is_fullscreen, &pal, cx))
            .child(render_context_menu(self, vw, vh, &pal, cx))
            .child(render_modal_layer(self, vw, vh, &pal, cx))
            .child(render_captcha_overlay(self, &pal, cx))
            .child(toast_host);

        image_cache(images).size_full().child(root)
    }
}

fn render_toasts(app: &VivelyApp, pal: &crate::design::colors::Palette) -> AnyElement {
    if app.toasts.is_empty() {
        return div().into_any_element();
    }

    let mut stack = div().flex().flex_col().items_center().gap(px(8.0));

    for toast in &app.toasts {
        stack = stack.child(
            div()
                .py(px(9.0))
                .px(px(14.0))
                .rounded(px(6.0))
                .bg(pal.ink)
                .text_color(pal.ground)
                .shadow(pal.shadow_pop())
                .text_size(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(toast.message.clone()),
        );
    }

    div()
        .absolute()
        .left_0()
        .right_0()
        .bottom(px(100.0))
        .flex()
        .justify_center()
        .child(stack)
        .into_any_element()
}
