//! Live reply notices for an open need.
//!
//! Postgres stays authoritative. This module publishes the need id and reply id
//! after a reply commits, and a watch turns those notices into a stream the App
//! can serve as server-sent events. It does not decide who may see the need.
//!
//! One global Redis stream, `replies`, holds the notices. Each entry is
//! `need_id` and `reply_id` only. The stream is capped with an approximate
//! `MAXLEN`. When Redis is absent, an in-process hub is the whole transport.
//! When Redis is present, that same hub still wakes subscribers in the
//! publishing process. A Redis failure is logged and does not undo the reply.
//!
//! `after_stream_id` is the `Last-Event-ID` the App received on reconnect.
//! Notices at or before that id are skipped. If the id has been trimmed, the
//! stream yields [`WatchEvent::Refresh`] so the client reloads the conversation
//! from Postgres. The client deduplicates by reply id.
//!
//! # Examples
//!
//! ```no_run
//! use ecclesia_sdk::live::{publish_committed_reply, watch_need};
//!
//! async fn after_commit(sdk: &ecclesia_sdk::Sdk) {
//!     publish_committed_reply(sdk, "need-1", "reply-1").await;
//!     let events = watch_need(sdk, "need-1", None);
//!     drop(events);
//! }
//!
//! fn main() {
//!     let start = after_commit;
//!     drop(start);
//! }
//! ```

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::anyhow;

const REPLY_STREAM: &str = "replies";
const REPLY_STREAM_LIMIT: usize = 10_000;
const REPLY_REDIS_BUDGET: Duration = Duration::from_secs(1);
const REMOTE_POLL: Duration = Duration::from_millis(500);

/// A committed reply, named by ids the client can deduplicate.
///
/// # Notes
/// `stream_id` is the Redis stream id, or a monotonic id from the in-process
/// hub when Redis did not accept the publish. It is stable for `Last-Event-ID`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyNotice {
    pub stream_id: String,
    pub need_id: String,
    pub reply_id: String,
}

/// One item on a need's live watch.
///
/// # Notes
/// [`WatchEvent::Refresh`] means the requested stream id is gone. Reload the
/// conversation from Postgres, then keep reading later notices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent {
    Reply(ReplyNotice),
    Refresh,
}

/// Publishes a reply that has already committed.
///
/// # Parameters
/// - `sdk`: the process SDK. Its hub wakes local subscribers.
/// - `need_id`: the need the reply belongs to.
/// - `reply_id`: the committed reply. The body is not published.
///
/// # Returns
/// Nothing. Redis errors are logged. The caller keeps a successful commit.
///
/// # Examples
///
/// See the module example. Call this only after the database commit returns.
pub async fn publish_committed_reply(sdk: &crate::story::Sdk, need_id: &str, reply_id: &str) {
    sdk.replies.publish(need_id, reply_id).await;
}

/// Watches notices for one need.
///
/// # Parameters
/// - `sdk`: the process SDK.
/// - `need_id`: the need to keep. Other needs on the shared stream are skipped.
/// - `after_stream_id`: the last stream id the client already applied.
///   `None` replays the notices still retained, then follows new ones.
///
/// # Returns
/// A stream of [`WatchEvent`]. It stays open until dropped. The App checks the
/// need audience before calling this.
///
/// # Examples
///
/// A reconnect passes the previous `Last-Event-ID`:
///
/// ```no_run
/// use ecclesia_sdk::live::watch_need;
///
/// fn follow(sdk: &ecclesia_sdk::Sdk, last_event_id: String) {
///     let events = watch_need(sdk, "need-1", Some(last_event_id));
///     drop(events);
/// }
///
/// fn main() {
///     let start = follow;
///     drop(start);
/// }
/// ```
#[must_use = "dropping the stream ends the watch"]
pub fn watch_need(
    sdk: &crate::story::Sdk,
    need_id: &str,
    after_stream_id: Option<String>,
) -> impl futures::Stream<Item = WatchEvent> + Send + use<> {
    futures::stream::unfold(
        WatchState::prepare(sdk, need_id, after_stream_id),
        WatchState::step,
    )
}

/// In-process reply log and the Redis publisher for one SDK.
#[derive(Clone)]
pub(crate) struct ReplyHub {
    inner: Arc<HubInner>,
}

struct HubInner {
    log: Mutex<MemoryLog>,
    wake: tokio::sync::broadcast::Sender<ReplyNotice>,
    remote: Remote,
    failures: AtomicU64,
    connection: Mutex<Option<redis::aio::MultiplexedConnection>>,
}

struct MemoryLog {
    limit: usize,
    last_ms: u64,
    seq: u64,
    entries: VecDeque<ReplyNotice>,
}

