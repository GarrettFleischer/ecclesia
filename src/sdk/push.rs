//! Push skin. Leaves emit notices; this file delivers them to a phone.

use std::sync::Arc;

use crate::db::{Db, PushDevice, PushSubscription};
use crate::leaf::NoticeDraft;

use super::web_push;

const DEV_VAPID_PEM: &str = "-----BEGIN EC PRIVATE KEY-----
MHcCAQEEIPZV5ms+9lrG/t3p8EXrHFKffRogwU8SYlDfGf23AFV0oAoGCCqGSM49
AwEHoUQDQgAETfITf09T65xXoc0eyNhQXYRdIaH7AZQzW4wbvJ5oe456rDQHgLy2
kWtVDCNi2zgMdsVAHl4Q3f7K4/cvzQvWTw==
-----END EC PRIVATE KEY-----
";

#[derive(Clone)]
pub struct PushHub {
    pub public_key: String,
    pem: Arc<str>,
    fcm_key: Option<Arc<str>>,
    http: reqwest::Client,
}

impl PushHub {
    pub fn silent() -> Self {
        Self {
            public_key: String::new(),
            pem: Arc::from(""),
            fcm_key: None,
            http: reqwest::Client::new(),
        }
    }

    pub fn load() -> Self {
        if let Some(hub) = from_env() {
            return hub;
        }
        tracing::warn!(
            "using the local development VAPID key; set ECCLESIA_VAPID_PEM for a shared host"
        );
        match hub_from_pem(DEV_VAPID_PEM, fcm_from_env()) {
            Ok(hub) => hub,
            Err(error) => {
                tracing::error!("development VAPID key failed to load: {error:#}");
                Self::silent()
            }
        }
    }

    pub async fn dispatch(&self, db: &Db, notices: &[NoticeDraft]) {
        if self.pem.is_empty() && self.fcm_key.is_none() {
            return;
        }
        for notice in notices {
            send_notice(self, db, notice).await;
        }
    }
}

fn from_env() -> Option<PushHub> {
    let pem = std::env::var("ECCLESIA_VAPID_PEM").ok()?;
    let pem = read_pem(&pem);
    if pem.is_empty() {
        return None;
    }
    match hub_from_pem(&pem, fcm_from_env()) {
        Ok(hub) => Some(hub),
        Err(error) => {
            tracing::error!("ECCLESIA_VAPID_PEM is not a usable P-256 key: {error:#}");
            None
        }
    }
}

fn fcm_from_env() -> Option<Arc<str>> {
    std::env::var("ECCLESIA_FCM_KEY")
        .ok()
        .filter(|key| !key.is_empty())
        .map(Arc::from)
}

fn hub_from_pem(pem: &str, fcm_key: Option<Arc<str>>) -> anyhow::Result<PushHub> {
    let public_key = match std::env::var("ECCLESIA_VAPID_PUBLIC") {
        Ok(value) if !value.is_empty() => value,
        _ => web_push::public_from_pem(pem)?,
    };
    Ok(PushHub {
        public_key,
        pem: Arc::from(pem),
        fcm_key,
        http: reqwest::Client::new(),
    })
}

fn read_pem(value: &str) -> String {
    let path = std::path::Path::new(value);
    if path.exists() {
        std::fs::read_to_string(path).unwrap_or_default()
    } else {
        value.replace("\\n", "\n")
    }
}

async fn send_notice(hub: &PushHub, db: &Db, notice: &NoticeDraft) {
    let payload = serde_json::json!({
        "title": notice.title.as_ref(),
        "body": notice.body,
        "url": notice.href,
    });
    let body = payload.to_string();
    if !hub.pem.is_empty() {
        send_web_list(hub, db, &notice.user_id, body.as_bytes()).await;
    }
    if let Some(key) = hub.fcm_key.as_deref() {
        send_fcm_list(hub, db, &notice.user_id, key, notice).await;
    }
}

async fn send_web_list(hub: &PushHub, db: &Db, user_id: &str, payload: &[u8]) {
    let Ok(subs) = db.push_subscriptions(user_id).await else {
        return;
    };
    for sub in subs {
        match send_web(hub, &sub, payload).await {
            Ok(()) => {}
            Err(error) => {
                tracing::warn!("web push failed for {}: {error:#}", sub.endpoint);
                forget_if_gone(db, &sub.endpoint, &error.to_string()).await;
            }
        }
    }
}

async fn send_fcm_list(hub: &PushHub, db: &Db, user_id: &str, key: &str, notice: &NoticeDraft) {
    let Ok(devices) = db.push_devices(user_id).await else {
        return;
    };
    for device in devices {
        if let Err(error) = send_fcm(hub, key, &device, notice).await {
            tracing::warn!("fcm failed for {}: {error:#}", device.token);
        }
    }
}

async fn forget_if_gone(db: &Db, endpoint: &str, error: &str) {
    if error.contains("410") || error.contains("404") {
        let _ = db.forget_push_subscription(endpoint).await;
    }
}

async fn send_web(hub: &PushHub, sub: &PushSubscription, payload: &[u8]) -> anyhow::Result<()> {
    let encrypted = web_push::encrypt(&sub.p256dh, &sub.auth, payload)?;
    let audience = web_push::audience(&sub.endpoint)?;
    let exp = now_unix().saturating_add(12 * 3600);
    let jwt = web_push::vapid_jwt(hub.pem.as_ref(), &audience, exp)?;
    let authorization = format!("vapid t={jwt}, k={}", hub.public_key);
    let response = hub
        .http
        .post(&sub.endpoint)
        .header("authorization", authorization)
        .header("ttl", "86400")
        .header("urgency", "high")
        .header("content-encoding", "aes128gcm")
        .header("content-type", "application/octet-stream")
        .body(encrypted)
        .send()
        .await?;
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    let text = response.text().await.unwrap_or_default();
    Err(anyhow::anyhow!("{status} {text}"))
}

async fn send_fcm(
    hub: &PushHub,
    key: &str,
    device: &PushDevice,
    notice: &NoticeDraft,
) -> anyhow::Result<()> {
    let body = serde_json::json!({
        "to": device.token,
        "notification": {
            "title": notice.title.as_ref(),
            "body": notice.body,
        },
        "data": {
            "url": notice.href,
        },
        "content_available": true,
    });
    let response = hub
        .http
        .post("https://fcm.googleapis.com/fcm/send")
        .header("authorization", format!("key={key}"))
        .json(&body)
        .send()
        .await?;
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    let text = response.text().await.unwrap_or_default();
    Err(anyhow::anyhow!("{status} {text}"))
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
