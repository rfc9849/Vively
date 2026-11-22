use serde::{Deserialize, Serialize};
use url::Url;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP transport failed")]
    Transport(#[source] reqwest::Error),
    #[error("HTTP {0}")]
    Http(u16),
    #[error("Could not decode {context}")]
    Decode {
        context: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("{0}")]
    Api(Box<ApiError>),
    #[error("{0}")]
    OAuth(Box<OAuthError>),
    #[error("Verification is required")]
    Validation(Validation),
    #[error("Captcha is required")]
    Captcha(Box<CaptchaChallenge>),
    #[error("{0}")]
    InvalidState(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
    #[error("Invalid URL")]
    Url(#[from] url::ParseError),
    #[error("{0}")]
    Media(String),
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self::Transport(error.without_url())
    }
}

impl Error {
    pub(crate) fn decoding(context: impl Into<String>, source: serde_json::Error) -> Self {
        Self::Decode {
            context: context.into(),
            source,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptchaChallenge {
    pub sid: String,
    pub redirect_url: Option<Url>,
    pub image_url: Option<Url>,
    pub attempt: Option<i64>,
    pub ts: Option<f64>,
    pub is_sound_available: Option<bool>,
}

impl CaptchaChallenge {
    pub fn has_web_challenge(&self) -> bool {
        self.redirect_url.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ApiError {
    #[serde(rename = "error_code")]
    pub code: i64,
    #[serde(rename = "error_msg")]
    pub message: String,
    pub captcha_sid: Option<String>,
    pub captcha_img: Option<String>,
    pub redirect_uri: Option<String>,
    pub captcha_ts: Option<f64>,
    pub captcha_attempt: Option<i64>,
    pub is_sound_captcha_available: Option<bool>,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VK error {}: {}", self.code, self.message)
    }
}

impl ApiError {
    pub fn is_refreshable(&self) -> bool {
        matches!(self.code, 5 | 1117)
    }

    pub fn captcha_challenge(&self) -> Option<CaptchaChallenge> {
        if self.code != 14 {
            return None;
        }
        Some(CaptchaChallenge {
            sid: self.captcha_sid.clone()?,
            redirect_url: self
                .redirect_uri
                .as_deref()
                .and_then(|v| Url::parse(v).ok()),
            image_url: self.captcha_img.as_deref().and_then(|v| Url::parse(v).ok()),
            attempt: self.captcha_attempt,
            ts: self.captcha_ts,
            is_sound_available: self.is_sound_captcha_available,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OAuthError {
    pub error: String,
    #[serde(rename = "error_description")]
    pub description: Option<String>,
    pub validation_type: Option<String>,
    pub validation_sid: Option<String>,
    pub phone_mask: Option<String>,
    pub captcha_sid: Option<String>,
    pub captcha_img: Option<String>,
    pub redirect_uri: Option<String>,
}

impl std::fmt::Display for OAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.description.as_deref().unwrap_or(&self.error))
    }
}

impl OAuthError {
    pub fn captcha_challenge(&self) -> Option<CaptchaChallenge> {
        Some(CaptchaChallenge {
            sid: self.captcha_sid.clone()?,
            redirect_url: self
                .redirect_uri
                .as_deref()
                .and_then(|v| Url::parse(v).ok()),
            image_url: self.captcha_img.as_deref().and_then(|v| Url::parse(v).ok()),
            attempt: None,
            ts: None,
            is_sound_available: None,
        })
    }

    pub fn validation(&self) -> Option<Validation> {
        if self.error != "need_validation" {
            return None;
        }
        let kind = match self.validation_type.as_deref()? {
            "2fa_sms" => ValidationKind::TwoFactorSms,
            "2fa_app" => ValidationKind::TwoFactorApp,
            "captcha" => ValidationKind::Captcha,
            other => ValidationKind::Other(other.into()),
        };
        Some(Validation {
            kind,
            sid: self.validation_sid.clone(),
            phone_mask: self.phone_mask.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Validation {
    pub kind: ValidationKind,
    pub sid: Option<String>,
    pub phone_mask: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationKind {
    TwoFactorSms,
    TwoFactorApp,
    Captcha,
    Other(String),
}
