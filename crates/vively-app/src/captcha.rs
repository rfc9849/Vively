use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use async_trait::async_trait;
use futures_channel::{mpsc, oneshot};
use serde_json::Value;
use url::Url;
use vively_core::{client::CaptchaResolver, error::CaptchaChallenge};

pub struct CaptchaPrompt {
    pub id: u64,
    pub challenge: CaptchaChallenge,
    pub reply: oneshot::Sender<Option<String>>,
}

pub struct CaptchaBridge {
    next_id: AtomicU64,
    in_flight: tokio::sync::Mutex<()>,
    tx: mpsc::UnboundedSender<CaptchaPrompt>,
}

impl CaptchaBridge {
    pub fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<CaptchaPrompt>) {
        let (tx, rx) = mpsc::unbounded();
        (
            Arc::new(Self {
                next_id: AtomicU64::new(1),
                in_flight: tokio::sync::Mutex::new(()),
                tx,
            }),
            rx,
        )
    }
}

#[async_trait]
impl CaptchaResolver for CaptchaBridge {
    async fn resolve(&self, challenge: &CaptchaChallenge) -> vively_core::Result<Option<String>> {
        let _guard = self.in_flight.lock().await;
        let (reply_tx, reply_rx) = oneshot::channel();
        let prompt = CaptchaPrompt {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            challenge: challenge.clone(),
            reply: reply_tx,
        };
        if self.tx.unbounded_send(prompt).is_err() {
            return Ok(None);
        }
        Ok(reply_rx.await.ok().flatten())
    }
}

/// JS injected into the captcha webview. It reports every fetch/XHR response
/// body through `window.ipc.postMessage`; Android exposes the same `ipc`
/// object via `addJavascriptInterface`.
pub fn captcha_interceptor_script() -> &'static str {
    r#"(function () {
  function report(text) {
    try {
      if (window.ipc && window.ipc.postMessage) {
        window.ipc.postMessage(String(text));
      }
    } catch (e) {}
  }
  var origFetch = window.fetch;
  if (origFetch) {
    window.fetch = function () {
      return origFetch.apply(this, arguments).then(function (resp) {
        try { resp.clone().text().then(report).catch(function () {}); } catch (e) {}
        return resp;
      });
    };
  }
  var origSend = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.send = function () {
    var xhr = this;
    xhr.addEventListener('load', function () {
      try { report(xhr.responseText); } catch (e) {}
    });
    return origSend.apply(this, arguments);
  };
})();"#
}

pub fn extract_captcha_success_token(text: &str) -> Option<String> {
    if !text.contains("success_token") {
        return None;
    }
    let root: Value = serde_json::from_str(text).ok()?;
    if let Some(tok) = root
        .get("response")
        .and_then(|r| r.get("success_token"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return Some(tok.to_owned());
    }
    root.get("success_token")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptchaNavOutcome {
    Success(Option<String>),
    Cancel,
}

pub fn parse_captcha_nav_outcome(raw_url: &str) -> Option<CaptchaNavOutcome> {
    let parsed = Url::parse(raw_url).ok()?;
    let fragment = parsed.fragment().unwrap_or_default();
    let fragment = fragment
        .split_once('?')
        .map_or(fragment, |(_, query)| query);
    let mut has_cancel = false;
    let mut has_success = false;
    let mut token = None;
    for (k, v) in parsed
        .query_pairs()
        .chain(url::form_urlencoded::parse(fragment.as_bytes()))
    {
        if k == "cancel" {
            has_cancel = true;
        } else if k == "success" {
            has_success = true;
        } else if (k == "success_token" || k == "token") && !v.is_empty() {
            token = Some(v.into_owned());
        }
    }
    if has_cancel {
        Some(CaptchaNavOutcome::Cancel)
    } else if has_success {
        Some(CaptchaNavOutcome::Success(token))
    } else {
        None
    }
}
