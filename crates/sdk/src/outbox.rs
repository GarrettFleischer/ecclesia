//! Worker loop. Claims outbox rows and talks to phones.

use crate::clock::now_iso;
use crate::db::{Db, OutboxFinish, OutboxRow};
use crate::push::PushHub;

#[derive(serde::Deserialize)]
struct PushPayload {
    user_id: String,
    title: String,
    body: String,
    href: String,
}

pub async fn run_worker(db: &Db, push: &PushHub) -> anyhow::Result<()> {
    loop {
        poll_once(db, push).await;
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

pub async fn poll_once(db: &Db, push: &PushHub) {
    let now = now_iso();
    match db.outbox_lag_seconds(&now).await {
        Ok(Some(lag)) => {
            tracing::info!(ecclesia_outbox_lag_seconds = lag, "outbox lag");
        }
        Ok(None) => {}
        Err(error) => tracing::warn!("outbox lag failed: {error:#}"),
    }
    let rows = match db.claim_outbox(&now).await {
        Ok(rows) => rows,
        Err(error) => {
            tracing::error!("outbox claim failed: {error:#}");
            return;
        }
    };
    for row in rows {
        deliver_row(db, push, row).await;
    }
}

async fn deliver_row(db: &Db, push: &PushHub, row: OutboxRow) {
    tracing::info!(outbox_id = %row.id, kind = %row.kind, "outbox claim");
    let result = match row.kind.as_str() {
        "push" => deliver_push(db, push, &row).await,
        "mail" => deliver_mail(&row).await,
        other => {
            tracing::warn!(outbox_id = %row.id, kind = other, "outbox kind skipped");
            Ok(())
        }
    };
    match result {
        Ok(()) => {
            if let Err(error) = db.finish_outbox(&row.id, OutboxFinish::Success).await {
                tracing::error!(outbox_id = %row.id, "outbox done failed: {error:#}");
            } else {
                tracing::info!(outbox_id = %row.id, "outbox success");
            }
        }
        Err(error) => {
            tracing::warn!(outbox_id = %row.id, "outbox retry: {error:#}");
            if let Err(finish_error) = db.finish_outbox(&row.id, OutboxFinish::Failed).await {
                tracing::error!(outbox_id = %row.id, "outbox fail write: {finish_error:#}");
            }
        }
    }
}

async fn deliver_push(db: &Db, push: &PushHub, row: &OutboxRow) -> anyhow::Result<()> {
    let payload = serde_json::from_str::<PushPayload>(&row.payload)
        .map_err(|error| anyhow::anyhow!("outbox payload was not a notice: {error}"))?;
    push.deliver_text(db, &payload.user_id, &payload.title, &payload.body, &payload.href)
        .await
}

#[derive(serde::Deserialize)]
struct MailPayload {
    to: String,
    subject: String,
    text: String,
}

const RESEND_URL: &str = "https://api.resend.com/emails";

async fn deliver_mail(row: &OutboxRow) -> anyhow::Result<()> {
    let env = crate::host::load_mail_env("http://127.0.0.1:3000")?;
    post_mail(row, &env, RESEND_URL).await
}

async fn post_mail(
    row: &OutboxRow,
    env: &crate::host::MailEnv,
    url: &str,
) -> anyhow::Result<()> {
    let (Some(key), Some(from)) = (env.api_key.as_deref(), env.from.as_deref()) else {
        return Ok(());
    };
    let payload: MailPayload = serde_json::from_str(&row.payload)?;
    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(key)
        .header("Idempotency-Key", format!("mail/{}", row.id))
        .json(&serde_json::json!({
            "from": from,
            "to": [payload.to],
            "subject": payload.subject,
            "text": payload.text,
        }))
        .send()
        .await?;
    if !response.status().is_success() {
        anyhow::bail!("resend {}", response.status());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::OutboxRow;
    use crate::host::{HostKind, mail_env_from};
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[tokio::test]
    async fn us_mail_07_posts_idempotency_key() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("hub bind");
        let addr = listener.local_addr().expect("hub addr");
        let hub = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("hub accept");
            let mut buf = vec![0u8; 8192];
            let n = stream.read(&mut buf).expect("hub read");
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
            String::from_utf8_lossy(&buf[..n]).into_owned()
        });
        let row = OutboxRow {
            id: "ob-mail-1".into(),
            kind: "mail".into(),
            payload: r#"{"to":"ada@verify.test","subject":"Your sign in link","text":"Open this."}"#
                .into(),
            attempts: 0,
            available_at: "2026-09-22T00:00:00Z".into(),
            status: "pending".into(),
            dead_at: None,
        };
        let env = mail_env_from(
            HostKind::Local,
            Some("re_test".into()),
            Some("Ecclesia <mail@ecclesia.test>".into()),
            None,
            "http://127.0.0.1:3000",
        )
        .expect("mail env");
        post_mail(&row, &env, &format!("http://{addr}/emails"))
            .await
            .expect("post mail");
        let recorded = hub.join().expect("hub join");
        let lower = recorded.to_ascii_lowercase();
        assert!(
            lower.contains("idempotency-key: mail/ob-mail-1"),
            "hub request was {recorded}"
        );
        assert!(lower.contains("authorization: bearer re_test"), "hub request was {recorded}");
        assert!(recorded.contains("ada@verify.test"), "hub request was {recorded}");
        assert!(recorded.contains("Your sign in link"), "hub request was {recorded}");
        assert!(
            recorded.contains("Ecclesia <mail@ecclesia.test>"),
            "hub request was {recorded}"
        );
    }
}
