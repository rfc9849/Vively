use std::{sync::Arc, time::Duration};

use tokio::sync::Mutex;
use vively_core::{
    Error, VkClient,
    auth::{
        AuthService, LoginContinuation, OtpChallenge, OtpMethod, QrAuthStatus, QrValidationResult,
        normalize_phone,
    },
    session::{Session, unix_now},
};

use crate::{
    app::{AppCore, CoreEvent, Dirty},
    models::{AuthStep, QrStage},
};

/// Auth text inputs owned by the front-end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthField {
    Phone,
    Code,
    Password,
}

pub struct AuthState {
    pub step: AuthStep,
    pub loading: bool,
    pub error: Option<String>,
    otp: Option<OtpChallenge>,
    qr_generation: u64,
    service: Arc<Mutex<AuthService>>,
    device_name: String,
}

impl AuthState {
    pub(crate) fn new(client: VkClient, device_name: String) -> Self {
        Self {
            step: AuthStep::Phone,
            loading: false,
            error: None,
            otp: None,
            qr_generation: 0,
            service: Arc::new(Mutex::new(AuthService::new(client))),
            device_name,
        }
    }
}

pub fn encode_qr_matrix(data: &str) -> Result<(usize, Vec<bool>), qrcode::types::QrError> {
    let code = qrcode::QrCode::with_error_correction_level(data.as_bytes(), qrcode::EcLevel::M)?;
    let size = code.width();
    let modules = code
        .to_colors()
        .into_iter()
        .map(|color| color == qrcode::Color::Dark)
        .collect();
    Ok((size, modules))
}

fn qr_poll_delay(consecutive_errors: u32) -> Duration {
    let secs = if consecutive_errors == 0 {
        2
    } else {
        (2u64 << consecutive_errors.min(2)).min(8)
    };
    Duration::from_secs(secs)
}

enum PrimaryAction {
    Phone,
    QrRestart,
    QrConfirm,
    Otp(usize),
    Password,
    Nothing,
}

impl AppCore {
    fn auth_fail(&mut self, message: impl Into<String>) {
        self.auth.error = Some(message.into());
        self.mark(Dirty::AUTH);
    }

    fn auth_begin_request(&mut self) {
        self.auth.loading = true;
        self.auth.error = None;
        self.mark(Dirty::AUTH);
    }

    fn focus_auth(&mut self, field: AuthField, clear: bool) {
        self.emit(CoreEvent::FocusAuthField { field, clear });
    }

    fn complete_login(&mut self, session: Session) {
        self.auth.qr_generation += 1;
        self.emit(CoreEvent::LoggedIn(session));
        self.mark(Dirty::AUTH | Dirty::SESSION);
    }

