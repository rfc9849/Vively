use gpui_kit::{
    AnyElement, Context, Focusable as _, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window, WindowControlArea,
    component::input::Input, div, px,
};

use crate::{
    design::{
        artwork::{make_initials, render_avatar},
        colors::Palette,
        icons::{Icon, icon},
    },
    state::{
        app_state::VivelyApp,
        router::{ActiveModal, SettingsTab},
    },
    ui::components::icon_button,
};

pub fn render_top_bar(
    app: &VivelyApp,
    pal: &Palette,
    window: &Window,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let can_back = app.router.can_go_back();
    let can_fwd = app.router.can_go_forward();
    let search_focused = app
        .top_search_input
        .read(cx)
        .focus_handle(cx)
        .is_focused(window);
    let has_search_text = !app.core.search_query.is_empty();

    let user = app.core.session.as_ref().and_then(|s| s.user.as_ref());
    let first_name = user
        .map(|u| u.first_name.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Профиль".into());
    let initials = user
        .map(|u| make_initials(&u.first_name, &u.last_name))
        .unwrap_or_else(|| "VK".into());
    let avatar_path = app.resolve_artwork(user.and_then(|u| u.photo_200.as_ref()), cx);
    let uid = app.signed_in_user_id().unwrap_or(1);

    let fill_bg = pal.fill;
    let mut back_btn = div()
        .id("nav-back-btn")
        .size(px(32.0))
        .rounded(px(6.0))
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .opacity(if can_back { 1.0 } else { 0.3 })
        .child(icon(Icon::ChevLeft, 20.0, pal.ink));
    if can_back {
        back_btn = back_btn
            .cursor_pointer()
            .hover(move |s| s.bg(fill_bg))
            .on_click(cx.listener(|this, _, window, cx| {
                this.go_back(window, cx);
            }));
    }

    let mut fwd_btn = div()
        .id("nav-fwd-btn")
        .size(px(32.0))
        .rounded(px(6.0))
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .opacity(if can_fwd { 1.0 } else { 0.3 })
        .child(icon(Icon::ChevRight, 20.0, pal.ink));
    if can_fwd {
        fwd_btn = fwd_btn
            .cursor_pointer()
            .hover(move |s| s.bg(fill_bg))
            .on_click(cx.listener(|this, _, window, cx| {
                this.go_forward(window, cx);
            }));
    }

    let mut search_box = div()
        .h(px(36.0))
        .px(px(12.0))
        .ml(px(8.0))
        .flex_1()
        .min_w_0()
        .rounded(px(6.0))
        .bg(pal.fill)
        .border_1()
        .border_color(if search_focused {
            pal.ink_3
        } else {
            gpui_kit::transparent_black()
        })
        .flex()
        .items_center()
        .gap(px(9.0))
        .child(icon(Icon::Search, 17.0, pal.ink_2))
        .child(
            div().flex_1().min_w_0().child(
                Input::new(&app.top_search_input)
                    .appearance(false)
                    .bordered(false)
                    .focus_bordered(false),
            ),
        );

    if has_search_text {
        search_box = search_box.child(
            div()
                .id("top-search-clear")
                .size(px(22.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .on_click(cx.listener(|this, _, window, cx| {
                    this.clear_search_box(window, cx);
                }))
                .child(icon(Icon::Close, 15.0, pal.ink_2)),
        );
    }

    let theme_icon = if pal.is_dark { Icon::Sun } else { Icon::Moon };
    let mut profile_pill = div()
        .id("profile-pill")
        .h(px(34.0))
        .pl(px(3.0))
        .pr(px(10.0))
        .ml(px(6.0))
        .rounded(px(6.0))
        .flex()
        .items_center()
        .gap(px(8.0))
        .cursor_pointer()
        .hover(move |s| s.bg(fill_bg))
        .on_click(cx.listener(|this, _, _, cx| {
            this.router.modal = Some(ActiveModal::Settings {
                tab: SettingsTab::Account,
            });
            cx.notify();
        }))
        .child(render_avatar(
            avatar_path,
            uid,
            &initials,
            28.0,
            pal.art_ring,
        ))
        .child(
            div()
                .text_size(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink)
                .child(first_name),
        );

    if app.core.settings.broadcast_enabled {
        profile_pill = profile_pill.child(div().size(px(7.0)).rounded_full().bg(pal.live_dot));
    }

    div()
        .h(px(60.0))
        .px(px(32.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .child(
            div()
                .w(px(420.0))
                .max_w(px(460.0))
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(back_btn)
                .child(fwd_btn)
                .child(search_box),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(24.0))
                .h_full()
                .window_control_area(WindowControlArea::Drag),
        )
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(
                    icon_button("theme-toggle-btn", theme_icon, 19.0, false, pal).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.toggle_theme_quick(window, cx);
                        }),
                    ),
                )
                .child(
                    icon_button("open-settings-btn", Icon::Settings, 19.0, false, pal).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.router.modal = Some(ActiveModal::Settings {
                                tab: SettingsTab::General,
                            });
                            cx.notify();
                        }),
                    ),
                )
                .child(profile_pill),
        )
        .into_any_element()
}
