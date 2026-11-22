use std::collections::BTreeMap;

use url::Url;

use crate::{Error, Result};

pub const API_VERSION: &str = "5.199";
pub const AUDIO_API_VERSION: &str = "5.272";
pub const ANDROID_CLIENT_SECRET: &str = "hHbZxrka2uZ6jB1inYsH";
pub const USER_AGENT: &str =
    "VKAndroidApp/8.116-28825 (Android 16; SDK 36; arm64-v8a; google Pixel 9; ru; 2560x1440)";
pub const CATALOG_USER_AGENT: &str =
    "VKAndroidApp/8.183-54468 (Android 16; SDK 36; arm64-v8a; google Pixel 9; ru; 2560x1440)";

#[derive(Clone)]
pub struct VkConfig {
    pub api_host: Url,
    pub api_version: String,
    pub language: String,
    pub app_id: String,
    pub client_id: String,
    pub client_secret: String,
    pub user_agent: String,
    pub catalog_user_agent: String,
}

impl VkConfig {
    pub fn default_android() -> Result<Self> {
        Self::android(ANDROID_CLIENT_SECRET)
    }

    pub fn android(client_secret: impl Into<String>) -> Result<Self> {
        Ok(Self {
            api_host: Url::parse("https://api.vk.ru")?,
            api_version: API_VERSION.into(),
            language: "ru".into(),
            app_id: "2274003".into(),
            client_id: "2274003".into(),
            client_secret: client_secret.into(),
            user_agent: USER_AGENT.into(),
            catalog_user_agent: CATALOG_USER_AGENT.into(),
        })
    }

    pub(crate) fn parameters(&self) -> BTreeMap<String, String> {
        [
            ("v", self.api_version.as_str()),
            ("https", "1"),
            ("lang", self.language.as_str()),
            ("api_id", self.app_id.as_str()),
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.to_owned()))
        .collect()
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.client_secret.is_empty() {
            return Err(Error::InvalidState("VK client secret is required".into()));
        }
        Ok(())
    }
}