enum Remote {
    Absent,
    Redis(redis::Client),
    #[cfg(test)]
    Failing,
}

struct Caught {
    refresh: bool,
    matching: Vec<ReplyNotice>,
    cursor: Option<String>,
}

enum IdOrder {
    Before,
    Same,
    After,
    Unreadable,
}

enum Phase {
    Start,
    Ready {
        queued: VecDeque<WatchEvent>,
        rx: tokio::sync::broadcast::Receiver<ReplyNotice>,
    },
}

enum HubDrain {
    Open,
    Closed,
}

struct WatchState {
    hub: ReplyHub,
    need_id: String,
    cursor: Option<String>,
    phase: Phase,
}

impl ReplyHub {
    pub(crate) fn open() -> Self {
        Self::with_remote(remote_from_environment(), REPLY_STREAM_LIMIT)
    }

    #[cfg(test)]
    fn memory_with_limit(limit: usize) -> Self {
        Self::with_remote(Remote::Absent, limit)
    }

    #[cfg(test)]
    fn failing() -> Self {
        Self::with_remote(Remote::Failing, REPLY_STREAM_LIMIT)
    }

    fn with_remote(remote: Remote, limit: usize) -> Self {
        let (wake, idle) = tokio::sync::broadcast::channel(1024);
        drop(idle);
        Self {
            inner: Arc::new(HubInner {
                log: Mutex::new(MemoryLog {
                    limit,
                    last_ms: 0,
                    seq: 0,
                    entries: VecDeque::new(),
                }),
                wake,
                remote,
                failures: AtomicU64::new(0),
                connection: Mutex::new(None),
            }),
        }
    }

    async fn publish(&self, need_id: &str, reply_id: &str) {
        let stream_id = self.assign_stream_id(need_id, reply_id).await;
        let notice = self.remember(need_id, reply_id, stream_id);
        self.wake(notice);
    }

    async fn assign_stream_id(&self, need_id: &str, reply_id: &str) -> String {
        match &self.inner.remote {
            Remote::Absent => self.mint_id(),
            #[cfg(test)]
            Remote::Failing => {
                self.note_failure(need_id, reply_id, "redis publish failed");
                self.mint_id()
            }
            Remote::Redis(_) => match self.xadd(need_id, reply_id).await {
                Ok(stream_id) => stream_id,
                Err(error) => {
                    self.note_failure(need_id, reply_id, &format!("{error:#}"));
                    self.mint_id()
                }
            },
        }
    }

    fn remember(&self, need_id: &str, reply_id: &str, stream_id: String) -> ReplyNotice {
        let notice = ReplyNotice {
            stream_id,
            need_id: need_id.to_owned(),
            reply_id: reply_id.to_owned(),
        };
        let mut log = self.inner.log.lock().expect("reply hub");
        log.entries.push_back(notice.clone());
        let mut trimmed_through = None;
        while log.entries.len() > log.limit {
            trimmed_through = log.entries.pop_front();
        }
        if let Some(boundary) = trimmed_through {
            tracing::trace!(
                "reply stream trimmed through {} on need {}",
                boundary.stream_id,
                boundary.need_id
            );
        }
        notice
    }

    fn wake(&self, notice: ReplyNotice) {
        match self.inner.wake.send(notice) {
            Ok(listeners) => {
                tracing::trace!("reply notice delivered to {listeners} listeners");
            }
            Err(tokio::sync::broadcast::error::SendError(missed)) => {
                tracing::trace!(
                    "reply {} on need {} stored for catch-up",
                    missed.reply_id,
                    missed.need_id
                );
            }
        }
    }

    fn mint_id(&self) -> String {
        self.inner.log.lock().expect("reply hub").mint()
    }

    fn note_failure(&self, need_id: &str, reply_id: &str, reason: &str) {
        let count = self
            .inner
            .failures
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        tracing::warn!(
            "reply stream publish failed for need {need_id} reply {reply_id} ({count}): {reason}"
        );
    }

    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<ReplyNotice> {
        self.inner.wake.subscribe()
    }

    async fn catch_up(&self, need_id: &str, after: Option<&str>) -> Caught {
        match &self.inner.remote {
            Remote::Redis(_) => self.redis_catch_up(need_id, after).await,
            Remote::Absent => self.memory_catch_up(need_id, after),
            #[cfg(test)]
            Remote::Failing => self.memory_catch_up(need_id, after),
        }
    }

