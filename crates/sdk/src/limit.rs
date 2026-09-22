//! Request budgets. Shared through the cache.

use crate::cache::Cache;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RateKind {
    Session,
    Register,
    Refine,
    Redeem,
    Push,
}

impl RateKind {
    pub fn tag(self) -> &'static str {
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateDecision {
    Allow,
    Refuse,
}

#[derive(Clone)]
pub struct RateGate {
    cache: Cache,
}

impl RateGate {
    pub fn new() -> Self {
        Self::with_cache(Cache::memory())
    }

    pub fn with_cache(cache: Cache) -> Self {
        Self { cache }
    }

    pub async fn decide(&self, kind: RateKind, who: &str) -> RateDecision {
        let key = rate_key(kind, who);
        let hits = self.cache.incr_rate(&key).await;
        if hits > kind.max_hits() {
            RateDecision::Refuse
        } else {
            RateDecision::Allow
        }
    }

    pub async fn decide_mail(&self, email: &str) -> RateDecision {
        let key = format!("rate:mail:{email}");
        let hits = self.cache.incr_rate_window(&key, 3600).await;
        if hits > 5 {
            RateDecision::Refuse
        } else {
            RateDecision::Allow
        }
    }
}

fn rate_key(kind: RateKind, who: &str) -> String {
    format!("rate:{}:{}", kind.tag(), who)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn us_sec_10_refine_budget_refuses_the_next_hit() {
        let gate = RateGate::new();
        for _ in 0..RateKind::Refine.max_hits() {
            assert_eq!(
                gate.decide(RateKind::Refine, "local").await,
                RateDecision::Allow
            );
        }
        assert_eq!(
            gate.decide(RateKind::Refine, "local").await,
            RateDecision::Refuse
        );
        assert_eq!(
            gate.decide(RateKind::Refine, "other").await,
            RateDecision::Allow
        );
    }

    #[tokio::test]
    async fn us_cache_04_two_gates_share_one_count() {
        let cache = Cache::memory();
        let left = RateGate::with_cache(cache.clone());
        let right = RateGate::with_cache(cache);
        for _ in 0..RateKind::Register.max_hits() {
            assert_eq!(
                left.decide(RateKind::Register, "same").await,
                RateDecision::Allow
            );
        }
        assert_eq!(
            right.decide(RateKind::Register, "same").await,
            RateDecision::Refuse
        );
    }
}
