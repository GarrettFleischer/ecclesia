//! Weigh whether words lift people up.
//!
//! Prefer Jev (typed nouls). On a local box, the same questions go to an
//! OpenAI-compatible server. Tests use `silent`, which always lifts.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::leaf::{Posture, VoiceKind};

const JEV_DEFAULT_URL: &str = "https://jevtypesafeai.com/api/v1/decide";
const NOUL_GATE: f64 = 0.5;

#[derive(Clone)]
pub struct JudgeHub {
    inner: Arc<JudgeInner>,
}

enum JudgeInner {
    Silent,
    WordGate,
    Jev {
        url: String,
        key: String,
        model: String,
        http: reqwest::Client,
    },
    Chat {
        url: String,
        key: Option<String>,
        model: String,
        http: reqwest::Client,
    },
}

impl JudgeHub {
    pub fn silent() -> Self {
        Self {
            inner: Arc::new(JudgeInner::Silent),
        }
    }

    pub fn word_gate() -> Self {
        Self {
            inner: Arc::new(JudgeInner::WordGate),
        }
    }

    pub fn load() -> Self {
        if let Some(hub) = from_jev_env() {
            return hub;
        }
        if let Some(hub) = from_chat_env() {
            return hub;
        }
        tracing::info!("no Jev or local LLM; using the word gate for obvious attacks");
        Self::word_gate()
    }

    pub async fn weigh(&self, kind: VoiceKind, parts: &[&str]) -> Posture {
        match self.try_weigh(kind, parts).await {
            Ok(posture) => posture,
            Err(error) => {
                tracing::warn!("judge failed, falling back to the word gate: {error:#}");
                word_gate(parts)
            }
        }
    }

    async fn try_weigh(&self, kind: VoiceKind, parts: &[&str]) -> anyhow::Result<Posture> {
        if joined_is_empty(parts) {
            return Ok(Posture::Lifts);
        }
        match self.inner.as_ref() {
            JudgeInner::Silent => Ok(Posture::Lifts),
            JudgeInner::WordGate => Ok(word_gate(parts)),
            JudgeInner::Jev {
                url,
                key,
                model,
                http,
            } => jev_weigh(http, url, key, model, kind, parts).await,
            JudgeInner::Chat {
                url,
                key,
                model,
                http,
            } => chat_weigh(http, url, key.as_deref(), model, kind, parts).await,
        }
    }
}

fn from_jev_env() -> Option<JudgeHub> {
    let key = std::env::var("ECCLESIA_JEV_KEY").ok().filter(filled)?;
    let url = std::env::var("ECCLESIA_JEV_URL").unwrap_or_else(|_| JEV_DEFAULT_URL.into());
    let model = std::env::var("ECCLESIA_JEV_MODEL").unwrap_or_else(|_| "jev-latest".into());
    tracing::info!("weighing words with Jev at {url}");
    Some(JudgeHub {
        inner: Arc::new(JudgeInner::Jev {
            url,
            key,
            model,
            http: http_client(8),
        }),
    })
}

fn from_chat_env() -> Option<JudgeHub> {
    let url = std::env::var("ECCLESIA_LLM_URL").ok().filter(filled)?;
    let model = std::env::var("ECCLESIA_LLM_MODEL").unwrap_or_else(|_| "llama3.1:8b".into());
    let key = std::env::var("ECCLESIA_LLM_KEY").ok().filter(filled);
    tracing::info!("weighing words with the local chat model {model} at {url}");
    Some(JudgeHub {
        inner: Arc::new(JudgeInner::Chat {
            url,
            key,
            model,
            http: http_client(8),
        }),
    })
}

fn http_client(seconds: u64) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(seconds))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn filled(value: &String) -> bool {
    !value.trim().is_empty()
}

fn joined_is_empty(parts: &[&str]) -> bool {
    parts.iter().all(|part| part.trim().is_empty())
}

fn join_parts(parts: &[&str]) -> String {
    let mut out = String::new();
    append_parts(&mut out, parts);
    out
}

fn append_parts(out: &mut String, parts: &[&str]) {
    for part in parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(trimmed);
    }
}

pub fn jev_body(kind: VoiceKind, parts: &[&str]) -> Value {
    let state = json!({
        "kind": kind.as_str(),
        "text": join_parts(parts),
    });
    json!({
        "model": "jev-latest",
        "state": state,
        "questions": {
            "tears_down": {
                "type": "noul",
                "instructions": "Does this text tear a person down, criticize them, insult them, or speak about them with contempt?",
                "criteria": {
                    "true": "It attacks, mocks, or criticizes a person.",
                    "false": "It names a need, a gift, or a kindness without attacking anyone."
                }
            },
            "profanity": {
                "type": "noul",
                "instructions": "Does this text contain profanity, slurs, or crude sexual language?",
                "criteria": {
                    "true": "It uses profanity or slurs.",
                    "false": "The wording is clean."
                }
            }
        }
    })
}

fn posture_from_nouls(tears_down: f64, profanity: f64) -> Posture {
    if tears_down >= NOUL_GATE || profanity >= NOUL_GATE {
        Posture::TearsDown
    } else {
        Posture::Lifts
    }
}

async fn jev_weigh(
    http: &reqwest::Client,
    url: &str,
    key: &str,
    model: &str,
    kind: VoiceKind,
    parts: &[&str],
) -> anyhow::Result<Posture> {
    let mut body = jev_body(kind, parts);
    body["model"] = json!(model);
    let response = http
        .post(url)
        .bearer_auth(key)
        .json(&body)
        .send()
        .await?
        .error_for_status()?;
    let parsed: JevResponse = response.json().await?;
    Ok(posture_from_nouls(
        parsed.noul("tears_down"),
        parsed.noul("profanity"),
    ))
}