    fn memory_catch_up(&self, need_id: &str, after: Option<&str>) -> Caught {
        let log = self.inner.log.lock().expect("reply hub");
        if let Some(after) = after {
            if let Some(oldest) = log.entries.front() {
                if history_was_trimmed(after, &oldest.stream_id) {
                    let cursor = log.entries.back().map(|entry| entry.stream_id.clone());
                    return Caught {
                        refresh: true,
                        matching: Vec::new(),
                        cursor,
                    };
                }
            } else {
                return Caught {
                    refresh: true,
                    matching: Vec::new(),
                    cursor: Some(after.to_owned()),
                };
            }
        }
        caught_from_notices(need_id, after, log.entries.iter())
    }

    async fn redis_catch_up(&self, need_id: &str, after: Option<&str>) -> Caught {
        let read = self.read_redis_catch_up(need_id, after);
        match tokio::time::timeout(REPLY_REDIS_BUDGET, read).await {
            Ok(Ok(mut caught)) => {
                self.merge_local(&mut caught, need_id, after);
                caught
            }
            Ok(Err(error)) => {
                tracing::warn!("reply stream catch-up failed: {error:#}");
                self.catch_up_from_memory_and_refresh(need_id, after)
            }
            Err(elapsed) => {
                tracing::warn!("reply stream catch-up timed out ({elapsed})");
                self.catch_up_from_memory_and_refresh(need_id, after)
            }
        }
    }

    fn catch_up_from_memory_and_refresh(&self, need_id: &str, after: Option<&str>) -> Caught {
        let mut caught = self.memory_catch_up(need_id, after);
        caught.refresh = true;
        caught
    }

    fn merge_local(&self, caught: &mut Caught, need_id: &str, after: Option<&str>) {
        if caught.refresh {
            return;
        }
        let local = self.memory_catch_up(need_id, after);
        for notice in local.matching {
            if caught
                .matching
                .iter()
                .any(|existing| existing.reply_id == notice.reply_id)
            {
                advance_cursor(&mut caught.cursor, &notice.stream_id);
                continue;
            }
            advance_cursor(&mut caught.cursor, &notice.stream_id);
            caught.matching.push(notice);
        }
    }

    fn reads_redis(&self) -> bool {
        matches!(self.inner.remote, Remote::Redis(_))
    }

    async fn read_remote_after(&self, cursor: Option<&str>) -> Vec<ReplyNotice> {
        if !self.reads_redis() {
            return Vec::new();
        }
        let start = match cursor {
            Some(cursor) => format!("({cursor}"),
            None => "-".to_owned(),
        };
        let read = async {
            let mut connection = self.connection().await?;
            xrange(&mut connection, &start, "+", None).await
        };
        match tokio::time::timeout(REPLY_REDIS_BUDGET, read).await {
            Ok(Ok(notices)) => notices,
            Ok(Err(error)) => {
                self.clear_connection();
                tracing::warn!("reply stream read failed: {error:#}");
                Vec::new()
            }
            Err(elapsed) => {
                self.clear_connection();
                tracing::warn!("reply stream read timed out ({elapsed})");
                Vec::new()
            }
        }
    }

    async fn read_redis_catch_up(
        &self,
        need_id: &str,
        after: Option<&str>,
    ) -> anyhow::Result<Caught> {
        let mut connection = self.connection().await?;
        let oldest = xrange(&mut connection, "-", "+", Some(1)).await?;
        if let Some(after) = after {
            let trimmed = match oldest.first() {
                None => true,
                Some(first) => history_was_trimmed(after, &first.stream_id),
            };
            if trimmed {
                let tip = newest_id(&mut connection).await?;
                return Ok(Caught {
                    refresh: true,
                    matching: Vec::new(),
                    cursor: tip,
                });
            }
            let rows = xrange(&mut connection, &format!("({after}"), "+", None).await?;
            return Ok(caught_from_notices(need_id, Some(after), rows.iter()));
        }
        let rows = xrange(&mut connection, "-", "+", None).await?;
        Ok(caught_from_notices(need_id, None, rows.iter()))
    }

    async fn xadd(&self, need_id: &str, reply_id: &str) -> anyhow::Result<String> {
        let write = async {
            let mut connection = self.connection().await?;
            let mut command = redis::cmd("XADD");
            command
                .arg(REPLY_STREAM)
                .arg("MAXLEN")
                .arg("~")
                .arg(REPLY_STREAM_LIMIT)
                .arg("*");
            for (field, value) in reply_fields(need_id, reply_id) {
                command.arg(field).arg(value);
            }
            let value: redis::Value = command.query_async(&mut connection).await?;
            redis_text(&value).ok_or_else(|| anyhow!("reply stream id was empty"))
        };
        match tokio::time::timeout(REPLY_REDIS_BUDGET, write).await {
            Ok(result) => {
                if result.is_err() {
                    self.clear_connection();
                }
                result
            }
            Err(elapsed) => {
                self.clear_connection();
                Err(anyhow!("timed out ({elapsed})"))
            }
        }
    }