    /// Primary button / Enter on the auth screen. `input` is the text of
    /// whichever field the current step shows (phone, code or password).
    pub fn submit_auth(&mut self, input: &str) {
        if self.auth.loading {
            return;
        }
        let action = match &self.auth.step {
            AuthStep::Phone => PrimaryAction::Phone,
            AuthStep::Qr(QrStage::Expired | QrStage::Declined) => PrimaryAction::QrRestart,
            AuthStep::Qr(QrStage::ConfirmCode { .. }) => PrimaryAction::QrConfirm,
            AuthStep::Qr(_) => PrimaryAction::Nothing,
            AuthStep::Otp { code_len, .. } => PrimaryAction::Otp(*code_len),
            AuthStep::Password { .. } => PrimaryAction::Password,
        };
        let auth = Arc::clone(&self.auth.service);
        match action {
            PrimaryAction::Nothing => {}
            PrimaryAction::QrRestart => self.start_qr_auth(),
            PrimaryAction::Phone => {
                let normalized = normalize_phone(input);
                if !(10..=15).contains(&normalized.len()) {
                    self.auth_fail("Введите корректный номер телефона в международном формате");
                    return;
                }
                self.auth_begin_request();
                self.spawn(
                    async move { auth.lock().await.begin(&normalized).await },
                    |core, res| {
                        core.auth.loading = false;
                        match res {
                            Ok(LoginContinuation::Otp(otp)) => core.apply_otp_challenge(otp),
                            Ok(LoginContinuation::Password) => {
                                core.auth.otp = None;
                                core.focus_auth(AuthField::Password, true);
                                core.auth.step = AuthStep::Password { had_otp: false };
                            }
                            Err(err) => core.auth.error = Some(err.to_string()),
                        }
                        core.mark(Dirty::AUTH);
                    },
                );
            }
            PrimaryAction::QrConfirm => {
                let code = input.trim().to_owned();
                if code.len() < 4 {
                    self.auth_fail("Введите код подтверждения полностью");
                    return;
                }
                self.auth_begin_request();
                let generation = self.auth.qr_generation;
                self.spawn(
                    async move { auth.lock().await.validate_qr_code(&code).await },
                    move |core, res| {
                        if core.auth.qr_generation != generation {
                            return;
                        }
                        core.auth.loading = false;
                        match res {
                            Ok(QrValidationResult::Accepted) => core.auth.error = None,
                            Ok(QrValidationResult::IncorrectCode) => {
                                core.auth.error = Some("Неверный код подтверждения".into());
                            }
                            Ok(QrValidationResult::SessionExpired) => {
                                core.auth.qr_generation += 1;
                                core.auth.step = AuthStep::Qr(QrStage::Expired);
                            }
                            Err(err) => core.auth.error = Some(err.to_string()),
                        }
                        core.mark(Dirty::AUTH);
                    },
                );
            }
            PrimaryAction::Otp(code_len) => {
                let code = input.trim().to_owned();
                if code.len() < code_len.min(4) {
                    self.auth_fail("Введите код подтверждения полностью");
                    return;
                }
                self.auth_begin_request();
                self.spawn(
                    async move {
                        let mut guard = auth.lock().await;
                        let needs_password = guard.submit_otp(&code).await?;
                        if needs_password {
                            Ok::<_, Error>(None)
                        } else {
                            Ok(Some(guard.submit_password("").await?))
                        }
                    },
                    |core, res| {
                        core.auth.loading = false;
                        match res {
                            Ok(Some(session)) => core.complete_login(session),
                            Ok(None) => {
                                core.focus_auth(AuthField::Password, true);
                                core.auth.step = AuthStep::Password { had_otp: true };
                            }
                            Err(err) => core.auth.error = Some(err.to_string()),
                        }
                        core.mark(Dirty::AUTH);
                    },
                );
            }
            PrimaryAction::Password => {
                if input.is_empty() {
                    self.auth_fail("Введите пароль от аккаунта ВКонтакте");
                    return;
                }
                let password = input.to_owned();
                self.auth_begin_request();
                self.spawn(
                    async move { auth.lock().await.submit_password(&password).await },
                    |core, res| {
                        core.auth.loading = false;
                        match res {
                            Ok(session) => core.complete_login(session),
                            Err(err) => core.auth.error = Some(err.to_string()),
                        }
                        core.mark(Dirty::AUTH);
                    },
                );
            }
        }
    }

    pub fn resend_auth_otp(&mut self) {
        if self.auth.loading {
            return;
        }
        self.auth_begin_request();
        let auth = Arc::clone(&self.auth.service);
        self.spawn(
            async move { auth.lock().await.resend_otp().await },
            |core, res| {
                core.auth.loading = false;
                match res {
                    Ok(otp) => {
                        core.apply_otp_challenge(otp);
                        core.toast("Код отправлен повторно");
                    }
                    Err(err) => core.auth.error = Some(err.to_string()),
                }
                core.mark(Dirty::AUTH);
            },
        );
    }