#[derive(Debug, Deserialize)]
struct JevResponse {
    #[serde(default)]
    answers: JevAnswers,
}

#[derive(Debug, Default, Deserialize)]
struct JevAnswers {
    #[serde(default)]
    tears_down: Option<JevNoul>,
    #[serde(default)]
    profanity: Option<JevNoul>,
}

#[derive(Debug, Deserialize)]
struct JevNoul {
    #[serde(default)]
    noul: f64,
}

impl JevResponse {
    fn noul(&self, name: &str) -> f64 {
        match name {
            "tears_down" => self
                .answers
                .tears_down
                .as_ref()
                .map(|n| n.noul)
                .unwrap_or(0.0),
            "profanity" => self
                .answers
                .profanity
                .as_ref()
                .map(|n| n.noul)
                .unwrap_or(0.0),
            _ => 0.0,
        }
    }
}

async fn chat_weigh(
    http: &reqwest::Client,
    base: &str,
    key: Option<&str>,
    model: &str,
    kind: VoiceKind,
    parts: &[&str],
) -> anyhow::Result<Posture> {
    let url = chat_completions_url(base);
    let text = join_parts(parts);
    let mut request = http.post(&url).json(&json!({
        "model": model,
        "temperature": 0,
        "messages": [
            {
                "role": "system",
                "content": "You weigh short church-app posts. Reply with JSON only: {\"tears_down\":0.0,\"profanity\":0.0}. Each value is 0 to 1. tears_down is high if the text attacks, mocks, or criticizes a person. profanity is high if it uses slurs or crude language. A plain need or a kind note is 0."
            },
            {
                "role": "user",
                "content": format!("kind={}\n{}", kind.as_str(), text)
            }
        ]
    }));
    if let Some(key) = key {
        request = request.bearer_auth(key);
    }
    let response = request.send().await?.error_for_status()?;
    let parsed: ChatResponse = response.json().await?;
    let scores = parse_score_json(parsed.first_text())?;
    Ok(posture_from_nouls(scores.tears_down, scores.profanity))
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChatResponse {
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
    pub(crate) fn first_text(&self) -> &str {
        self.choices
            .first()
            .map(|choice| choice.message.content.as_str())
            .unwrap_or("")
    }
}

#[derive(Debug, Deserialize)]
struct ScoreJson {
    #[serde(default)]
    tears_down: f64,
    #[serde(default)]
    profanity: f64,
}

fn parse_score_json(raw: &str) -> anyhow::Result<ScoreJson> {
    let trimmed = raw.trim();
    let json_slice = json_object(trimmed).unwrap_or(trimmed);
    Ok(serde_json::from_str(json_slice)?)
}

fn json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(&raw[start..=end])
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

pub fn word_gate(parts: &[&str]) -> Posture {
    let mut hay = String::new();
    append_folded(&mut hay, parts);
    if forbidden_needles()
        .iter()
        .any(|needle| hay.contains(needle))
    {
        Posture::TearsDown
    } else {
        Posture::Lifts
    }
}

fn append_folded(out: &mut String, parts: &[&str]) {
    for part in parts {
        for ch in part.chars() {
            if ch.is_ascii_alphabetic() {
                out.push(ch.to_ascii_lowercase());
            } else if !out.ends_with(' ') {
                out.push(' ');
            }
        }
    }
}

fn forbidden_needles() -> &'static [&'static str] {
    &[
        "fuck",
        "shit",
        "bitch",
        "asshole",
        "bastard",
        "cunt",
        "nigger",
        "faggot",
        "retard",
        "whore",
        "slut",
        "you suck",
        "worthless",
        "hate you",
        "kill yourself",
        "idiot",
        "moron",
        "dumbass",
        "piece of crap",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_tone_01_word_gate_lets_a_kind_note_through() {
        assert_eq!(
            word_gate(&["She stayed until the last parent came."]),
            Posture::Lifts
        );
        assert_eq!(
            word_gate(&["Need five dinners this week for the Okonkwos."]),
            Posture::Lifts
        );
    }

    #[test]
    fn us_tone_01_word_gate_stops_an_attack() {
        assert_eq!(
            word_gate(&["You are worthless and you suck."]),
            Posture::TearsDown
        );
        assert_eq!(word_gate(&["What a dumbass."]), Posture::TearsDown);
    }

    #[test]
    fn us_tone_01_jev_body_asks_two_nouls() {
        let body = jev_body(VoiceKind::Endorsement, &["She stayed."]);
        assert_eq!(body["questions"]["tears_down"]["type"], "noul");
        assert_eq!(body["questions"]["profanity"]["type"], "noul");
        assert_eq!(body["state"]["kind"], "endorsement");
    }

    #[test]
    fn us_tone_01_chat_url_grows_the_completions_path() {
        assert_eq!(
            chat_completions_url("http://127.0.0.1:11434"),
            "http://127.0.0.1:11434/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("http://127.0.0.1:11434/v1"),
            "http://127.0.0.1:11434/v1/chat/completions"
        );
    }

    #[test]
    fn us_tone_01_score_json_reads_fenced_output() {
        let scores =
            parse_score_json("```json\n{\"tears_down\":0.9,\"profanity\":0.1}\n```").unwrap();
        assert_eq!(scores.tears_down, 0.9);
        assert_eq!(
            posture_from_nouls(scores.tears_down, scores.profanity),
            Posture::TearsDown
        );
    }

    #[tokio::test]
    async fn us_tone_01_silent_judge_always_lifts() {
        let hub = JudgeHub::silent();
        assert_eq!(
            hub.weigh(VoiceKind::Need, &["You are worthless."]).await,
            Posture::Lifts
        );
    }
}