    async fn connection(&self) -> anyhow::Result<redis::aio::MultiplexedConnection> {
        if let Some(connection) = self.cached_connection() {
            return Ok(connection);
        }
        let client = self.redis_client()?;
        let connection = client.get_multiplexed_async_connection().await?;
        let mut slot = self.inner.connection.lock().expect("reply connection");
        if let Some(existing) = slot.as_ref() {
            return Ok(existing.clone());
        }
        *slot = Some(connection.clone());
        Ok(connection)
    }

    fn cached_connection(&self) -> Option<redis::aio::MultiplexedConnection> {
        match self
            .inner
            .connection
            .lock()
            .expect("reply connection")
            .as_ref()
        {
            Some(connection) => Some(connection.clone()),
            None => None,
        }
    }

    fn clear_connection(&self) {
        let mut slot = self.inner.connection.lock().expect("reply connection");
        *slot = None;
    }

    fn redis_client(&self) -> anyhow::Result<&redis::Client> {
        match &self.inner.remote {
            Remote::Redis(client) => Ok(client),
            Remote::Absent => Err(anyhow!("reply stream has no redis server")),
            #[cfg(test)]
            Remote::Failing => Err(anyhow!("reply stream redis is unavailable")),
        }
    }

    #[cfg(test)]
    fn recorded(&self) -> Vec<ReplyNotice> {
        let log = self.inner.log.lock().expect("reply hub");
        let mut snapshot = Vec::with_capacity(log.entries.len());
        for entry in &log.entries {
            snapshot.push(entry.clone());
        }
        snapshot
    }

    #[cfg(test)]
    fn failures(&self) -> u64 {
        self.inner.failures.load(Ordering::Relaxed)
    }
}

impl MemoryLog {
    fn mint(&mut self) -> String {
        let now = unix_ms();
        if now > self.last_ms {
            self.last_ms = now;
            self.seq = 0;
        } else {
            match self.seq.checked_add(1) {
                Some(next) => self.seq = next,
                None => {
                    self.last_ms = self.last_ms.saturating_add(1);
                    self.seq = 0;
                }
            }
        }
        format!("{}-{}", self.last_ms, self.seq)
    }
}

impl WatchState {
    fn prepare(sdk: &crate::story::Sdk, need_id: &str, after_stream_id: Option<String>) -> Self {
        Self {
            hub: sdk.replies.clone(),
            need_id: need_id.to_owned(),
            cursor: after_stream_id,
            phase: Phase::Start,
        }
    }

    async fn step(mut state: Self) -> Option<(WatchEvent, Self)> {
        let event = state.next_event().await?;
        Some((event, state))
    }

    async fn next_event(&mut self) -> Option<WatchEvent> {
        if let Some(event) = self.pop_queued() {
            return Some(event);
        }
        if matches!(&self.phase, Phase::Start) {
            self.prime().await;
            if let Some(event) = self.pop_queued() {
                return Some(event);
            }
        }
        self.wait_live().await
    }

    fn pop_queued(&mut self) -> Option<WatchEvent> {
        let Phase::Ready { queued, .. } = &mut self.phase else {
            return None;
        };
        queued.pop_front()
    }

    async fn prime(&mut self) {
        let rx = self.hub.subscribe();
        let caught = self
            .hub
            .catch_up(&self.need_id, self.cursor.as_deref())
            .await;
        let mut queued = VecDeque::new();
        if caught.refresh {
            queued.push_back(WatchEvent::Refresh);
        }
        for notice in caught.matching {
            queued.push_back(WatchEvent::Reply(notice));
        }
        if let Some(cursor) = caught.cursor {
            self.cursor = Some(cursor);
        }
        self.phase = Phase::Ready { queued, rx };
    }

    async fn wait_live(&mut self) -> Option<WatchEvent> {
        let hub = self.hub.clone();
        let need_id = self.need_id.clone();
        let Phase::Ready { queued, rx } = &mut self.phase else {
            return None;
        };
        wait_for_live(&hub, &need_id, &mut self.cursor, queued, rx).await
    }
}

