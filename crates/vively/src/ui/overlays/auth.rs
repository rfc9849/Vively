use gpui_kit::{
    AnyElement, Bounds, Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, WindowControlArea, canvas,
    component::input::Input, div, fill, point, px, relative, rgb, size,
};

use crate::{
    design::{
        colors::Palette,
        icons::{Icon, icon, spinner},
    },
    state::{
        app_state::VivelyApp,
        models::{AuthStep, QrStage},
    },
    ui::components::{icon_button, pill_button, primary_play_pill},
};

pub fn render_signed_out_screen(
    app: &VivelyApp,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    div()
        .size_full()
        .bg(pal.ground)
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(44.0))
                .w_full()
                .flex_shrink_0()
                .window_control_area(WindowControlArea::Drag),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .pb(px(44.0))
                .px(px(24.0))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .w_full()
                        .max_w(px(440.0))
                        .rounded(px(10.0))
                        .bg(pal.raised)
                        .border_1()
                        .border_color(pal.line)
                        .child(render_auth_card_inner(app, false, pal, cx)),
                ),
        )
        .into_any_element()
}

pub fn render_auth_modal_content(
    app: &VivelyApp,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    render_auth_card_inner(app, true, pal, cx)
}

fn render_auth_card_inner(
    app: &VivelyApp,
    closable: bool,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let heading = match &app.core.auth.step {
        AuthStep::Phone => "Добро пожаловать".to_owned(),
        AuthStep::Qr(_) => "Вход по QR-коду".to_owned(),
        AuthStep::Otp { method_label, .. } => method_label.clone(),
        AuthStep::Password { .. } => "Пароль".to_owned(),
    };

    let mut head = div()
        .pt(px(22.0))
        .pl(px(24.0))
        .pr(px(20.0))
        .pb(px(12.0))
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(
                    div()
                        .size(px(30.0))
                        .rounded(px(7.0))
                        .bg(pal.ink)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon(Icon::Logo, 19.0, pal.ground)),
                )
                .child(
                    div()
                        .text_size(px(19.0))
                        .font_weight(FontWeight::BOLD)
                        .line_height(relative(1.2))
                        .text_color(pal.ink)
                        .child(heading),
                ),
        );

    if closable {
        head = head.child(
            icon_button("auth-modal-close", Icon::Close, 18.0, false, pal).on_click(cx.listener(
                |this, _, _, cx| {
                    this.dismiss_modal(cx);
                },
            )),
        );
    }

    let body = match &app.core.auth.step {
        AuthStep::Qr(stage) => render_qr_body(app, stage, pal),
        _ => render_form_body(app, pal, cx),
    };

    let foot = render_auth_footer(app, closable, pal, cx);

    div()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .flex()
        .flex_col()
        .child(head)
        .child(body)
        .child(foot)
        .into_any_element()
}

fn render_form_body(
    app: &VivelyApp,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let (subtext, field_label) = match &app.core.auth.step {
        AuthStep::Phone => (
            "Войдите через ВКонтакте, чтобы слушать музыку".to_owned(),
            "Номер телефона".to_owned(),
        ),
        AuthStep::Otp {
            subtitle,
            placeholder,
            ..
        } => (subtitle.clone(), placeholder.clone()),
        AuthStep::Password { .. } => (
            "Введите пароль от аккаунта ВКонтакте".to_owned(),
            "Пароль".to_owned(),
        ),
        AuthStep::Qr(_) => unreachable!(),
    };

    let input_box = match &app.core.auth.step {
        AuthStep::Phone => Input::new(&app.auth_phone_input)
            .appearance(false)
            .bordered(false)
            .focus_bordered(false),
        AuthStep::Otp { .. } => Input::new(&app.auth_code_input)
            .appearance(false)
            .bordered(false)
            .focus_bordered(false),
        AuthStep::Password { .. } => Input::new(&app.auth_password_input)
            .appearance(false)
            .bordered(false)
            .focus_bordered(false),
        AuthStep::Qr(_) => unreachable!(),
    };

    let mut body = div()
        .px(px(24.0))
        .py(px(16.0))
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(
            div()
                .text_size(px(13.5))
                .text_color(pal.ink_2)
                .child(subtext),
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
                        .child(field_label),
                )
                .child(
                    div()
                        .h(px(38.0))
                        .px(px(12.0))
                        .rounded(px(6.0))
                        .bg(pal.fill)
                        .border_1()
                        .border_color(pal.line)
                        .flex()
                        .items_center()
                        .child(div().flex_1().min_w_0().child(input_box)),
                ),
        );

    if let Some(err) = &app.core.auth.error {
        body = body.child(render_auth_error(err, pal));
    }

    if let AuthStep::Otp { resend_label, .. } = &app.core.auth.step {
        let hover_ink = pal.ink;
        body = body.child(
            div()
                .id("auth-resend-otp")
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.ink_2)
                .cursor_pointer()
                .hover(move |s| s.text_color(hover_ink))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.resend_auth_otp(window, cx);
                }))
                .child(resend_label.clone()),
        );
    }

    body.into_any_element()
}

