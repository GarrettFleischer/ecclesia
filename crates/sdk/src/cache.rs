//! Shared fragments and rate keys. Never landing, Home, or inbox HTML.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ecclesia_domain::Write;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::host::is_public_host;

const FRAGMENT_TTL: Duration = Duration::from_secs(3600);
const RATE_TTL: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct Cache {
    inner: Arc<CacheInner>,
}

enum CacheInner {
    Memory(Mutex<MemoryStore>),
    Redis(RedisStore),
}

struct MemoryStore {
    values: HashMap<String, MemoryValue>,
}

struct MemoryValue {
    body: String,
    expires: Instant,
}

struct RedisStore {
    client: redis::Client,
}

pub fn require_public_cache_url(url: Option<&str>) -> anyhow::Result<&str> {
    match url {
        Some(url) if !url.is_empty() => Ok(url),
        _ => anyhow::bail!("UPSTASH_REDIS_URL is required on a public host"),
    }
}

impl Cache {
    pub fn memory() -> Self {
        Self {
            inner: Arc::new(CacheInner::Memory(Mutex::new(MemoryStore {
                values: HashMap::new(),
            }))),
        }
    }

    pub fn load() -> anyhow::Result<Self> {
        let url = std::env::var("UPSTASH_REDIS_URL").ok();
        if is_public_host() {
            let url = require_public_cache_url(url.as_deref())?;
            return Cache::redis(url);
        }
        match url {
            Some(url) if !url.is_empty() => match Cache::redis(&url) {
                Ok(cache) => Ok(cache),
                Err(error) => {
                    tracing::warn!("Upstash failed to open; using in process cache: {error:#}");
                    Ok(Cache::memory())
                }
            },
            _ => Ok(Cache::memory()),
        }
    }

    pub fn redis(url: &str) -> anyhow::Result<Self> {
        let client = redis::Client::open(url)?;
        Ok(Self {
            inner: Arc::new(CacheInner::Redis(RedisStore { client })),
        })
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        match self.try_get(key).await {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!("cache get failed for {key}: {error:#}");
                None
            }
        }
    }

    async fn try_get(&self, key: &str) -> anyhow::Result<Option<String>> {
        match self.inner.as_ref() {
            CacheInner::Memory(store) => {
                let mut map = store.lock().expect("cache");
                let Some(value) = map.values.get(key) else {
                    return Ok(None);
                };
                if Instant::now() >= value.expires {
                    map.values.remove(key);
                    return Ok(None);
                }
                Ok(Some(value.body.clone()))
            }
            CacheInner::Redis(store) => {
                let mut conn = store.client.get_multiplexed_async_connection().await?;
                let value: Option<String> = redis::cmd("GET").arg(key).query_async(&mut conn).await?;
                Ok(value)
            }
        }
    }

    pub async fn get_json<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let raw = self.get(key).await?;
        match serde_json::from_str(&raw) {
            Ok(value) => Some(value),
            Err(error) => {
                tracing::warn!("cache json failed for {key}: {error:#}");
                None
            }
        }
    }

    pub async fn set_json<T: Serialize>(&self, key: &str, value: &T) {
        let Ok(body) = serde_json::to_string(value) else {
            return;
        };
        if let Err(error) = self.try_set(key, &body, FRAGMENT_TTL).await {
            tracing::warn!("cache set failed for {key}: {error:#}");
        }
    }

    async fn try_set(&self, key: &str, body: &str, ttl: Duration) -> anyhow::Result<()> {
        match self.inner.as_ref() {
            CacheInner::Memory(store) => {
                let mut map = store.lock().expect("cache");
                map.values.insert(
                    key.to_string(),
                    MemoryValue {
                        body: body.to_string(),
                        expires: Instant::now() + ttl,
                    },
                );
                Ok(())
            }
            CacheInner::Redis(store) => {
                let mut conn = store.client.get_multiplexed_async_connection().await?;
                redis::cmd("SET")
                    .arg(key)
                    .arg(body)
                    .arg("EX")
                    .arg(ttl.as_secs())
                    .query_async::<()>(&mut conn)
                    .await?;
                Ok(())
            }
        }
    }

    pub async fn del_many(&self, keys: &[String]) {
        if keys.is_empty() {
            return;
        }
        if let Err(error) = self.try_del(keys).await {
            tracing::warn!("cache del failed: {error:#}");
        }
    }

    async fn try_del(&self, keys: &[String]) -> anyhow::Result<()> {
        match self.inner.as_ref() {
            CacheInner::Memory(store) => {
                let mut map = store.lock().expect("cache");
                for key in keys {
                    map.values.remove(key);
                }
                Ok(())
            }
            CacheInner::Redis(store) => {
                let mut conn = store.client.get_multiplexed_async_connection().await?;
                let mut cmd = redis::cmd("DEL");
                for key in keys {
                    cmd.arg(key);
                }
                cmd.query_async::<()>(&mut conn).await?;
                Ok(())
            }
        }
    }

    pub async fn incr_rate(&self, key: &str) -> u32 {
        self.incr_rate_window(key, RATE_TTL.as_secs()).await
    }

    pub async fn incr_rate_window(&self, key: &str, ttl_secs: u64) -> u32 {
        match self.try_incr_rate(key, ttl_secs).await {
            Ok(hits) => hits,
            Err(error) => {
                tracing::warn!("rate incr failed for {key}: {error:#}");
                1
            }
        }
    }

    async fn try_incr_rate(&self, key: &str, ttl_secs: u64) -> anyhow::Result<u32> {
        match self.inner.as_ref() {
            CacheInner::Memory(store) => {
                let mut map = store.lock().expect("cache");
                let now = Instant::now();
                if let Some(value) = map.values.get(key)
                    && now >= value.expires
                {
                    map.values.remove(key);
                }
                let hits = match map.values.get_mut(key) {
                    Some(value) => {
                        let next = value.body.parse::<u32>().unwrap_or(0).saturating_add(1);
                        value.body = next.to_string();
                        next
                    }
                    None => {
                        map.values.insert(
                            key.to_string(),
                            MemoryValue {
                                body: "1".into(),
                                expires: now + Duration::from_secs(ttl_secs),
                            },
                        );
                        1
                    }
                };
                Ok(hits)
            }
            CacheInner::Redis(store) => {
                let mut conn = store.client.get_multiplexed_async_connection().await?;
                let hits: i64 = redis::cmd("INCR").arg(key).query_async(&mut conn).await?;
                if hits == 1 {
                    redis::cmd("EXPIRE")
                        .arg(key)
                        .arg(ttl_secs as i64)
                        .query_async::<()>(&mut conn)
                        .await?;
                }
                Ok(hits as u32)
            }
        }
    }
}

