//! The words we send to Jev and to the chat models.
//!
//! Edit this file when you want to change how classification or rewrite
//! behaves. The same strings are printed in docs/VOICE.md.

use crate::leaf::VoiceKind;

pub fn classify_system() -> &'static str {
    "You weigh short church-app posts. Reply with JSON only: {\"tears_down\":0.0,\"profanity\":0.0}. Each value is 0 to 1. tears_down is high if the text attacks, mocks, or criticizes a person. profanity is high if it uses slurs or crude language. A plain need or a kind note is 0."
}

pub fn classify_user(kind: VoiceKind, text: &str) -> String {
    format!("kind={}\n{text}", kind.as_str())
}

pub fn refine_system(kind: VoiceKind) -> &'static str {
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

pub fn jev_tears_down_instructions() -> &'static str {
    "Does this text tear a person down, criticize them, insult them, or speak about them with contempt?"
}

pub fn jev_tears_down_true() -> &'static str {
    "It attacks, mocks, or criticizes a person."
}

pub fn jev_tears_down_false() -> &'static str {
    "It names a need, a gift, or a kindness without attacking anyone."
}

pub fn jev_profanity_instructions() -> &'static str {
    "Does this text contain profanity, slurs, or crude sexual language?"
}

pub fn jev_profanity_true() -> &'static str {
    "It uses profanity or slurs."
}

pub fn jev_profanity_false() -> &'static str {
    "The wording is clean."
}
