use gpui_kit::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, component::input::Input, div,
    px,
};

use crate::{
    design::{colors::Palette, icons::Icon},
    state::app_state::VivelyApp,
    ui::components::{icon_button, pill_button, primary_play_pill},
};

pub fn render_captcha_overlay(
    app: &VivelyApp,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let Some(active) = &app.active_captcha else {
        return div().into_any_element();
    };

    let browser_url = active
        .prompt
        .challenge
        .redirect_url
        .as_ref()
        .filter(|url| matches!(url.scheme(), "http" | "https"))
        .map(|url| url.to_string());

    let head = div()
        .pt(px(18.0))
        .pl(px(24.0))
        .pr(px(18.0))
        .pb(px(12.0))
        .border_b_1()
        .border_color(pal.line)
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(18.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(pal.ink)
                        .child("Подтверждение SmartCaptcha"),
                )
                .child(div().text_size(px(12.5)).text_color(pal.ink_2).child(
                    if active.webview.is_some() {
                        "После подтверждения исходный запрос выполнится автоматически"
                    } else if browser_url.is_some() {
                        "Пройдите проверку в браузере и вставьте адрес страницы результата"
                    } else {
                        "Введите код с картинки"
                    },
                )),
        )
        .child(
            icon_button("captcha-close-btn", Icon::Close, 18.0, false, pal).on_click(cx.listener(
                |this, _, _, cx| {
                    this.finish_captcha(None, cx);
                },
            )),
        );

    let body: AnyElement = if let Some(wv) = &active.webview {
        div()
            .w_full()
            .h(px(460.0))
            .overflow_hidden()
            .child(wv.clone())
            .into_any_element()
    } else {
        let img_el = active
            .prompt
            .challenge
            .image_url
            .as_ref()
            .and_then(|u| app.resolve_artwork(Some(u), cx))
            .map(|path| {
                div()
                    .h(px(80.0))
                    .flex()
                    .justify_center()
                    .child(gpui_kit::img(path).h(px(72.0)))
            });

        let body = div()
            .p(px(24.0))
            .flex()
            .flex_col()
            .gap(px(14.0))
            .children(img_el);
        let body = if let Some(url) = browser_url {
            body.child(
                div().flex().child(
                    pill_button(
                        "captcha-open-browser",
                        None,
                        "Открыть в браузере",
                        false,
                        pal,
                    )
                    .on_click(move |_, _, cx| cx.open_url(&url)),
                ),
            )
        } else {
            body
        };
        body.child(
            div()
                .h(px(38.0))
                .px(px(12.0))
                .rounded(px(6.0))
                .bg(pal.fill)
                .border_1()
                .border_color(pal.line)
                .flex()
                .items_center()
                .child(
                    Input::new(&app.captcha_manual_input)
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false),
                ),
        )
        .child(
            div()
                .flex()
                .justify_end()
                .gap(px(8.0))
                .child(
                    pill_button("captcha-cancel", None, "Отмена", false, pal)
                        .h(px(40.0))
                        .px(px(16.0))
                        .on_click(cx.listener(|this, _, _, cx| this.finish_captcha(None, cx))),
                )
                .child(
                    primary_play_pill("captcha-submit", Icon::Check, "Подтвердить", pal)
                        .on_click(cx.listener(|this, _, _, cx| this.submit_manual_captcha(cx))),
                ),
        )
        .into_any_element()
    };

    div()
        .id("captcha-backdrop")
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
            cx.listener(|this, _, _, cx| {
                this.finish_captcha(None, cx);
            }),
        )
        .child(
            div()
                .w_full()
                .max_w(px(480.0))
                .rounded(px(10.0))
                .bg(pal.raised)
                .border_1()
                .border_color(pal.line)
                .overflow_hidden()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(head)
                .child(body),
        )
        .into_any_element()
}
