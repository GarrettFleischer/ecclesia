//! Rewrite before publish. OpenRouter's free router is the default hosted path.
//! A local OpenAI-compatible server is for a box with a GPU.
//! Tests use `silent` (echo) or `polish` (collapse spaces, marked live).

use std::sync::Arc;

use serde_json::json;

use ecclesia_domain::VoiceKind;

use super::chat::{
    ChatEndpoint, ChatResponse, apply_headers, chat_completions_url,
};
use super::prompts;

#[derive(Clone)]
pub struct RefineHub {
    inner: Arc<RefineInner>,
}

#[allow(dead_code)]
enum RefineInner {
    Silent,
    Polish,
    Chat {
        endpoint: ChatEndpoint,
        http: reqwest::Client,
    },
}

impl RefineHub {
    pub fn silent() -> Self {
        Self {
            inner: Arc::new(RefineInner::Silent),
        }
    }

    pub fn polish() -> Self {
        Self {
            inner: Arc::new(RefineInner::Polish),
        }
    }

    pub fn load() -> Self {
        Self::silent()
    }

    pub fn is_live(&self) -> bool {
        !matches!(self.inner.as_ref(), RefineInner::Silent)
    }

    pub async fn rewrite(&self, kind: VoiceKind, text: &str) -> anyhow::Result<String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(String::new());
        }
        match self.inner.as_ref() {
            RefineInner::Silent => Ok(trimmed.to_string()),
            RefineInner::Polish => Ok(collapse_spaces(trimmed)),
            RefineInner::Chat { endpoint, http } => {
                chat_rewrite(http, endpoint, kind, trimmed).await
            }
        }
    }
}

fn collapse_spaces(text: &str) -> String {
    let mut out = String::new();
    let mut pending_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(ch);
    }
    out
}

async fn chat_rewrite(
    http: &reqwest::Client,
    endpoint: &ChatEndpoint,
    kind: VoiceKind,
    text: &str,
) -> anyhow::Result<String> {
    let url = chat_completions_url(&endpoint.url);
    let body = json!({
        "model": endpoint.model,
        "temperature": 0.4,
        "messages": [
            { "role": "system", "content": prompts::refine_system(kind) },
            { "role": "user", "content": text }
        ]
    });
    let request = apply_headers(http.post(&url).json(&body), endpoint);
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

    #[tokio::test]
    async fn us_refine_02_polish_collapses_spaces() {
        let hub = RefineHub::polish();
        let text = hub
            .rewrite(VoiceKind::Need, "Need   five   dinners")
            .await
            .unwrap();
        assert_eq!(text, "Need five dinners");
        assert!(hub.is_live());
    }
}
