# Voice

Everything posted here should lift people up. No tear-downs, no criticism of a
person, no profanity. A separate, optional rewrite helps someone say it more
clearly, the way Gmail offers a rewrite. They still press Send.

The leaf never calls a model. The skin weighs the words and hands the leaf a
`Posture`. The leaf only knows `Lifts` or `TearsDown`.

```
words  →  JudgeHub (Jev, local chat, or word gate)  →  Posture
words  →  RefineHub (local chat, or silent)         →  rewritten text
```

## Why not Rig in this binary

[Rig](https://github.com/0xPlaygrounds/rig) is a solid Rust agent crate. It
needs Rust 1.85 and edition 2024. This app is pinned to Rust 1.83 / edition
2021 so the rest of the crate graph stays buildable. Rig also does not run
weights on the GPU. The GPU work belongs in Ollama or llama.cpp on the box
with the 4080 Super. Ecclesia already speaks HTTP with `reqwest`. That is the
client.

## Classification: Jev first

[Jev](https://jevtypesafeai.com) is a hosted System One. You ask typed
questions (choice, score, noul). It does not write or rewrite. That is the
right tool for a gate: two nouls, `tears_down` and `profanity`, refuse at 0.5.

Set `ECCLESIA_JEV_KEY` (`jv_live_…`). Optional:

- `ECCLESIA_JEV_URL` — default `https://jevtypesafeai.com/api/v1/decide`
- `ECCLESIA_JEV_MODEL` — default `jev-latest`

Jev has no local weights. If you want the same questions on your own GPU,
skip the Jev key and point `ECCLESIA_LLM_URL` at Ollama. The chat path asks
for the same two scores as JSON.

If neither is set, a word gate catches obvious slurs and attacks. Tests use
`JudgeHub::word_gate()` or `silent()` (always lifts). If Jev or the local
model fails, the request falls back to the word gate so an outage does not
block every post.

Empty optional fields (a blank bio, a blank gift note) lift without a
network call. We do not require a high “this lifts people” score. A plain
need for five dinners is allowed.

## Rewrite: a second model, on your GPU

Rewrite is optional. The person can send what they typed. `POST /refine`
takes CSRF, a `VoiceKind`, and the text. The page has a Rewrite control on
bios, needs, offers, endorsements, gift notes, and church descriptions.

Point the OpenAI-compatible server at the 4080 Super:

```bash
# Ollama
ollama pull llama3.1:8b
ECCLESIA_LLM_URL=http://127.0.0.1:11434 \
ECCLESIA_LLM_MODEL=llama3.1:8b \
ECCLESIA_REFINE_MODEL=llama3.1:8b \
cargo run
```

```bash
# llama.cpp server, same protocol
./llama-server -m llama-3.1-8b-instruct.Q8_0.gguf --port 8081
ECCLESIA_LLM_URL=http://127.0.0.1:8081/v1 \
ECCLESIA_LLM_MODEL=llama-3.1-8b-instruct \
cargo run
```

`ECCLESIA_LLM_KEY` is optional. `ECCLESIA_REFINE_MODEL` overrides the
classify model if you want a larger rewrite model and a smaller gate.

On 16 GB (RTX 4080 Super), an 8B Q8 or a 14B Q4 fits. Do not load weights
inside the Axum process.

With no `ECCLESIA_LLM_URL`, rewrite is silent: it returns the same words
trimmed. The control still shows. It says no model is running.

## What the leaf sees

`Posture` is a flag like `CatalogPresence`. The skin calls `weigh` before
the leaf, then `register`, `plant_church`, `post_need`, `apply_to_need`,
`endorse`, `update_profile`, and `add_gift` receive it. `require_uplifting`
returns `DomainError::TearsDown`. The flash is `tone`: “Write it so it lifts
someone up.”

`VoiceKind` names the kind of words: need, offer, endorsement, gift, bio,
church. The judge and the rewriter ask a different question for each.