fn render_qr_body(app: &VivelyApp, stage: &QrStage, pal: &Palette) -> AnyElement {
    let mut body = div()
        .px(px(24.0))
        .py(px(14.0))
        .flex()
        .flex_col()
        .gap(px(14.0));

    match stage {
        QrStage::ConfirmCode { .. } => {
            let input_box = Input::new(&app.auth_code_input)
                .appearance(false)
                .bordered(false)
                .focus_bordered(false);
            body = body
                .child(
                    div()
                        .text_size(px(13.5))
                        .text_color(pal.ink_2)
                        .child("Введите код подтверждения, который появился в приложении ВКонтакте"),
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
                                .child("Код подтверждения"),
                        )
                        .child(
                            div()
                                .h(px(38.0))
                                .px(px(12.0))
                                .rounded(px(6.0))
                                .bg(pal.fill)
                                .border_1()
                                .border_color(pal.line)
                                .flex()
                                .items_center()
                                .child(div().flex_1().min_w_0().child(input_box)),
                        ),
                );
        }
        QrStage::Loading => {
            body = body
                .child(
                    div()
                        .text_size(px(13.5))
                        .text_color(pal.ink_2)
                        .child("Откройте приложение ВКонтакте на телефоне и наведите камеру на QR-код"),
                )
                .child(
                    div()
                        .w_full()
                        .flex()
                        .justify_center()
                        .py(px(4.0))
                        .child(
                            div()
                                .size(px(224.0))
                                .rounded(px(12.0))
                                .bg(pal.fill)
                                .border_1()
                                .border_color(pal.line)
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .gap(px(10.0))
                                .child(spinner("qr-loading-spinner", 26.0, pal.ink_2))
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(pal.ink_2)
                                        .child("Генерация QR-кода..."),
                                ),
                        ),
                );
        }
        QrStage::Ready { size: qr_size, modules, .. } => {
            body = body
                .child(
                    div()
                        .text_size(px(13.5))
                        .text_color(pal.ink_2)
                        .child("Откройте приложение ВКонтакте на телефоне, перейдите в сканер QR-кодов и наведите камеру на экран"),
                )
                .child(
                    div()
                        .w_full()
                        .flex()
                        .justify_center()
                        .py(px(4.0))
                        .child(render_qr_code_box(*qr_size, modules)),
                )
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(px(8.0))
                        .child(spinner("qr-wait-spinner", 14.0, pal.ink_2))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(pal.ink_2)
                                .child("Ожидание сканирования..."),
                        ),
                );
        }
        QrStage::Scanned {
            size: qr_size,
            modules,
            user_name,
            ..
        } => {
            let status_text = match user_name {
                Some(name) => format!("Подтвердите вход для {name} в приложении ВКонтакте"),
                None => "QR-код отсканирован. Подтвердите вход в приложении ВКонтакте".to_owned(),
            };
            body = body
                .child(
                    div()
                        .text_size(px(13.5))
                        .text_color(pal.ink_2)
                        .child("Нажмите «Подтвердить» на телефоне, чтобы завершить вход"),
                )
                .child(
                    div()
                        .w_full()
                        .flex()
                        .justify_center()
                        .py(px(4.0))
                        .child(render_qr_code_box(*qr_size, modules).opacity(0.45)),
                )
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(px(8.0))
                        .child(spinner("qr-confirm-spinner", 14.0, pal.hi))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(pal.ink)
                                .child(status_text),
                        ),
                );
        }
        QrStage::Expired | QrStage::Declined => {
            let message = if matches!(stage, QrStage::Declined) {
                "Вход был отклонён на телефоне"
            } else {
                "Срок действия QR-кода истёк"
            };
            body = body.child(
                div()
                    .w_full()
                    .flex()
                    .justify_center()
                    .py(px(4.0))
                    .child(
                        div()
                            .size(px(224.0))
                            .rounded(px(12.0))
                            .bg(pal.fill)
                            .border_1()
                            .border_color(pal.line)
                            .px(px(20.0))
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(10.0))
                            .child(icon(Icon::QrCode, 32.0, pal.ink_2))
                            .child(
                                div()
                                    .text_size(px(13.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(pal.ink)
                                    .text_center()
                                    .child(message),
                            ),
                    ),
            );
        }
    }

    if let Some(err) = &app.core.auth.error {
        body = body.child(render_auth_error(err, pal));
    }

    body.into_any_element()
}

