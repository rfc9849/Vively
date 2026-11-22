use std::{
    collections::BTreeMap,
    sync::{Arc, Weak},
    time::Duration,
};

use async_trait::async_trait;
use reqwest::{
    Client,
    header::{HeaderMap, HeaderName, HeaderValue},
    multipart,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::RwLock;
use url::Url;

use crate::{
    Error, Result,
    config::VkConfig,
    error::{ApiError, CaptchaChallenge, OAuthError},
};

pub type Parameters = BTreeMap<String, String>;

#[async_trait]
pub trait CaptchaResolver: Send + Sync {
    async fn resolve(&self, challenge: &CaptchaChallenge) -> Result<Option<String>>;
}

#[async_trait]
pub trait TokenRefresher: Send + Sync {
    async fn refreshed_token(&self, expired: &str) -> Result<Option<String>>;
}

#[derive(Clone)]
pub struct VkClient(Arc<ClientInner>);

struct ClientInner {
    http: Client,
    config: VkConfig,
    device_id: String,
    captcha: RwLock<Option<Arc<dyn CaptchaResolver>>>,
    refresher: RwLock<Option<Arc<dyn TokenRefresher>>>,
}

pub(crate) struct WeakVkClient(Weak<ClientInner>);

impl WeakVkClient {
    pub(crate) fn upgrade(&self) -> Option<VkClient> {
        self.0.upgrade().map(VkClient)
    }
}

#[derive(Clone, Copy)]
pub enum ResponseStyle {
    Method,
    OAuth,
}

impl VkClient {
    pub fn new(config: VkConfig, device_id: impl Into<String>) -> Result<Self> {
        config.validate()?;
        let http = Client::builder()
            .cookie_store(true)
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self(Arc::new(ClientInner {
            http,
            config,
            device_id: device_id.into(),
            captcha: RwLock::new(None),
            refresher: RwLock::new(None),
        })))
    }

    pub fn config(&self) -> &VkConfig {
        &self.0.config
    }
    pub fn http(&self) -> &Client {
        &self.0.http
    }

    pub async fn set_captcha_resolver(&self, resolver: Option<Arc<dyn CaptchaResolver>>) {
        *self.0.captcha.write().await = resolver;
    }

    pub async fn set_token_refresher(&self, refresher: Option<Arc<dyn TokenRefresher>>) {
        *self.0.refresher.write().await = refresher;
    }

    pub(crate) fn downgrade(&self) -> WeakVkClient {
        WeakVkClient(Arc::downgrade(&self.0))
    }

    pub async fn method<T: DeserializeOwned>(
        &self,
        name: &str,
        parameters: Parameters,
        token: Option<&str>,
    ) -> Result<T> {
        self.method_with_headers(name, parameters, HeaderMap::new(), token)
            .await
    }

    pub async fn method_with_headers<T: DeserializeOwned>(
        &self,
        name: &str,
        parameters: Parameters,
        headers: HeaderMap,
        token: Option<&str>,
    ) -> Result<T> {
        validate_name(name)?;
        self.send(
            &format!("/method/{name}"),
            parameters,
            headers,
            token,
            ResponseStyle::Method,
        )
        .await
    }

    pub async fn oauth<T: DeserializeOwned>(
        &self,
        name: &str,
        parameters: Parameters,
        token: Option<&str>,
    ) -> Result<T> {
        validate_name(name)?;
        self.send(
            &format!("/oauth/{name}"),
            parameters,
            HeaderMap::new(),
            token,
            ResponseStyle::OAuth,
        )
        .await
    }

    pub fn request_parameters(&self, parameters: &Parameters, token: Option<&str>) -> Parameters {
        let mut merged = self.0.config.parameters();
        merged.insert("device_id".into(), self.0.device_id.clone());
        merged.extend(parameters.clone());
        if let Some(token) = token {
            merged.insert("access_token".into(), token.into());
        }
        merged
    }

    fn headers(&self, overrides: HeaderMap) -> Result<HeaderMap> {
        let config = &self.0.config;
        let mut headers = HeaderMap::new();
        for (name, value) in [
            ("content-type", "application/x-www-form-urlencoded"),
            ("user-agent", config.user_agent.as_str()),
            ("x-vk-android-client", "new"),
            ("accept-language", config.language.as_str()),
            (
                "accept",
                "image/gif, image/x-xbitmap, image/jpeg, image/pjpeg, */*",
            ),
        ] {
            headers.insert(
                HeaderName::from_static(name),
                HeaderValue::from_str(value)
                    .map_err(|_| Error::InvalidState(format!("Invalid {name} header")))?,
            );
        }
        headers.extend(overrides);
        Ok(headers)
    }

    async fn send<T: DeserializeOwned>(
        &self,
        path: &str,
        mut parameters: Parameters,
        headers: HeaderMap,
        token: Option<&str>,
        style: ResponseStyle,
    ) -> Result<T> {
        let mut token = token.map(str::to_owned);
        let mut refreshed = false;
        let mut captcha_attempts = 0;
        loop {
            let response = self
                .0
                .http
                .post(self.0.config.api_host.join(path)?)
                .headers(self.headers(headers.clone())?)
                .body(form_encode(
                    &self.request_parameters(&parameters, token.as_deref()),
                ))
                .send()
                .await?;
            let status = response.status();
            let bytes = response.bytes().await?;
            let result = decode_response::<T>(&bytes, style);
            let error = match result {
                Ok(value) if status.is_success() => return Ok(value),
                Ok(_) => return Err(Error::Http(status.as_u16())),
                Err(Error::Decode { .. }) if !status.is_success() => {
                    return Err(Error::Http(status.as_u16()));
                }
                Err(error) => error,
            };
            if let Error::Api(api) = &error {
                let refresher = self.0.refresher.read().await.clone();
                if !refreshed
                    && api.is_refreshable()
                    && let (Some(expired), Some(refresher)) = (token.as_deref(), refresher)
                    && let Some(new) = refresher.refreshed_token(expired).await?
                    && new != expired
                {
                    token = Some(new);
                    refreshed = true;
                    continue;
                }
            }
            if let Error::Captcha(challenge) = &error {
                let resolver = self.0.captcha.read().await.clone();
                if captcha_attempts < 3
                    && challenge.has_web_challenge()
                    && let Some(resolver) = resolver
                    && let Some(success) = resolver.resolve(challenge).await?
                {
                    parameters.insert("captcha_sid".into(), challenge.sid.clone());
                    parameters.insert("success_token".into(), success);
                    if let Some(v) = challenge.attempt {
                        parameters.insert("captcha_attempt".into(), v.to_string());
                    }
                    if let Some(v) = challenge.ts {
                        parameters.insert("captcha_ts".into(), format!("{v:.6}"));
                    }
                    if let Some(v) = challenge.is_sound_available {
                        parameters
                            .insert("is_sound_captcha".into(), if v { "1" } else { "0" }.into());
                    }
                    captcha_attempts += 1;
                    continue;
                }
            }
            return Err(error);
        }
    }

    pub async fn upload(
        &self,
        url: Url,
        bytes: Vec<u8>,
        filename: &str,
        mime: &str,
    ) -> Result<String> {
        let part = multipart::Part::bytes(bytes)
            .file_name(filename.to_owned())
            .mime_str(mime)?;
        let response = self
            .0
            .http
            .post(url)
            .header("user-agent", &self.0.config.user_agent)
            .timeout(Duration::from_secs(300))
            .multipart(multipart::Form::new().part("file", part))
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(Error::Http(response.status().as_u16()));
        }
        Ok(response.text().await?)
    }
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_'))
    {
        return Err(Error::InvalidState("Invalid VK endpoint name".into()));
    }
    Ok(())
}