    pub fn start_qr_auth(&mut self) {
        self.cancel_qr_auth();
        self.auth.qr_generation += 1;
        let generation = self.auth.qr_generation;
        self.auth.loading = true;
        self.auth.error = None;
        self.auth.step = AuthStep::Qr(QrStage::Loading);
        self.mark(Dirty::AUTH);

        let auth = Arc::clone(&self.auth.service);
        let device_name = self.auth.device_name.clone();
        self.spawn(
            async move { auth.lock().await.start_qr_auth(&device_name).await },
            move |core, res| {
                if core.auth.qr_generation != generation {
                    return;
                }
                core.auth.loading = false;
                match res {
                    Ok(qr) => match encode_qr_matrix(&qr.auth_url) {
                        Ok((size, modules)) => {
                            core.auth.step = AuthStep::Qr(QrStage::Ready {
                                url: qr.auth_url,
                                size,
                                modules,
                            });
                            core.schedule_qr_poll(generation, qr.expires_at, 0);
                        }
                        Err(err) => {
                            core.auth.error = Some(format!("Не удалось сформировать QR-код: {err}"));
                            core.auth.step = AuthStep::Qr(QrStage::Expired);
                        }
                    },
                    Err(err) => {
                        core.auth.error = Some(err.to_string());
                        core.auth.step = AuthStep::Qr(QrStage::Expired);
                    }
                }
                core.mark(Dirty::AUTH);
            },
        );
    }

    pub fn cancel_qr_auth(&mut self) {
        let was_qr = matches!(self.auth.step, AuthStep::Qr(_));
        self.auth.qr_generation += 1;
        if was_qr {
            let auth = Arc::clone(&self.auth.service);
            self.fire(async move {
                let _ = auth.lock().await.revoke_qr_auth().await;
            });
        }
    }

    fn schedule_qr_poll(&mut self, generation: u64, expires_at: Option<i64>, errors: u32) {
        self.after(qr_poll_delay(errors), move |core| {
            core.qr_poll_tick(generation, expires_at, errors);
        });
    }

    fn qr_poll_tick(&mut self, generation: u64, expires_at: Option<i64>, errors: u32) {
        if self.auth.qr_generation != generation {
            return;
        }
        if expires_at.is_some_and(|exp| unix_now() >= exp) {
            self.auth.step = AuthStep::Qr(QrStage::Expired);
            self.auth.loading = false;
            self.mark(Dirty::AUTH);
            return;
        }
        let auth = Arc::clone(&self.auth.service);
        self.spawn(
            async move { auth.lock().await.check_qr_auth().await },
            move |core, res| core.on_qr_poll(generation, expires_at, errors, res),
        );
    }

    fn on_qr_poll(
        &mut self,
        generation: u64,
        expires_at: Option<i64>,
        errors: u32,
        res: vively_core::Result<QrAuthStatus>,
    ) {
        if self.auth.qr_generation != generation {
            return;
        }
        let mut next_errors = 0;
        match res {
            Ok(QrAuthStatus::Pending) => {}
            Ok(QrAuthStatus::Scanned { user_info }) => {
                let user_name = user_info.and_then(|u| u.display_name());
                let next = match &self.auth.step {
                    AuthStep::Qr(QrStage::Ready { url, size, modules }) => Some(QrStage::Scanned {
                        url: url.clone(),
                        size: *size,
                        modules: modules.clone(),
                        user_name,
                    }),
                    AuthStep::Qr(QrStage::Scanned {
                        url,
                        size,
                        modules,
                        user_name: prev,
                    }) if *prev != user_name && user_name.is_some() => Some(QrStage::Scanned {
                        url: url.clone(),
                        size: *size,
                        modules: modules.clone(),
                        user_name,
                    }),
                    _ => None,
                };
                if let Some(stage) = next {
                    self.auth.step = AuthStep::Qr(stage);
                    self.mark(Dirty::AUTH);
                }
            }
            Ok(QrAuthStatus::NeedConfirmationCode) => {
                let url = match &self.auth.step {
                    AuthStep::Qr(QrStage::Ready { url, .. } | QrStage::Scanned { url, .. }) => {
                        Some(url.clone())
                    }
                    _ => None,
                };
                if let Some(url) = url {
                    self.focus_auth(AuthField::Code, true);
                    self.auth.step = AuthStep::Qr(QrStage::ConfirmCode { url });
                    self.mark(Dirty::AUTH);
                }
            }
            Ok(QrAuthStatus::Authorized(session)) => {
                self.auth.loading = false;
                self.auth.error = None;
                self.auth.step = AuthStep::Phone;
                self.complete_login(session);
                return;
            }
            Ok(QrAuthStatus::Declined) => {
                self.finish_qr(QrStage::Declined, None);
                return;
            }
            Ok(QrAuthStatus::Expired) => {
                self.finish_qr(QrStage::Expired, None);
                return;
            }
            Err(err) => {
                let transient = matches!(err, Error::Http(_) | Error::Transport(_));
                if transient && errors < 3 {
                    next_errors = errors + 1;
                } else {
                    self.finish_qr(QrStage::Expired, Some(err.to_string()));
                    return;
                }
            }
        }
        self.schedule_qr_poll(generation, expires_at, next_errors);
    }