async fn wait_for_live(
    hub: &ReplyHub,
    need_id: &str,
    cursor: &mut Option<String>,
    queued: &mut VecDeque<WatchEvent>,
    rx: &mut tokio::sync::broadcast::Receiver<ReplyNotice>,
) -> Option<WatchEvent> {
    loop {
        if let Some(event) = queued.pop_front() {
            return Some(event);
        }
        tokio::select! {
            biased;
            incoming = rx.recv() => {
                match incoming {
                    Ok(notice) => {
                        if let Some(event) = accept(cursor, need_id, notice) {
                            return Some(event);
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!("reply watch missed {skipped} notices");
                        return Some(WatchEvent::Refresh);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                }
            }
            () = remote_pause(hub) => {
                let batch = hub.read_remote_after(cursor.as_deref()).await;
                for notice in batch {
                    if let Some(event) = accept(cursor, need_id, notice) {
                        queued.push_back(event);
                    }
                }
                match drain_hub(cursor, need_id, queued, rx) {
                    HubDrain::Open => {}
                    HubDrain::Closed => {
                        if queued.is_empty() {
                            return None;
                        }
                    }
                }
            }
        }
    }
}

fn drain_hub(
    cursor: &mut Option<String>,
    need_id: &str,
    queued: &mut VecDeque<WatchEvent>,
    rx: &mut tokio::sync::broadcast::Receiver<ReplyNotice>,
) -> HubDrain {
    loop {
        match rx.try_recv() {
            Ok(notice) => {
                if let Some(event) = accept(cursor, need_id, notice) {
                    queued.push_back(event);
                }
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => return HubDrain::Open,
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(skipped)) => {
                tracing::warn!("reply watch missed {skipped} notices");
                queued.push_back(WatchEvent::Refresh);
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Closed) => return HubDrain::Closed,
        }
    }
}

fn accept(cursor: &mut Option<String>, need_id: &str, notice: ReplyNotice) -> Option<WatchEvent> {
    if let Some(current) = cursor.as_deref() {
        match id_order(&notice.stream_id, current) {
            IdOrder::After => {}
            IdOrder::Before | IdOrder::Same => return None,
            IdOrder::Unreadable => {
                tracing::warn!(
                    "reply stream id {} could not be ordered against {current}",
                    notice.stream_id
                );
                *cursor = Some(notice.stream_id);
                return Some(WatchEvent::Refresh);
            }
        }
    }
    let matches_need = notice.need_id == need_id;
    *cursor = Some(notice.stream_id.clone());
    if matches_need {
        Some(WatchEvent::Reply(notice))
    } else {
        None
    }
}

async fn remote_pause(hub: &ReplyHub) {
    if hub.reads_redis() {
        tokio::time::sleep(REMOTE_POLL).await;
    } else {
        std::future::pending::<()>().await;
    }
}

fn caught_from_notices<'a>(
    need_id: &str,
    after: Option<&str>,
    notices: impl Iterator<Item = &'a ReplyNotice>,
) -> Caught {
    let mut matching = Vec::new();
    let mut cursor = after.map(str::to_owned);
    for notice in notices {
        if let Some(after) = after {
            match id_order(&notice.stream_id, after) {
                IdOrder::After => {}
                IdOrder::Before | IdOrder::Same => continue,
                IdOrder::Unreadable => {
                    tracing::warn!(
                        "reply stream id {} could not be ordered against {after}",
                        notice.stream_id
                    );
                    cursor = Some(notice.stream_id.clone());
                    continue;
                }
            }
        }
        cursor = Some(notice.stream_id.clone());
        if notice.need_id == need_id {
            matching.push(notice.clone());
        }
    }
    Caught {
        refresh: false,
        matching,
        cursor,
    }
}

fn advance_cursor(cursor: &mut Option<String>, stream_id: &str) {
    match cursor.as_deref() {
        None => *cursor = Some(stream_id.to_owned()),
        Some(current) => match id_order(stream_id, current) {
            IdOrder::After | IdOrder::Unreadable => *cursor = Some(stream_id.to_owned()),
            IdOrder::Before | IdOrder::Same => {}
        },
    }
}

fn history_was_trimmed(after: &str, oldest: &str) -> bool {
    match id_order(after, oldest) {
        IdOrder::Before | IdOrder::Unreadable => true,
        IdOrder::Same | IdOrder::After => false,
    }
}

fn id_order(left: &str, right: &str) -> IdOrder {
    if left == right {
        return IdOrder::Same;
    }
    let Some(left) = parse_stream_id(left) else {
        return IdOrder::Unreadable;
    };
    let Some(right) = parse_stream_id(right) else {
        return IdOrder::Unreadable;
    };
    match left.cmp(&right) {
        std::cmp::Ordering::Less => IdOrder::Before,
        std::cmp::Ordering::Equal => IdOrder::Same,
        std::cmp::Ordering::Greater => IdOrder::After,
    }
}

fn parse_stream_id(id: &str) -> Option<(u64, u64)> {
    let (millis, seq) = id.split_once('-')?;
    let millis = millis.parse().ok()?;
    let seq = seq.parse().ok()?;
    Some((millis, seq))
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn reply_fields<'a>(need_id: &'a str, reply_id: &'a str) -> [(&'static str, &'a str); 2] {
    [("need_id", need_id), ("reply_id", reply_id)]
}

fn remote_from_environment() -> Remote {
    #[cfg(test)]
    {
        Remote::Absent
    }
    #[cfg(not(test))]
    {
        let url = std::env::var("UPSTASH_REDIS_URL").ok();
        remote_from_url(url.as_deref())
    }
}

fn remote_from_url(url: Option<&str>) -> Remote {
    let Some(url) = url.filter(|url| !url.is_empty()) else {
        return Remote::Absent;
    };
    match redis::Client::open(url) {
        Ok(client) => Remote::Redis(client),
        Err(error) => {
            tracing::warn!("reply stream redis url was refused ({:?})", error.kind());
            Remote::Absent
        }
    }
}

async fn xrange(
    connection: &mut redis::aio::MultiplexedConnection,
    start: &str,
    end: &str,
    count: Option<i64>,
) -> anyhow::Result<Vec<ReplyNotice>> {
    let mut command = redis::cmd("XRANGE");
    command.arg(REPLY_STREAM).arg(start).arg(end);
    if let Some(count) = count {
        command.arg("COUNT").arg(count);
    }
    let value: redis::Value = command.query_async(connection).await?;
    Ok(notices_from_value(value))
}

async fn newest_id(
    connection: &mut redis::aio::MultiplexedConnection,
) -> anyhow::Result<Option<String>> {
    let mut command = redis::cmd("XREVRANGE");
    command
        .arg(REPLY_STREAM)
        .arg("+")
        .arg("-")
        .arg("COUNT")
        .arg(1_i64);
    let value: redis::Value = command.query_async(connection).await?;
    let notices = notices_from_value(value);
    Ok(notices.into_iter().next().map(|notice| notice.stream_id))
}

fn notices_from_value(value: redis::Value) -> Vec<ReplyNotice> {
    let value = unwrap_attribute(value);
    let redis::Value::Array(rows) = value else {
        if matches!(value, redis::Value::Nil) {
            return Vec::new();
        }
        tracing::warn!("reply stream range had an unexpected shape: {value:?}");
        return Vec::new();
    };
    let mut notices = Vec::new();
    for row in rows {
        if let Some(notice) = notice_from_entry(row) {
            notices.push(notice);
        }
    }
    notices
}

fn notice_from_entry(entry: redis::Value) -> Option<ReplyNotice> {
    let entry = unwrap_attribute(entry);
    let redis::Value::Array(mut parts) = entry else {
        tracing::warn!("reply stream entry had an unexpected shape: {entry:?}");
        return None;
    };
    if parts.len() < 2 {
        tracing::warn!("reply stream entry was missing its id or fields");
        return None;
    }
    let fields = parts.pop().expect("stream entry fields");
    let id_value = parts.pop().expect("stream entry id");
    let stream_id = redis_text(&id_value)?;
    notice_from_fields(stream_id, fields_from_value(fields))
}

fn notice_from_fields(stream_id: String, fields: Vec<(String, String)>) -> Option<ReplyNotice> {
    let mut need_id = None;
    let mut reply_id = None;
    for (key, value) in fields {
        match key.as_str() {
            "need_id" => need_id = Some(value),
            "reply_id" => reply_id = Some(value),
            other => tracing::trace!("reply stream ignored field {other} ({} bytes)", value.len()),
        }
    }
    let (Some(need_id), Some(reply_id)) = (need_id, reply_id) else {
        tracing::warn!("reply stream entry {stream_id} was missing need_id or reply_id");
        return None;
    };
    Some(ReplyNotice {
        stream_id,
        need_id,
        reply_id,
    })
}

fn fields_from_value(value: redis::Value) -> Vec<(String, String)> {
    let value = unwrap_attribute(value);
    match value {
        redis::Value::Array(items) => pairs_from_flat(items),
        redis::Value::Map(items) => {
            let mut pairs = Vec::with_capacity(items.len());
            for (key, value) in items {
                let Some(key) = redis_text(&key) else {
                    tracing::trace!("reply stream skipped a non-text field name");
                    continue;
                };
                let Some(value) = redis_text(&value) else {
                    tracing::trace!("reply stream skipped a non-text value for {key}");
                    continue;
                };
                pairs.push((key, value));
            }
            pairs
        }
        other => {
            tracing::warn!("reply stream fields had an unexpected shape: {other:?}");
            Vec::new()
        }
    }
}

fn pairs_from_flat(items: Vec<redis::Value>) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut items = items.into_iter();
    loop {
        let Some(key) = items.next() else {
            break;
        };
        let Some(value) = items.next() else {
            tracing::warn!("reply stream field list ended on a key");
            break;
        };
        let Some(key) = redis_text(&key) else {
            tracing::trace!("reply stream skipped a non-text field name");
            continue;
        };
        let Some(value) = redis_text(&value) else {
            tracing::trace!("reply stream skipped a non-text value for {key}");
            continue;
        };
        pairs.push((key, value));
    }
    pairs
}