pub fn form_encode(parameters: &Parameters) -> String {
    fn encode(s: &str) -> String {
        let mut out = String::new();
        for b in s.bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        out
    }
    parameters
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

pub fn decode_response<T: DeserializeOwned>(bytes: &[u8], style: ResponseStyle) -> Result<T> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|e| Error::decoding("VK response", e))?;
    match style {
        ResponseStyle::Method => {
            if let Some(error) = value.get("error").filter(|v| !v.is_null()) {
                let error: ApiError = serde_json::from_value(error.clone())
                    .map_err(|e| Error::decoding("VK error", e))?;
                if let Some(challenge) = error.captcha_challenge() {
                    return Err(Error::Captcha(Box::new(challenge)));
                }
                return Err(Error::Api(Box::new(error)));
            }
            let response = value.get("response").ok_or_else(|| {
                Error::decoding(
                    "VK response",
                    <serde_json::Error as serde::de::Error>::custom("Response field is missing"),
                )
            })?;
            let execute_error = value
                .get("execute_errors")
                .and_then(Value::as_array)
                .and_then(|errors| errors.first())
                .and_then(|error| serde_json::from_value::<ApiError>(error.clone()).ok());
            if let Some(error) = &execute_error
                && matches!(response, Value::Null | Value::Bool(false))
            {
                return Err(Error::Api(Box::new(error.clone())));
            }
            serde_json::from_value(response.clone()).map_err(|e| match execute_error {
                Some(error) => Error::Api(Box::new(error)),
                None => Error::decoding("VK response", e),
            })
        }
        ResponseStyle::OAuth => {
            if value.get("error").is_some_and(Value::is_string) {
                let error: OAuthError =
                    serde_json::from_value(value).map_err(|e| Error::decoding("OAuth error", e))?;
                if let Some(challenge) = error.captcha_challenge() {
                    return Err(Error::Captcha(Box::new(challenge)));
                }
                if let Some(validation) = error.validation() {
                    return Err(Error::Validation(validation));
                }
                return Err(Error::OAuth(Box::new(error)));
            }
            serde_json::from_value(value).map_err(|e| Error::decoding("OAuth response", e))
        }
    }
}

pub fn params<const N: usize>(pairs: [(&str, String); N]) -> Parameters {
    pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}
