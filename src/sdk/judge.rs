//! Weigh whether words lift people up.
//!
//! Prefer Jev (typed nouls). For testing, OpenRouter's free router asks the
//! same two scores. A local OpenAI-compatible server is the last chat path.
//! Tests use `silent`, which always lifts.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::leaf::{Posture, VoiceKind};

use super::chat::{
    ChatEndpoint, ChatResponse, apply_headers, chat_completions_url, from_local_env,
    from_openrouter_env, http_client, is_openrouter,
};
use super::prompts;

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
        endpoint: ChatEndpoint,
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
        if let Some(endpoint) = from_openrouter_env() {
            tracing::info!(
                "weighing words with OpenRouter {} at {}",
                endpoint.model,
                endpoint.url
            );
            return Self {
                inner: Arc::new(JudgeInner::Chat {
                    endpoint,
                    http: http_client(12),
                }),
            };
        }
        if let Some(endpoint) = from_local_env() {
            tracing::info!(
                "weighing words with the local chat model {} at {}",
                endpoint.model,
                endpoint.url
            );
            return Self {
                inner: Arc::new(JudgeInner::Chat {
                    endpoint,
                    http: http_client(8),
                }),
            };
        }
        tracing::info!("no Jev or chat model; using the word gate for obvious attacks");
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
            JudgeInner::Chat { endpoint, http } => chat_weigh(http, endpoint, kind, parts).await,
        }
    }
}

fn from_jev_env() -> Option<JudgeHub> {
    let key = std::env::var("ECCLESIA_JEV_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())?;
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
                "instructions": prompts::jev_tears_down_instructions(),
                "criteria": {
                    "true": prompts::jev_tears_down_true(),
                    "false": prompts::jev_tears_down_false()
                }
            },
            "profanity": {
                "type": "noul",
                "instructions": prompts::jev_profanity_instructions(),
                "criteria": {
                    "true": prompts::jev_profanity_true(),
                    "false": prompts::jev_profanity_false()
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
    endpoint: &ChatEndpoint,
    kind: VoiceKind,
    parts: &[&str],
) -> anyhow::Result<Posture> {
    let url = chat_completions_url(&endpoint.url);
    let text = join_parts(parts);
    let mut body = json!({
        "model": endpoint.model,
        "temperature": 0,
        "messages": [
            { "role": "system", "content": prompts::classify_system() },
            { "role": "user", "content": prompts::classify_user(kind, &text) }
        ]
    });
    if is_openrouter(&endpoint.url) {
        body["response_format"] = json!({ "type": "json_object" });
    }
    let request = apply_headers(http.post(&url).json(&body), endpoint);
    let response = request.send().await?.error_for_status()?;
    let parsed: ChatResponse = response.json().await?;
    let scores = parse_score_json(parsed.first_text())?;
    Ok(posture_from_nouls(scores.tears_down, scores.profanity))
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
        append_folded_part(out, part);
    }
}

fn append_folded_part(out: &mut String, part: &str) {
    for ch in part.chars() {
        if let Some(letter) = fold_letter(ch) {
            out.push(letter);
        }
    }
}

fn fold_letter(ch: char) -> Option<char> {
    match ch {
        '0' => Some('o'),
        '1' | '!' => Some('i'),
        '3' => Some('e'),
        '4' | '@' => Some('a'),
        '5' | '$' => Some('s'),
        '7' => Some('t'),
        c if c.is_ascii_alphabetic() => Some(c.to_ascii_lowercase()),
        _ => None,
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
        "yousuck",
        "worthless",
        "hateyou",
        "killyourself",
        "idiot",
        "moron",
        "dumbass",
        "pieceofcrap",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk::chat::chat_completions_url;

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
        assert_eq!(word_gate(&["f u c k off"]), Posture::TearsDown);
        assert_eq!(word_gate(&["You are w0rthless."]), Posture::TearsDown);
        assert_eq!(word_gate(&["k1ll yourself"]), Posture::TearsDown);
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
