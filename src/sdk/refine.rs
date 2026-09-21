//! Optional rewrite, like Gmail's refine. A separate model from the judge.
//!
//! Point `ECCLESIA_LLM_URL` at Ollama or llama.cpp on the machine with the GPU.
//! Tests and a box with no model use `silent`, which returns the same words.

use std::sync::Arc;

use serde_json::json;

use crate::leaf::VoiceKind;

use super::judge::{chat_completions_url, ChatResponse};

#[derive(Clone)]
pub struct RefineHub {
    inner: Arc<RefineInner>,
}

enum RefineInner {
    Silent,
    Chat {
        url: String,
        key: Option<String>,
        model: String,
        http: reqwest::Client,
    },
}

impl RefineHub {
    pub fn silent() -> Self {
        Self {
            inner: Arc::new(RefineInner::Silent),
        }
    }

    pub fn load() -> Self {
        match from_chat_env() {
            Some(hub) => hub,
            None => Self::silent(),
        }
    }

    pub fn is_live(&self) -> bool {
        matches!(self.inner.as_ref(), RefineInner::Chat { .. })
    }

    pub async fn rewrite(&self, kind: VoiceKind, text: &str) -> anyhow::Result<String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(String::new());
        }
        match self.inner.as_ref() {
            RefineInner::Silent => Ok(trimmed.to_string()),
            RefineInner::Chat {
                url,
                key,
                model,
                http,
            } => chat_rewrite(http, url, key.as_deref(), model, kind, trimmed).await,
        }
    }
}

fn from_chat_env() -> Option<RefineHub> {
    let url = std::env::var("ECCLESIA_LLM_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())?;
    let model = refine_model();
    let key = std::env::var("ECCLESIA_LLM_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty());
    tracing::info!("rewriting with the local chat model {model} at {url}");
    Some(RefineHub {
        inner: Arc::new(RefineInner::Chat {
            url,
            key,
            model,
            http: http_client(),
        }),
    })
}

fn refine_model() -> String {
    std::env::var("ECCLESIA_REFINE_MODEL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("ECCLESIA_LLM_MODEL")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "llama3.1:8b".into())
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn rewrite_preamble(kind: VoiceKind) -> &'static str {
    match kind {
        VoiceKind::Need => {
            "Rewrite this church need so it is clear and kind. Keep the facts they named. Return only the rewritten text."
        }
        VoiceKind::Offer => {
            "Rewrite this offer to help so it is clear and kind. Keep when they can come and what they can do. Return only the rewritten text."
        }
        VoiceKind::Endorsement => {
            "Rewrite this endorsement so it is clear and heartfelt. Keep the specific thing they saw. Return only the rewritten text."
        }
        VoiceKind::GiftNote => {
            "Rewrite this gift note so it is clear. Keep how they serve. Return only the rewritten text."
        }
        VoiceKind::Bio => {
            "Rewrite this short bio so it is clear. Keep how they named themselves. Return only the rewritten text."
        }
        VoiceKind::Church => {
            "Rewrite this church description so it is clear. Keep where they are and who comes. Return only the rewritten text."
        }
    }
}

async fn chat_rewrite(
    http: &reqwest::Client,
    base: &str,
    key: Option<&str>,
    model: &str,
    kind: VoiceKind,
    text: &str,
) -> anyhow::Result<String> {
    let url = chat_completions_url(base);
    let mut request = http.post(&url).json(&json!({
        "model": model,
        "temperature": 0.4,
        "messages": [
            { "role": "system", "content": rewrite_preamble(kind) },
            { "role": "user", "content": text }
        ]
    }));
    if let Some(key) = key {
        request = request.bearer_auth(key);
    }
    let response = request.send().await?.error_for_status()?;
    let parsed: ChatResponse = response.json().await?;
    let rewritten = parsed.first_text().trim();
    if rewritten.is_empty() {
        return Ok(text.to_string());
    }
    Ok(rewritten.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn us_refine_01_silent_returns_the_same_words() {
        let hub = RefineHub::silent();
        let text = hub
            .rewrite(VoiceKind::Endorsement, "  She stayed.  ")
            .await
            .unwrap();
        assert_eq!(text, "She stayed.");
        assert!(!hub.is_live());
    }
}
