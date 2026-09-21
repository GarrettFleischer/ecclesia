//! In-process request budgets. The skin asks; this only counts.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const BUCKET_CAP: usize = 4096;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RateKind {
    Session,
    Register,
    Refine,
    Redeem,
    Push,
}

impl RateKind {
    fn tag(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Register => "register",
            Self::Refine => "refine",
            Self::Redeem => "redeem",
            Self::Push => "push",
        }
    }

    fn max_hits(self) -> u32 {
        match self {
            Self::Session => 10,
            Self::Register => 5,
            Self::Refine => 20,
            Self::Redeem => 8,
            Self::Push => 30,
        }
    }

    fn window(self) -> Duration {
        Duration::from_secs(60)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateDecision {
    Allow,
    Refuse,
}

struct Bucket {
    hits: u32,
    started: Instant,
}

#[derive(Clone)]
pub struct RateGate {
    inner: Arc<Mutex<HashMap<String, Bucket>>>,
}

impl RateGate {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn decide(&self, kind: RateKind, who: &str) -> RateDecision {
        let key = rate_key(kind, who);
        let now = Instant::now();
        let mut map = self.inner.lock().expect("rate gate");
        prune_if_full(&mut map, now);
        let bucket = map.entry(key).or_insert(Bucket {
            hits: 0,
            started: now,
        });
        if now.duration_since(bucket.started) >= kind.window() {
            bucket.hits = 0;
            bucket.started = now;
        }
        if bucket.hits >= kind.max_hits() {
            return RateDecision::Refuse;
        }
        bucket.hits += 1;
        RateDecision::Allow
    }
}

fn rate_key(kind: RateKind, who: &str) -> String {
    let mut key = String::with_capacity(kind.tag().len() + who.len() + 1);
    key.push_str(kind.tag());
    key.push(':');
    key.push_str(who);
    key
}

fn prune_if_full(map: &mut HashMap<String, Bucket>, now: Instant) {
    if map.len() < BUCKET_CAP {
        return;
    }
    map.retain(|_, bucket| now.duration_since(bucket.started) < Duration::from_secs(60));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_sec_10_refine_budget_refuses_the_next_hit() {
        let gate = RateGate::new();
        for _ in 0..RateKind::Refine.max_hits() {
            assert_eq!(gate.decide(RateKind::Refine, "local"), RateDecision::Allow);
        }
        assert_eq!(gate.decide(RateKind::Refine, "local"), RateDecision::Refuse);
        assert_eq!(gate.decide(RateKind::Refine, "other"), RateDecision::Allow);
    }
}
