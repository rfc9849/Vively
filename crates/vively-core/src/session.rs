use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    Error, Result, VkClient,
    auth::{RefreshTokensResponse, refresh_tokens},
    client::{TokenRefresher, WeakVkClient},
    models::VkUser,
};

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(i64::MAX as u64) as i64
}

const APPLE_REFERENCE_EPOCH_OFFSET: i64 = 978_307_200;

fn flexible_expires_at<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<i64>, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    let Some(num) = value.as_number() else {
        return Ok(None);
    };
    if let Some(i) = num.as_i64() {
        if (500_000_000..1_200_000_000).contains(&i) {
            return Ok(Some(i.saturating_add(APPLE_REFERENCE_EPOCH_OFFSET)));
        }
        return Ok(Some(i));
    }
    if let Some(f) = num.as_f64()
        && f.is_finite()
    {
        if (500_000_000.0..1_200_000_000.0).contains(&f) {
            return Ok(Some((f + APPLE_REFERENCE_EPOCH_OFFSET as f64).round() as i64));
        }
        return Ok(Some(f.round() as i64));
    }
    Ok(None)
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    #[serde(alias = "userID")]
    pub user_id: i64,
    #[serde(alias = "accessToken")]
    pub access_token: String,
    #[serde(default, alias = "commonToken")]
    pub common_token: Option<String>,
    #[serde(default, alias = "trustedHash")]
    pub trusted_hash: Option<String>,
    #[serde(default, alias = "expiresAt", deserialize_with = "flexible_expires_at")]
    pub expires_at: Option<i64>,
    #[serde(default, deserialize_with = "crate::models::tolerant")]
    pub user: Option<VkUser>,
}

impl Session {
    pub fn is_expired(&self) -> bool {
        self.is_expired_at(unix_now())
    }
    pub fn is_expired_at(&self, now: i64) -> bool {
        self.expires_at.is_some_and(|v| v <= now)
    }
}

pub trait SessionPersistence: Send + Sync {
    fn load(&self) -> Result<Option<Session>>;
    fn save(&self, session: &Session) -> Result<()>;
    fn delete(&self) -> Result<()>;
}

pub struct SessionStore {
    session: Mutex<Option<Session>>,
    persistence: Arc<dyn SessionPersistence>,
}

impl SessionStore {
    pub fn new(persistence: Arc<dyn SessionPersistence>) -> Result<Self> {
        let session = persistence.load()?;
        Ok(Self {
            session: Mutex::new(session),
            persistence,
        })
    }

    pub fn session(&self) -> Result<Option<Session>> {
        Ok(self
            .session
            .lock()
            .map_err(|_| Error::InvalidState("Session lock is poisoned".into()))?
            .clone())
    }

    pub fn adopt(&self, session: Session) -> Result<()> {
        let mut state = self
            .session
            .lock()
            .map_err(|_| Error::InvalidState("Session lock is poisoned".into()))?;
        self.persistence.save(&session)?;
        *state = Some(session);
        Ok(())
    }

    pub fn update_user(&self, user: VkUser) -> Result<()> {
        self.update(|s| s.user = Some(user))
    }

    pub fn apply_refreshed_token(&self, token: String, expires_in: Option<i64>) -> Result<()> {
        self.update(|s| {
            s.access_token = token;
            s.expires_at = expires_in.map(|v| unix_now().saturating_add(v));
        })
    }

    fn finish_refresh(
        &self,
        expired: &str,
        user_id: i64,
        token: String,
        expires_in: Option<i64>,
    ) -> Result<Option<String>> {
        let mut state = self
            .session
            .lock()
            .map_err(|_| Error::InvalidState("Session lock is poisoned".into()))?;
        let Some(current) = state.as_ref().filter(|s| s.user_id == user_id) else {
            return Ok(None);
        };
        if current.access_token != expired {
            return Ok(Some(current.access_token.clone()));
        }
        let mut updated = current.clone();
        updated.access_token = token.clone();
        updated.expires_at = expires_in.map(|v| unix_now().saturating_add(v));
        self.persistence.save(&updated)?;
        *state = Some(updated);
        Ok(Some(token))
    }

    fn invalidate_expired(&self, expired: &str, user_id: i64) -> Result<()> {
        let mut state = self
            .session
            .lock()
            .map_err(|_| Error::InvalidState("Session lock is poisoned".into()))?;
        if state
            .as_ref()
            .is_some_and(|s| s.access_token == expired && s.user_id == user_id)
        {
            self.persistence.delete()?;
            *state = None;
        }
        Ok(())
    }

    fn update(&self, f: impl FnOnce(&mut Session)) -> Result<()> {
        let mut state = self
            .session
            .lock()
            .map_err(|_| Error::InvalidState("Session lock is poisoned".into()))?;
        if let Some(mut session) = state.clone() {
            f(&mut session);
            self.persistence.save(&session)?;
            *state = Some(session);
        }
        Ok(())
    }

    pub fn sign_out(&self) -> Result<()> {
        let mut state = self
            .session
            .lock()
            .map_err(|_| Error::InvalidState("Session lock is poisoned".into()))?;
        self.persistence.delete()?;
        *state = None;
        Ok(())
    }
}

pub struct SessionTokenRefresher {
    client: WeakVkClient,
    store: Arc<SessionStore>,
    in_flight: tokio::sync::Mutex<()>,
}

impl SessionTokenRefresher {
    pub fn new(client: &VkClient, store: Arc<SessionStore>) -> Self {
        Self {
            client: client.downgrade(),
            store,
            in_flight: tokio::sync::Mutex::new(()),
        }
    }
}

#[async_trait]
impl TokenRefresher for SessionTokenRefresher {
    async fn refreshed_token(&self, expired: &str) -> Result<Option<String>> {
        let _guard = self.in_flight.lock().await;
        let Some(session) = self.store.session()? else {
            return Ok(None);
        };
        if session.access_token != expired {
            return Ok(Some(session.access_token));
        }
        let Some(common) = session.common_token else {
            return Ok(None);
        };
        let Some(client) = self.client.upgrade() else {
            return Ok(None);
        };
        let response: RefreshTokensResponse = match refresh_tokens(&client, &common).await {
            Ok(v) => v,
            Err(error) => {
                if matches!(&error, Error::Api(e) if e.is_refreshable())
                    || matches!(&error, Error::OAuth(_))
                {
                    self.store.invalidate_expired(expired, session.user_id)?;
                }
                return Err(error);
            }
        };
        let success = response
            .success
            .into_iter()
            .next()
            .ok_or_else(|| Error::InvalidState("Token refresh returned no token".into()))?;
        let token = success.access_token;
        self.store
            .finish_refresh(expired, session.user_id, token.token, token.expires_in)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceProfile {
    pub device_id: String,
}

impl DeviceProfile {
    pub fn load_or_create(path: &Path) -> Result<Self> {
        match fs::read(path) {
            Ok(data) => {
                serde_json::from_slice(&data).map_err(|e| Error::decoding("device profile", e))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let device_id = rand::rng()
                    .sample_iter(Alphanumeric)
                    .take(16)
                    .map(char::from)
                    .collect();
                let profile = Self { device_id };
                save_json(path, &profile)?;
                Ok(profile)
            }
            Err(e) => Err(e.into()),
        }
    }
}

pub(crate) fn save_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|e| Error::decoding("saved data", e))?;
    atomic_write(path, &bytes)
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".vively-{:032x}.tmp", rand::random::<u128>()));
    let result = (|| -> Result<()> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