pub fn keys_for_write(write: &Write, church_id: Option<&str>) -> Vec<String> {
    match write {
        Write::InsertChurch(church) => {
            vec!["directory".into(), format!("church:{}", church.id)]
        }
        Write::InsertMembership(membership) => {
            vec![
                format!("church:{}", membership.church_id),
                "directory".into(),
            ]
        }
        Write::SetMembershipStatus { .. } => church_id
            .map(|id| vec![format!("church:{id}"), "directory".into()])
            .unwrap_or_default(),
        Write::InsertNeed(need) => {
            vec![format!("church:{}", need.church_id), "directory".into()]
        }
        Write::SetNeedStatus { .. } => church_id
            .map(|id| vec![format!("church:{id}"), "directory".into()])
            .unwrap_or_default(),
        Write::InsertUser(_)
        | Write::UpdateUser { .. }
        | Write::InsertApplication(_)
        | Write::SetApplicationStatus { .. }
        | Write::InsertEndorsement(_)
        | Write::SetEndorsementStatus { .. }
        | Write::UpsertMemberGift { .. }
        | Write::RemoveMemberGift { .. } => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecclesia_domain::Church;

    fn church(id: &str) -> Church {
        Church {
            id: id.into(),
            name: id.into(),
            city: "Cedar Falls".into(),
            region: "Iowa".into(),
            country: "US".into(),
            description: String::new(),
            gathering: String::new(),
            owner_id: "o".into(),
            invite_code: "c".into(),
            created_at: "t".into(),
        }
    }

    #[test]
    fn us_cache_03_insert_church_deletes_directory_and_card() {
        let keys = keys_for_write(&Write::InsertChurch(church("grace")), None);
        assert_eq!(
            keys,
            vec!["directory".to_string(), "church:grace".to_string()]
        );
    }

    #[tokio::test]
    async fn us_cache_06_bad_json_is_a_miss_not_a_crash() {
        let cache = Cache::memory();
        cache.set_json("catalog", &"oops").await;
        let gifts: Option<Vec<ecclesia_domain::Gift>> = cache.get_json("catalog").await;
        assert!(gifts.is_none());
    }

    #[tokio::test]
    async fn us_cache_04_two_gates_share_one_memory_count() {
        let cache = Cache::memory();
        let first = cache.incr_rate("rate:register:same").await;
        let second = cache.incr_rate("rate:register:same").await;
        assert_eq!(first, 1);
        assert_eq!(second, 2);
    }

    #[test]
    fn us_cache_05_public_host_refuses_a_missing_upstash_url() {
        assert!(require_public_cache_url(None).is_err());
        assert!(require_public_cache_url(Some("")).is_err());
        assert_eq!(
            require_public_cache_url(Some("rediss://upstash.example")).unwrap(),
            "rediss://upstash.example"
        );
    }

    #[test]
    fn us_cache_01_forbids_personalized_html_keys() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        walk(&root, &mut offenders);
        assert!(
            offenders.is_empty(),
            "cache must not store landing, Home, or inbox HTML:\n{}",
            offenders.join("\n")
        );
    }

    fn walk(path: &std::path::Path, offenders: &mut Vec<String>) {
        if path.is_dir() {
            for entry in std::fs::read_dir(path).expect("read dir") {
                walk(&entry.expect("dir entry").path(), offenders);
            }
            return;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            return;
        }
        if path.ends_with("cache.rs") {
            return;
        }
        let src = std::fs::read_to_string(path).expect("read rust");
        for (index, line) in src.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.contains("\"landing\"")
                || trimmed.contains("\"home:")
                || trimmed.contains("\"inbox:")
                || trimmed.contains("format!(\"home:")
                || trimmed.contains("format!(\"inbox:")
                || trimmed.contains("format!(\"landing")
            {
                offenders.push(format!("{}:{}: {}", path.display(), index + 1, trimmed));
            }
        }
    }
}
