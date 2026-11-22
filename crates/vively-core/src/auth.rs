use serde::{Deserialize, Deserializer, Serialize};
use url::Url;

use crate::{
    Error, Result, VkClient,
    client::{Parameters, params},
    models::VkUser,
    session::{Session, unix_now},
};

#[derive(Clone, Deserialize)]
pub struct AnonymTokenResponse {
    pub token: String,
    pub expired_at: Option<f64>,
}

#[derive(Clone, Deserialize)]
pub struct ValidateAccountResponse {
    pub is_phone: Option<bool>,
    pub flow_name: Option<String>,
    pub flow_names: Option<Vec<String>>,
    pub sid: String,
    pub next_step: Option<NextStep>,
    pub remember_hash: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NextStep {
    pub verification_method: Option<String>,
    pub has_another_verification_methods: Option<bool>,
}

#[derive(Clone, Deserialize)]
pub struct SendOtpResponse {
    pub code_length: Option<usize>,
    pub info: Option<String>,
    pub sid: Option<String>,
    pub status: Option<i64>,
}

#[derive(Clone, Deserialize)]
pub struct CheckOtpResponse {
    pub sid: String,
    pub profile_exist: Option<bool>,
    pub can_skip_password: Option<bool>,
    pub auth_hash: Option<String>,
    pub phone: Option<String>,
    pub profile: Option<ProfilePreview>,
}

#[derive(Clone, Deserialize)]
pub struct ProfilePreview {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub photo_200: Option<Url>,
    pub has_2fa: Option<bool>,
    pub phone: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub expires_in: Option<i64>,
    pub user_id: i64,
    pub trusted_hash: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct ExchangeTokenResponse {
    pub users_exchange_tokens: Vec<ExchangeToken>,
}

#[derive(Clone, Deserialize)]
pub struct ExchangeToken {
    pub common_token: Option<String>,
    pub user_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetAuthCodeResponse {
    pub auth_code: String,
    pub auth_hash: String,
    pub auth_id: Option<String>,
    pub auth_url: String,
    pub expires_in: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct QrUserInfo {
    #[serde(default)]
    pub first_name: Option<String>,
    #[serde(default)]
    pub last_name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_url")]
    pub photo_200: Option<Url>,
}

impl QrUserInfo {
    pub fn display_name(&self) -> Option<String> {
        let name = format!(
            "{} {}",
            self.first_name.as_deref().unwrap_or_default().trim(),
            self.last_name.as_deref().unwrap_or_default().trim()
        )
        .trim()
        .to_owned();
        if name.is_empty() { None } else { Some(name) }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckAuthCodeResponse {
    pub status: i64,
    #[serde(default)]
    pub expires_in: Option<i64>,
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_i64")]
    pub user_id: Option<i64>,
    #[serde(default)]
    pub trusted_hash: Option<String>,
    #[serde(default)]
    pub super_app_token: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_bool_or_int")]
    pub need_password: Option<bool>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub sid: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_qr_user_info")]
    pub user_info: Option<QrUserInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ValidateAuthCodeResponse {
    pub status: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RevokeAuthCodeResponse {
    pub status: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrAuthSession {
    pub auth_code: String,
    pub auth_hash: String,
    pub auth_id: Option<String>,
    pub auth_url: String,
    pub expires_at: Option<i64>,
}

#[derive(Clone, PartialEq)]
pub enum QrAuthStatus {
    Pending,
    Scanned { user_info: Option<QrUserInfo> },
    NeedConfirmationCode,
    Authorized(Session),
    Declined,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrValidationResult {
    Accepted,
    IncorrectCode,
    SessionExpired,
}

fn deserialize_opt_bool_or_int<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Bool(b)) => Ok(Some(b)),
        Some(serde_json::Value::Number(n)) => Ok(n.as_i64().map(|v| v != 0)),
        Some(serde_json::Value::String(s)) => match s.as_str() {
            "1" | "true" => Ok(Some(true)),
            "0" | "false" => Ok(Some(false)),
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

fn deserialize_opt_i64<'de, D>(deserializer: D) -> std::result::Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => Ok(n.as_i64()),
        Some(serde_json::Value::String(s)) => Ok(s.trim().parse::<i64>().ok()),
        _ => Ok(None),
    }
}

fn deserialize_opt_url<'de, D>(deserializer: D) -> std::result::Result<Option<Url>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        Some(serde_json::Value::String(s)) if !s.trim().is_empty() => Ok(Url::parse(s.trim()).ok()),
        _ => Ok(None),
    }
}

fn deserialize_opt_qr_user_info<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<QrUserInfo>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        Some(v @ serde_json::Value::Object(_)) => Ok(serde_json::from_value(v).ok()),
        _ => Ok(None),
    }
}

fn normalize_expires_at(expires_in: Option<i64>) -> Option<i64> {
    match expires_in {
        Some(v) if v <= 0 => None,
        Some(v) if v > 1_000_000_000 => Some(v),
        Some(v) => Some(unix_now().saturating_add(v)),
        None => None,
    }
}

#[derive(Clone, Deserialize)]
pub struct RefreshTokensResponse {
    pub success: Vec<RefreshSuccess>,
}

#[derive(Clone, Deserialize)]
pub struct RefreshSuccess {
    pub user_id: Option<i64>,
    pub access_token: RefreshedToken,
}

#[derive(Clone, Deserialize)]
pub struct RefreshedToken {
    pub token: String,
    pub expires_in: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountChallenge {
    pub verification_method: Option<String>,
    pub needs_password: bool,
    pub can_skip_password: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoginContinuation {
    Otp(OtpChallenge),
    Password,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OtpChallenge {
    pub method: OtpMethod,
    pub code_length: usize,
    pub masked_destination: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OtpMethod {
    Sms,
    CallReset,
    Push,
    Email,
    Other(String),
}

impl OtpMethod {
    pub fn from_channel(channel: Option<&str>) -> Self {
        match channel {
            Some("sms") => Self::Sms,
            None | Some("callreset") => Self::CallReset,
            Some("push" | "codegen") => Self::Push,
            Some("email") => Self::Email,
            Some(other) => Self::Other(other.into()),
        }
    }
}

pub struct AuthService {
    client: VkClient,
    anonym_token: Option<String>,
    login: Option<String>,
    validate_sid: Option<String>,
    verification_method: Option<String>,
    confirmation_sid: Option<String>,
    qr_auth_hash: Option<String>,
    pub profile_preview: Option<ProfilePreview>,
}

impl AuthService {
    pub fn new(client: VkClient) -> Self {
        Self {
            client,
            anonym_token: None,
            login: None,
            validate_sid: None,
            verification_method: None,
            confirmation_sid: None,
            qr_auth_hash: None,
            profile_preview: None,
        }
    }

    pub fn reset(&mut self) {
        self.anonym_token = None;
        self.login = None;
        self.validate_sid = None;
        self.verification_method = None;
        self.confirmation_sid = None;
        self.qr_auth_hash = None;
        self.profile_preview = None;
    }

    pub async fn start_qr_auth(&mut self, device_name: &str) -> Result<QrAuthSession> {
        self.reset();
        let anonym: AnonymTokenResponse = self
            .client
            .oauth("get_anonym_token", Parameters::new(), None)
            .await?;
        self.anonym_token = Some(anonym.token.clone());
        let response: GetAuthCodeResponse = self
            .client
            .method(
                "auth.getAuthCode",
                params([
                    ("device_name", device_name.into()),
                    ("force_regenerate", "1".into()),
                    ("auth_code_flow", "0".into()),
                ]),
                Some(&anonym.token),
            )
            .await?;
        self.qr_auth_hash = Some(response.auth_hash.clone());
        Ok(QrAuthSession {
            auth_code: response.auth_code,
            auth_hash: response.auth_hash,
            auth_id: response.auth_id,
            auth_url: response.auth_url,
            expires_at: normalize_expires_at(response.expires_in),
        })
    }

    pub async fn check_qr_auth(&mut self) -> Result<QrAuthStatus> {
        let (Some(anonym), Some(auth_hash)) =
            (self.anonym_token.clone(), self.qr_auth_hash.clone())
        else {
            return Err(Error::InvalidState(
                "Start QR authorization before polling its status".into(),
            ));
        };
        let response: CheckAuthCodeResponse = self
            .client
            .method(
                "auth.checkAuthCode",
                params([
                    ("auth_hash", auth_hash),
                    ("web_auth", "0".into()),
                    ("scope", "all".into()),
                ]),
                Some(&anonym),
            )
            .await?;
        match response.status {
            0 => Ok(QrAuthStatus::Pending),
            1 => Ok(QrAuthStatus::Scanned {
                user_info: response.user_info,
            }),
            5 => Ok(QrAuthStatus::NeedConfirmationCode),
            3 => {
                self.qr_auth_hash = None;
                Ok(QrAuthStatus::Declined)
            }
            4 => {
                self.qr_auth_hash = None;
                Ok(QrAuthStatus::Expired)
            }
            2 => {
                if let Some(access_token) = response.access_token.filter(|t| !t.is_empty()) {
                    let session = self
                        .build_session(
                            access_token,
                            response.user_id,
                            response.expires_in,
                            response.trusted_hash,
                        )
                        .await?;
                    Ok(QrAuthStatus::Authorized(session))
                } else if response.need_password.unwrap_or(false) {
                    self.qr_auth_hash = None;
                    Err(Error::InvalidState(
                        "Для этого аккаунта требуется вход по номеру телефона и паролю".into(),
                    ))
                } else {
                    self.qr_auth_hash = None;
                    Err(Error::InvalidState(
                        "ВКонтакте не вернул токен доступа после подтверждения QR-кода".into(),
                    ))
                }
            }
            _ => Ok(QrAuthStatus::Pending),
        }
    }

    pub async fn validate_qr_code(&self, validation_code: &str) -> Result<QrValidationResult> {
        let (Some(anonym), Some(auth_hash)) = (&self.anonym_token, &self.qr_auth_hash) else {
            return Err(Error::InvalidState(
                "Start QR authorization before validating a confirmation code".into(),
            ));
        };
        let response: ValidateAuthCodeResponse = self
            .client
            .method(
                "auth.validateAuthCode",
                params([
                    ("auth_hash", auth_hash.clone()),
                    ("validation_code", validation_code.into()),
                ]),
                Some(anonym),
            )
            .await?;
        Ok(match response.status {
            0 => QrValidationResult::Accepted,
            1 => QrValidationResult::IncorrectCode,
            _ => QrValidationResult::SessionExpired,
        })
    }

    pub async fn revoke_qr_auth(&mut self) -> Result<()> {
        let Some(auth_hash) = self.qr_auth_hash.take() else {
            return Ok(());
        };
        let Some(anonym) = self.anonym_token.as_deref() else {
            return Ok(());
        };
        let _: RevokeAuthCodeResponse = self
            .client
            .method(
                "auth.revokeAuthCode",
                params([("auth_hash", auth_hash)]),
                Some(anonym),
            )
            .await?;
        Ok(())
    }

    pub async fn begin(&mut self, login: &str) -> Result<LoginContinuation> {
        self.reset();
        self.login = Some(login.into());
        let anonym: AnonymTokenResponse = self
            .client
            .oauth("get_anonym_token", Parameters::new(), None)
            .await?;
        self.anonym_token = Some(anonym.token.clone());
        let validation: ValidateAccountResponse = self
            .client
            .method(
                "auth.validateAccount",
                params([
                    ("login", login.into()),
                    ("force_password", "0".into()),
                    ("passkey_supported", "0".into()),
                    (
                        "supported_ways",
                        "callreset,codegen,email,reserve_code,password,push,sms".into(),
                    ),
                    ("flow_type", "auth_without_password".into()),
                    ("sak_version", "1.130".into()),
                ]),
                Some(&anonym.token),
            )
            .await?;
        self.validate_sid = Some(validation.sid);
        self.verification_method = validation.next_step.and_then(|s| s.verification_method);
        if self.verification_method.is_none() {
            return Ok(LoginContinuation::Password);
        }
        Ok(LoginContinuation::Otp(self.resend_otp().await?))
    }

    pub async fn resend_otp(&self) -> Result<OtpChallenge> {
        let (Some(anonym), Some(sid)) = (&self.anonym_token, &self.validate_sid) else {
            return Err(Error::InvalidState(
                "Validate the account before requesting a code".into(),
            ));
        };
        let method = self.verification_method.as_deref();
        if matches!(method, Some("codegen" | "reserve_code")) {
            return Ok(OtpChallenge {
                method: OtpMethod::from_channel(method),
                code_length: 6,
                masked_destination: None,
            });
        }
        let endpoint = match method {
            Some("sms") => "ecosystem.sendOtpSms",
            Some("push") => "ecosystem.sendOtpPush",
            Some("email") => "ecosystem.sendOtpEmail",
            _ => "ecosystem.sendOtpCallReset",
        };
        let response: SendOtpResponse = self
            .client
            .method(endpoint, params([("sid", sid.clone())]), Some(anonym))
            .await?;
        Ok(OtpChallenge {
            method: OtpMethod::from_channel(method),
            code_length: response.code_length.unwrap_or(6),
            masked_destination: response.info,
        })
    }

    pub async fn submit_otp(&mut self, code: &str) -> Result<bool> {
        let (Some(anonym), Some(sid)) = (&self.anonym_token, &self.validate_sid) else {
            return Err(Error::InvalidState(
                "Validate the account before submitting a code".into(),
            ));
        };
        let response: CheckOtpResponse = self
            .client
            .method(
                "ecosystem.checkOtp",
                params([
                    ("sid", sid.clone()),
                    ("code", code.into()),
                    (
                        "verification_method",
                        self.verification_method.as_deref().unwrap_or("sms").into(),
                    ),
                ]),
                Some(anonym),
            )
            .await?;
        self.confirmation_sid = Some(response.sid);
        self.profile_preview = response.profile;
        Ok(!response.can_skip_password.unwrap_or(false))
    }

    pub async fn submit_password(&mut self, password: &str) -> Result<Session> {
        let (Some(anonym), Some(sid), Some(login)) = (
            &self.anonym_token,
            self.confirmation_sid
                .as_ref()
                .or(self.validate_sid.as_ref()),
            &self.login,
        ) else {
            return Err(Error::InvalidState(
                "Validate the account before submitting a password".into(),
            ));
        };
        let token: TokenResponse = self
            .client
            .oauth(
                "token",
                params([
                    ("grant_type", "phone_confirmation_sid".into()),
                    ("username", login.clone()),
                    ("password", password.into()),
                    ("sid", sid.clone()),
                    ("anonymous_token", anonym.clone()),
                    ("scope", "all".into()),
                    ("libverify_support", "0".into()),
                    ("2fa_supported", "1".into()),
                ]),
                None,
            )
            .await?;
        self.build_session(
            token.access_token,
            Some(token.user_id),
            token.expires_in,
            token.trusted_hash,
        )
        .await
    }

    async fn build_session(
        &mut self,
        access_token: String,
        user_id: Option<i64>,
        expires_in: Option<i64>,
        trusted_hash: Option<String>,
    ) -> Result<Session> {
        let exchange: Result<ExchangeTokenResponse> = self
            .client
            .method(
                "auth.getExchangeToken",
                params([
                    ("create_common_token", "1".into()),
                    ("create_tier_tokens", "0".into()),
                ]),
                Some(&access_token),
            )
            .await;
        let common_token = exchange
            .ok()
            .and_then(|r| r.users_exchange_tokens.into_iter().next()?.common_token);
        let user: Option<VkUser> = self
            .client
            .method(
                "account.getProfileInfo",
                Parameters::new(),
                Some(&access_token),
            )
            .await
            .ok();
        let mut resolved_user_id = user_id.filter(|&id| id > 0);
        if resolved_user_id.is_none()
            && let Some(profile) = &user
            && profile.id > 0
        {
            resolved_user_id = Some(profile.id);
        }
        if resolved_user_id.is_none()
            && let Ok(users) = self
                .client
                .method::<Vec<VkUser>>("users.get", Parameters::new(), Some(&access_token))
                .await
            && let Some(first) = users.into_iter().next()
            && first.id > 0
        {
            resolved_user_id = Some(first.id);
        }
        let user_id = resolved_user_id.ok_or_else(|| {
            Error::InvalidState("Failed to resolve VK user ID after authorization".into())
        })?;
        let session = Session {
            user_id,
            access_token,
            common_token,
            trusted_hash,
            expires_at: normalize_expires_at(expires_in),
            user,
        };
        self.reset();
        Ok(session)
    }
}

pub fn normalize_phone(raw: &str) -> String {
    let trimmed = raw.trim();
    let had_plus = trimmed.starts_with('+');
    let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
    if !had_plus && digits.len() == 11 && digits.starts_with('8') {
        format!("7{}", &digits[1..])
    } else {
        digits
    }
}

pub fn filename_artist_title(stem: &str) -> (String, String) {
    for sep in [" - ", " — ", " – "] {
        if let Some((artist, title)) = stem.split_once(sep) {
            let artist = artist.trim();
            let title = title.trim();
            if !artist.is_empty() && !title.is_empty() {
                return (artist.to_owned(), title.to_owned());
            }
        }
    }
    (String::new(), stem.trim().to_owned())
}

pub async fn refresh_tokens(
    client: &VkClient,
    common_token: &str,
) -> Result<RefreshTokensResponse> {
    client
        .method(
            "auth.refreshTokens",
            params([
                ("exchange_tokens", common_token.into()),
                ("active_index", "0".into()),
                ("scope", "all".into()),
                ("initiator", "expired_token".into()),
            ]),
            None,
        )
        .await
}