fn unwrap_attribute(value: redis::Value) -> redis::Value {
    match value {
        redis::Value::Attribute { data, attributes } => {
            if !attributes.is_empty() {
                tracing::trace!(
                    "reply stream value included {} attributes",
                    attributes.len()
                );
            }
            *data
        }
        other => other,
    }
}

fn redis_text(value: &redis::Value) -> Option<String> {
    match value {
        redis::Value::BulkString(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
        redis::Value::SimpleString(text) => Some(text.clone()),
        redis::Value::Int(number) => Some(number.to_string()),
        redis::Value::VerbatimString { format, text } => {
            tracing::trace!("reply stream read verbatim text ({format:?})");
            Some(text.clone())
        }
        redis::Value::Attribute { data, attributes } => {
            if !attributes.is_empty() {
                tracing::trace!(
                    "reply stream value included {} attributes",
                    attributes.len()
                );
            }
            redis_text(data)
        }
        other => {
            if !matches!(other, redis::Value::Nil) {
                tracing::trace!("reply stream ignored a non-text value: {other:?}");
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use futures::StreamExt;

    use super::*;
    use crate::identity::DeviceMeta;
    use crate::story::{Sdk, plant_church, post_need, register, reply_to_need};

    #[test]
    fn a_notice_carries_ids_only() {
        let fields = reply_fields("need-1", "reply-1");
        assert_eq!(fields[0], ("need_id", "need-1"));
        assert_eq!(fields[1], ("reply_id", "reply-1"));
    }

    #[test]
    fn a_redis_url_is_parsed_without_connecting() {
        let Remote::Redis(client) = remote_from_url(Some("redis://127.0.0.1:9")) else {
            panic!("redis url should parse without connecting");
        };
        let address = format!("{:?}", client.get_connection_info().addr);
        assert!(address.contains("127.0.0.1"), "{address}");
        assert!(matches!(remote_from_url(None), Remote::Absent));
        assert!(matches!(remote_from_url(Some("")), Remote::Absent));
        assert!(matches!(remote_from_url(Some("not a url")), Remote::Absent));
    }

    #[tokio::test]
    async fn publish_catch_up_filters_by_need() {
        let sdk = test_sdk().await;
        publish_committed_reply(&sdk, "need-a", "a1").await;
        publish_committed_reply(&sdk, "need-b", "b1").await;
        publish_committed_reply(&sdk, "need-a", "a2").await;

        let mut events = pinned(&sdk, "need-a", None);
        assert_eq!(
            reply_ids(&mut events, 2).await,
            ["a1".to_string(), "a2".to_string()]
        );
        idle_watch(&mut events).await;
    }

    #[tokio::test]
    async fn reconnect_after_the_last_id_skips_what_was_seen() {
        let sdk = test_sdk().await;
        publish_committed_reply(&sdk, "need-a", "a1").await;
        publish_committed_reply(&sdk, "need-b", "b1").await;
        publish_committed_reply(&sdk, "need-a", "a2").await;
        let seen = sdk.replies.recorded();
        let a1 = seen
            .iter()
            .find(|notice| notice.reply_id == "a1")
            .expect("a1");

        let mut events = pinned(&sdk, "need-a", Some(a1.stream_id.clone()));
        assert_eq!(reply_id_of(next_watch(&mut events).await), "a2");

        publish_committed_reply(&sdk, "need-b", "b2").await;
        publish_committed_reply(&sdk, "need-a", "a3").await;
        assert_eq!(reply_id_of(next_watch(&mut events).await), "a3");
        idle_watch(&mut events).await;
    }

    #[tokio::test]
    async fn trimmed_history_asks_for_a_refresh() {
        let mut sdk = test_sdk().await;
        sdk.replies = ReplyHub::memory_with_limit(2);
        publish_committed_reply(&sdk, "need-a", "a1").await;
        let expired = sdk.replies.recorded()[0].stream_id.clone();
        publish_committed_reply(&sdk, "need-a", "a2").await;
        publish_committed_reply(&sdk, "need-a", "a3").await;
        let retained = sdk.replies.recorded();
        assert_eq!(retained.len(), 2);
        assert!(retained.iter().all(|notice| notice.stream_id != expired));

        let mut events = pinned(&sdk, "need-a", Some(expired));
        assert_eq!(next_watch(&mut events).await, WatchEvent::Refresh);
        idle_watch(&mut events).await;
    }

    #[tokio::test]
    async fn a_failed_redis_publish_keeps_the_committed_reply() {
        let mut sdk = test_sdk().await;
        sdk.replies = ReplyHub::failing();
        let (viewer, need_id) = open_need(&sdk).await;
        let body = "I can bring dinner on Saturday.";
        reply_to_need(&sdk, &viewer, &need_id, body, None)
            .await
            .expect("reply story")
            .expect("reply committed");

        let stored = sdk.db.need_replies(&need_id).await.expect("replies");
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].body, body);
        assert_eq!(sdk.replies.failures(), 1);

        let mut events = pinned(&sdk, &need_id, None);
        let WatchEvent::Reply(notice) = next_watch(&mut events).await else {
            panic!("expected the committed reply on the hub");
        };
        assert_eq!(notice.need_id, need_id);
        assert_eq!(notice.reply_id, stored[0].id);
        assert!(!notice.stream_id.is_empty());
    }

    async fn test_sdk() -> Sdk {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "ecclesia-live-{}-{}-{}.db",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let db = crate::db::Db::connect(&format!("sqlite://{}", path.display()))
            .await
            .expect("test database");
        Sdk::assemble(
            db,
            crate::judge::JudgeHub::silent(),
            crate::refine::RefineHub::silent(),
            crate::push::PushHub::silent(),
            crate::cache::Cache::memory(),
        )
    }

    async fn open_need(sdk: &Sdk) -> (ecclesia_domain::Viewer, String) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let email = format!("live-{}@grace.test", NEXT.fetch_add(1, Ordering::Relaxed));
        let device = DeviceMeta {
            user_agent: String::new(),
            ip: "local".into(),
        };
        register(
            sdk,
            "Ada",
            "Lane",
            &email,
            "Thursday dinners at six oclock",
            &device,
        )
        .await
        .expect("register")
        .expect("account");
        let planted = plant_church(
            sdk,
            &sdk.db
                .user_by_email(&email)
                .await
                .expect("user")
                .expect("row"),
            "Grace Covenant",
            "100 Main Street",
            42.53,
            -92.45,
            "A church on Main Street.",
            "Sunday at 10.",
            "12-3456789",
            "IA",
            "123456",
        )
        .await
        .expect("plant")
        .expect("church");
        let church_id = planted.church_id.expect("church id");
        let user = sdk
            .db
            .user_by_email(&email)
            .await
            .expect("user")
            .expect("row");
        let viewer = sdk.viewer(user).await.expect("viewer");
        let posted = post_need(
            sdk,
            &viewer,
            &church_id,
            "Dinners for the Okonkwo family",
            "Five dinners this week.",
            None,
            "church",
        )
        .await
        .expect("post")
        .expect("need");
        (viewer, posted.need_id.expect("need id"))
    }

    fn pinned(
        sdk: &Sdk,
        need_id: &str,
        after: Option<String>,
    ) -> std::pin::Pin<Box<dyn futures::Stream<Item = WatchEvent> + Send>> {
        Box::pin(watch_need(sdk, need_id, after))
    }

    async fn next_watch(
        events: &mut std::pin::Pin<Box<dyn futures::Stream<Item = WatchEvent> + Send>>,
    ) -> WatchEvent {
        match tokio::time::timeout(Duration::from_secs(2), events.next()).await {
            Ok(Some(event)) => event,
            Ok(None) => panic!("reply watch ended"),
            Err(elapsed) => panic!("timed out waiting for a reply event ({elapsed})"),
        }
    }

    async fn idle_watch(
        events: &mut std::pin::Pin<Box<dyn futures::Stream<Item = WatchEvent> + Send>>,
    ) {
        match tokio::time::timeout(Duration::from_millis(200), events.next()).await {
            Err(elapsed) => {
                let detail = elapsed.to_string();
                assert!(
                    detail.contains("elapsed"),
                    "watch should wait for the next reply ({detail})"
                );
            }
            Ok(event) => panic!("watch should wait for the next reply, got {event:?}"),
        }
    }

    async fn reply_ids(
        events: &mut std::pin::Pin<Box<dyn futures::Stream<Item = WatchEvent> + Send>>,
        count: usize,
    ) -> Vec<String> {
        let mut ids = Vec::with_capacity(count);
        let mut step = 0;
        while step < count {
            ids.push(reply_id_of(next_watch(events).await));
            step += 1;
        }
        ids
    }

    fn reply_id_of(event: WatchEvent) -> String {
        match event {
            WatchEvent::Reply(notice) => notice.reply_id,
            WatchEvent::Refresh => panic!("expected a reply notice"),
        }
    }
}