    fn finish_qr(&mut self, stage: QrStage, error: Option<String>) {
        self.auth.loading = false;
        self.auth.error = error;
        self.auth.step = AuthStep::Qr(stage);
        self.mark(Dirty::AUTH);
    }

    pub fn auth_go_back(&mut self) {
        self.auth.error = None;
        self.auth.loading = false;
        match &self.auth.step {
            AuthStep::Phone => {}
            AuthStep::Qr(_) => {
                self.cancel_qr_auth();
                self.auth.step = AuthStep::Phone;
                self.focus_auth(AuthField::Phone, false);
            }
            AuthStep::Otp { .. } => {
                self.auth.step = AuthStep::Phone;
                self.focus_auth(AuthField::Phone, false);
            }
            AuthStep::Password { had_otp } => {
                if *had_otp && let Some(otp) = self.auth.otp.clone() {
                    self.apply_otp_challenge(otp);
                } else {
                    self.auth.step = AuthStep::Phone;
                    self.focus_auth(AuthField::Phone, false);
                }
            }
        }
        self.mark(Dirty::AUTH);
    }

    /// Back to a clean phone step (sign-out, account switch).
    pub fn reset_auth(&mut self) {
        self.cancel_qr_auth();
        self.auth.step = AuthStep::Phone;
        self.auth.loading = false;
        self.auth.error = None;
        self.auth.otp = None;
        self.mark(Dirty::AUTH);
    }

    fn apply_otp_challenge(&mut self, otp: OtpChallenge) {
        let len = otp.code_length;
        let (method_label, subtitle, placeholder, resend_label) = match otp.method {
            OtpMethod::CallReset => (
                "Звонок для входа".to_owned(),
                format!(
                    "Вам поступит звонок — отвечать не нужно. Введите последние {len} цифр номера, с которого звонят."
                ),
                format!("Последние {len} цифр"),
                "Позвонить ещё раз".to_owned(),
            ),
            OtpMethod::Sms => (
                "Введите код".to_owned(),
                format!(
                    "Мы отправили код в SMS{}.",
                    otp.masked_destination
                        .as_deref()
                        .map(|d| format!(" на {d}"))
                        .unwrap_or_default()
                ),
                "Код из SMS".to_owned(),
                "Отправить код снова".to_owned(),
            ),
            OtpMethod::Push => (
                "Подтверждение в приложении".to_owned(),
                format!("Введите {len}-значный код из уведомления ВКонтакте."),
                "Код из Push".to_owned(),
                "Отправить ещё раз".to_owned(),
            ),
            OtpMethod::Email => (
                "Код из письма".to_owned(),
                format!(
                    "Мы отправили код на почту{}.",
                    otp.masked_destination
                        .as_deref()
                        .map(|d| format!(" {d}"))
                        .unwrap_or_default()
                ),
                "Код из письма".to_owned(),
                "Отправить код снова".to_owned(),
            ),
            OtpMethod::Other(_) => (
                "Подтверждение".to_owned(),
                format!("Введите код подтверждения из {len} цифр."),
                "Код подтверждения".to_owned(),
                "Отправить код снова".to_owned(),
            ),
        };
        self.auth.otp = Some(otp);
        self.focus_auth(AuthField::Code, true);
        self.auth.step = AuthStep::Otp {
            method_label,
            subtitle,
            placeholder,
            resend_label,
            code_len: len,
        };
    }
}
