# Voice

Everything posted here should lift people up. No tear-downs, no criticism of a
person, no profanity.

Submit is two steps. The first pass rewrites the words. The person reads that
version, edits it, then publishes. Classification runs on what they publish.

Domain never calls a model. The skin weighs the words and hands Domain a
`Posture`. Domain only knows `Lifts` or `TearsDown`.

```
words  →  RefineHub (OpenRouter free, local chat, or polish)  →  review
words  →  JudgeHub (Jev, OpenRouter, local, or word gate)     →  Posture
```

## Nightly Rust

This crate tracks nightly (`rust-toolchain.toml`) and edition 2024. Install
with `rustup toolchain install nightly`. The parallel frontend on recent
nightlies is the reason.

## Classification

1. **Jev** if `ECCLESIA_JEV_KEY` is set — two nouls, `tears_down` and
   `profanity`, refuse at 0.5.
2. **OpenRouter free router** if `ECCLESIA_OPENROUTER_KEY` or
   `OPENROUTER_API_KEY` is set — model `openrouter/free` unless you set
   `ECCLESIA_OPENROUTER_MODEL`.
3. **Local chat** if `ECCLESIA_LLM_URL` is set.
4. **Word gate** otherwise, and whenever a remote judge fails.

Jev cannot rewrite and has no local weights. Empty optional fields lift
without a network call.

## Rewrite

OpenRouter is the default hosted path for testing. Same key as above.
`ECCLESIA_REFINE_MODEL` overrides the model for rewrite only.

```bash
ECCLESIA_OPENROUTER_KEY=sk-or-... \
ECCLESIA_OPENROUTER_MODEL=openrouter/free \
cargo run
```

On the 4080 Super, skip the OpenRouter key and point at Ollama:

```bash
ECCLESIA_LLM_URL=http://127.0.0.1:11434 \
ECCLESIA_LLM_MODEL=llama3.1:8b \
cargo run
```

`POST /refine` takes CSRF, a `VoiceKind`, and the text. First submit of a
writing form refines the fields and shows them. The button becomes Publish.

Tests use `JudgeHub::word_gate()` and either `RefineHub::silent()` (echo,
publishes on the first submit) or `RefineHub::polish()` (collapses spaces
and is live, so the first submit is a review).

## Prompts

These live in `crates/sdk/src/prompts.rs`. Change them there.

### Classify (chat / OpenRouter)

System:

```
You weigh short church-app posts. Reply with JSON only: {"tears_down":0.0,"profanity":0.0}. Each value is 0 to 1. tears_down is high if the text attacks, mocks, or criticizes a person. profanity is high if it uses slurs or crude language. A plain need or a kind note is 0.
```

User:

```
kind={need|offer|endorsement|gift|bio|church}
{the words}
```

### Classify (Jev nouls)

`tears_down`

- Instructions: Does this text tear a person down, criticize them, insult them, or speak about them with contempt?
- True: It attacks, mocks, or criticizes a person.
- False: It names a need, a gift, or a kindness without attacking anyone.

`profanity`

- Instructions: Does this text contain profanity, slurs, or crude sexual language?
- True: It uses profanity or slurs.
- False: The wording is clean.

### Rewrite

Need: Rewrite this church need so it is clear and kind. Keep the facts they named. Return only the rewritten text.

Offer: Rewrite this offer to help so it is clear and kind. Keep when they can come and what they can do. Return only the rewritten text.

Endorsement: Rewrite this endorsement so it is clear and heartfelt. Keep the specific thing they saw. Return only the rewritten text.

Gift note: Rewrite this gift note so it is clear. Keep how they serve. Return only the rewritten text.

Bio: Rewrite this short bio so it is clear. Keep how they named themselves. Return only the rewritten text.

Church: Rewrite this church description so it is clear. Keep where they are and who comes. Return only the rewritten text.

The user message for rewrite is the raw field.

## What Domain sees

`VoicePass::Review` is the first submit. `VoicePass::Publish` is the second.
`Posture` still arrives on publish. `require_uplifting` returns
`DomainError::TearsDown`. The flash is `tone`.
