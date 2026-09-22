//! OpenAI-compatible chat. OpenRouter is the default hosted path.

use reqwest::RequestBuilder;
use serde::Deserialize;

pub const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1";
pub const OPENROUTER_FREE: &str = "openrouter/free";

pub struct ChatEndpoint {
    pub url: String,
    pub key: Option<String>,
    pub model: String,
}

pub fn from_openrouter_env() -> Option<ChatEndpoint> {
    let key = first_filled(&["ECCLESIA_OPENROUTER_KEY", "OPENROUTER_API_KEY"])?;
    let url = std::env::var("ECCLESIA_OPENROUTER_URL")
        .ok()
        .filter(filled)
        .unwrap_or_else(|| OPENROUTER_URL.into());
    let model = std::env::var("ECCLESIA_OPENROUTER_MODEL")
        .ok()
        .filter(filled)
        .unwrap_or_else(|| OPENROUTER_FREE.into());
    Some(ChatEndpoint {
        url,
        key: Some(key),
        model,
    })
}

pub fn from_local_env() -> Option<ChatEndpoint> {
    let url = std::env::var("ECCLESIA_LLM_URL").ok().filter(filled)?;
    let model = std::env::var("ECCLESIA_LLM_MODEL")
        .ok()
        .filter(filled)
        .unwrap_or_else(|| "llama3.1:8b".into());
    let key = std::env::var("ECCLESIA_LLM_KEY").ok().filter(filled);
    Some(ChatEndpoint { url, key, model })
}

pub fn refine_model_override(default: &str) -> String {
    std::env::var("ECCLESIA_REFINE_MODEL")
        .ok()
        .filter(filled)
        .unwrap_or_else(|| default.to_string())
}

pub fn http_client(seconds: u64) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(seconds))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

pub fn apply_headers(mut request: RequestBuilder, endpoint: &ChatEndpoint) -> RequestBuilder {
    if let Some(key) = endpoint.key.as_deref() {
        request = request.bearer_auth(key);
    }
    if is_openrouter(&endpoint.url) {
        let referer = std::env::var("ECCLESIA_PUBLIC_URL")
            .ok()
            .filter(filled)
            .unwrap_or_else(|| "http://127.0.0.1:43781".into());
        request = request
            .header("HTTP-Referer", referer)
            .header("X-Title", "Ecclesia")
            .header("X-OpenRouter-Title", "Ecclesia");
    }
    request
}

pub fn is_openrouter(url: &str) -> bool {
    url.contains("openrouter.ai")
}

pub fn chat_completions_url(base: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.into()
    } else if trimmed.ends_with("/v1") {
        format!("{trimmed}/chat/completions")
    } else {
        format!("{trimmed}/v1/chat/completions")
    }
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    #[serde(default)]
    message: ChatMessage,
}

#[derive(Debug, Default, Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: String,
}

impl ChatResponse {
    pub fn first_text(&self) -> &str {
        self.choices
            .first()
            .map(|choice| choice.message.content.as_str())
            .unwrap_or("")
    }
}

fn first_filled(names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(filled))
}

fn filled(value: &String) -> bool {
    !value.trim().is_empty()
}