fn render_qr_code_box(qr_size: usize, modules: &[bool]) -> gpui_kit::Div {
    let modules = modules.to_vec();
    div()
        .size(px(224.0))
        .rounded(px(12.0))
        .bg(rgb(0xFFFFFF))
        .p(px(16.0))
        .flex()
        .items_center()
        .justify_center()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    if qr_size == 0 || modules.len() < qr_size * qr_size {
                        return;
                    }
                    let side = f32::from(bounds.size.width).min(f32::from(bounds.size.height));
                    if side <= 0.0 {
                        return;
                    }
                    let cell = side / qr_size as f32;
                    let origin_x = f32::from(bounds.left()) + (f32::from(bounds.size.width) - side) * 0.5;
                    let origin_y = f32::from(bounds.top()) + (f32::from(bounds.size.height) - side) * 0.5;
                    let black = rgb(0x000000);
                    for row in 0..qr_size {
                        let y0 = (origin_y + row as f32 * cell).round();
                        let y1 = (origin_y + (row + 1) as f32 * cell).round();
                        let row_start = row * qr_size;
                        let mut col = 0;
                        while col < qr_size {
                            if !modules[row_start + col] {
                                col += 1;
                                continue;
                            }
                            let run_start = col;
                            while col < qr_size && modules[row_start + col] {
                                col += 1;
                            }
                            let x0 = (origin_x + run_start as f32 * cell).round();
                            let x1 = (origin_x + col as f32 * cell).round();
                            window.paint_quad(fill(
                                Bounds::new(point(px(x0), px(y0)), size(px(x1 - x0), px(y1 - y0))),
                                black,
                            ));
                        }
                    }
                },
            )
            .size_full(),
        )
}

fn render_auth_error(err: &str, pal: &Palette) -> AnyElement {
    div()
        .px(px(12.0))
        .py(px(8.0))
        .rounded(px(6.0))
        .bg(pal.danger.opacity(0.14))
        .text_size(px(13.0))
        .text_color(pal.danger)
        .child(err.to_owned())
        .into_any_element()
}

fn render_auth_footer(
    app: &VivelyApp,
    closable: bool,
    pal: &Palette,
    cx: &mut Context<VivelyApp>,
) -> AnyElement {
    let mut right = div().flex().items_center().gap(px(8.0));

    match &app.core.auth.step {
        AuthStep::Phone => {
            if closable {
                right = right.child(
                    pill_button("auth-cancel-btn", None, "Закрыть", false, pal).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.dismiss_modal(cx);
                        }),
                    ),
                );
            }
            let cta_text = if app.core.auth.loading {
                "Подождите...".to_owned()
            } else {
                "Продолжить".to_owned()
            };
            right = right.child(
                primary_play_pill("auth-submit-btn", Icon::Check, cta_text, pal).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.submit_auth_primary(window, cx);
                    }),
                ),
            );

            return div()
                .pt(px(8.0))
                .px(px(24.0))
                .pb(px(22.0))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(
                    pill_button("auth-qr-btn", Some(Icon::QrCode), "Войти по QR", false, pal)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.start_qr_auth(window, cx);
                        })),
                )
                .child(right)
                .into_any_element();
        }
        AuthStep::Qr(stage) => {
            right = right.child(
                pill_button("auth-back-btn", None, "Назад", false, pal).on_click(cx.listener(
                    |this, _, window, cx| {
                        this.auth_go_back(window, cx);
                    },
                )),
            );
            match stage {
                QrStage::Expired | QrStage::Declined => {
                    let label = if app.core.auth.loading {
                        "Подождите...".to_owned()
                    } else {
                        "Обновить QR-код".to_owned()
                    };
                    right = right.child(
                        primary_play_pill("auth-qr-refresh-btn", Icon::QrCode, label, pal)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.start_qr_auth(window, cx);
                            })),
                    );
                }
                QrStage::ConfirmCode { .. } => {
                    let label = if app.core.auth.loading {
                        "Подождите...".to_owned()
                    } else {
                        "Подтвердить".to_owned()
                    };
                    right = right.child(
                        primary_play_pill("auth-submit-btn", Icon::Check, label, pal).on_click(
                            cx.listener(|this, _, window, cx| {
                                this.submit_auth_primary(window, cx);
                            }),
                        ),
                    );
                }
                QrStage::Loading | QrStage::Ready { .. } | QrStage::Scanned { .. } => {}
            }
        }
        AuthStep::Otp { .. } | AuthStep::Password { .. } => {
            let primary_label = if matches!(app.core.auth.step, AuthStep::Otp { .. }) {
                "Подтвердить"
            } else {
                "Войти"
            };
            let cta_text = if app.core.auth.loading {
                "Подождите...".to_owned()
            } else {
                primary_label.to_owned()
            };
            right = right
                .child(
                    pill_button("auth-back-btn", None, "Назад", false, pal).on_click(cx.listener(
                        |this, _, window, cx| {
                            this.auth_go_back(window, cx);
                        },
                    )),
                )
                .child(
                    primary_play_pill("auth-submit-btn", Icon::Check, cta_text, pal).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.submit_auth_primary(window, cx);
                        }),
                    ),
                );
        }
    }

    div()
        .pt(px(8.0))
        .px(px(24.0))
        .pb(px(22.0))
        .flex()
        .items_center()
        .justify_end()
        .gap(px(8.0))
        .child(right)
        .into_any_element()
}
