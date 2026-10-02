//! Register, sign in, sessions, and mail tokens.

use chrono::{Duration, SecondsFormat, Utc};
use ecclesia_domain::{
    DomainError, EmailAvailability, Strength, User, Viewer, accept_password, display_name,
    invite_member as domain_invite_member, normalize_email, parse_invite_email,
    register as domain_register, VoiceKind,
};

use crate::bearer::{
    access_exp_unix, decode_access, encode_access, hash_refresh_wire, mint_refresh_wire,
    ApiSessionTokens, ACCESS_SECONDS,
};
use serde::Serialize;
use crate::clock::{new_id, now_iso};
use crate::db::{
    MailWrite, PasswordHashWrite, SessionTransport, SessionWrite, StoryExtras, TokenWrite,
};
use crate::password::{hash_and_wipe, hash_token, mint_token, score, verify_password};
use crate::session::{fresh_csrf, Session};
use crate::story::{finish_with, Sdk, StoryOk};

const MAGIC_MINUTES: i64 = 60;
const RESET_HOURS: i64 = 24;

pub struct DeviceMeta {
    pub user_agent: String,
    pub ip: String,
}

pub async fn register(
    sdk: &Sdk,
    first_name: &str,
    last_name: &str,
    email: &str,
    password: &str,
    device: &DeviceMeta,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let normalized = match normalize_email(email) {
        Ok(email) => email,
        Err(error) => return Ok(Err(error)),
    };
    let availability = EmailAvailability::of_existing(sdk.db.user_by_email(&normalized).await?);
    let posture = sdk.weigh(VoiceKind::Bio, &[first_name, last_name]).await;
    let known_as = display_name(first_name, last_name);
    let strength = score(password, &known_as, &normalized);
    let mut password = password.to_string();
    let hash = match strength {
        Strength::Acceptable => hash_and_wipe(&mut password)?,
        Strength::TooGuessable => {
            password.clear();
            String::new()
        }
    };
    let user_id = new_id();
    let now = now_iso();
    let effect = match domain_register(
        first_name,
        last_name,
        &normalized,
        availability,
        posture,
        strength,
        user_id.clone(),
        now,
    ) {
        Ok(effect) => effect,
        Err(error) => return Ok(Err(error)),
    };
    let extras = session_extras(&user_id, device, StoryExtras {
        password_hash: Some(PasswordHashWrite {
            user_id: user_id.clone(),
            hash,
        }),
        ..StoryExtras::default()
    });
    let mut ok = StoryOk::from_effect(&effect);
    ok.session_id = extras.session.as_ref().map(|row| row.id.clone());
    ok.csrf = extras.session.as_ref().map(|row| row.csrf.clone());
    sdk.commit_with(&effect, &extras).await?;
    Ok(Ok(ok))
}

pub async fn sign_in(
    sdk: &Sdk,
    email: &str,
    password: &str,
    device: &DeviceMeta,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let normalized = match normalize_email(email) {
        Ok(email) => email,
        Err(_) => return Ok(Err(DomainError::NotFound)),
    };
    let Some(user) = sdk.db.user_by_email(&normalized).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    let Some(hash) = sdk.db.user_password_hash(&user.id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    if !verify_password(password, &hash) {
        return Ok(Err(DomainError::NotFound));
    }
    mint_session_only(sdk, &user.id, device).await
}

async fn mint_session_only(
    sdk: &Sdk,
    user_id: &str,
    device: &DeviceMeta,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let extras = session_extras(user_id, device, StoryExtras::default());
    mint_from_extras(sdk, user_id, extras).await
}

async fn mint_from_extras(
    sdk: &Sdk,
    user_id: &str,
    extras: StoryExtras,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let session = extras.session.as_ref().expect("session");
    let ok = StoryOk {
        user_id: Some(user_id.to_string()),
        session_id: Some(session.id.clone()),
        csrf: Some(session.csrf.clone()),
        ..StoryOk::default()
    };
    sdk.db
        .apply_with(&ecclesia_domain::Effect::default(), &extras)
        .await?;
    Ok(Ok(ok))
}

fn session_extras(
    user_id: &str,
    device: &DeviceMeta,
    mut extras: StoryExtras,
) -> StoryExtras {
    let now = now_iso();
    extras.session = Some(cookie_session_write(user_id, device, now));
    extras
}

fn cookie_session_write(user_id: &str, device: &DeviceMeta, now: String) -> SessionWrite {
    SessionWrite {
        id: new_id(),
        user_id: user_id.to_string(),
        csrf: fresh_csrf(),
        created_at: now.clone(),
        last_seen_at: now,
        user_agent: truncate_agent(&device.user_agent),
        ip: device.ip.clone(),
        transport: SessionTransport::Cookie,
        refresh_token_hash: None,
    }
}

fn api_session_write(
    user_id: &str,
    device: &DeviceMeta,
    now: String,
    refresh_hash: String,
) -> SessionWrite {
    SessionWrite {
        id: new_id(),
        user_id: user_id.to_string(),
        csrf: fresh_csrf(),
        created_at: now.clone(),
        last_seen_at: now,
        user_agent: truncate_agent(&device.user_agent),
        ip: device.ip.clone(),
        transport: SessionTransport::Api,
        refresh_token_hash: Some(refresh_hash),
    }
}

fn tokens_for_session(secret: &str, session_id: &str, refresh_wire: &str) -> ApiSessionTokens {
    let now = Utc::now().timestamp();
    let exp = access_exp_unix(now);
    ApiSessionTokens {
        access_token: encode_access(secret, session_id, exp),
        refresh_token: refresh_wire.to_string(),
        expires_in: ACCESS_SECONDS,
    }
}

pub async fn sign_in_api(
    sdk: &Sdk,
    secret: &str,
    email: &str,
    password: &str,
    device: &DeviceMeta,
) -> anyhow::Result<Result<ApiSessionTokens, DomainError>> {
    let normalized = match normalize_email(email) {
        Ok(email) => email,
        Err(_) => return Ok(Err(DomainError::NotFound)),
    };
    let Some(user) = sdk.db.user_by_email(&normalized).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    let Some(hash) = sdk.db.user_password_hash(&user.id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    if !verify_password(password, &hash) {
        return Ok(Err(DomainError::NotFound));
    }
    mint_api_session(sdk, secret, &user.id, device).await
}

async fn mint_api_session(
    sdk: &Sdk,
    secret: &str,
    user_id: &str,
    device: &DeviceMeta,
) -> anyhow::Result<Result<ApiSessionTokens, DomainError>> {
    let refresh_wire = mint_refresh_wire();
    let refresh_hash = hash_refresh_wire(&refresh_wire);
    let now = now_iso();
    let session = api_session_write(user_id, device, now, refresh_hash);
    let session_id = session.id.clone();
    let extras = StoryExtras {
        session: Some(session),
        ..StoryExtras::default()
    };
    sdk.db
        .apply_with(&ecclesia_domain::Effect::default(), &extras)
        .await?;
    Ok(Ok(tokens_for_session(secret, &session_id, &refresh_wire)))
}

pub async fn refresh_api(
    sdk: &Sdk,
    secret: &str,
    refresh_wire: &str,
) -> anyhow::Result<Option<ApiSessionTokens>> {
    if !refresh_wire.starts_with("rt.") {
        return Ok(None);
    }
    let old_hash = hash_refresh_wire(refresh_wire);
    let Some(row) = sdk.db.session_by_refresh_hash(&old_hash).await? else {
        return Ok(None);
    };
    if SessionTransport::parse(&row.transport) != Some(SessionTransport::Api) {
        return Ok(None);
    }
    if idle(&row.last_seen_at) {
        return Ok(None);
    }
    let now = now_iso();
    let touch = if day_changed(&row.last_seen_at) {
        now.clone()
    } else {
        row.last_seen_at.clone()
    };
    let new_wire = mint_refresh_wire();
    let new_hash = hash_refresh_wire(&new_wire);
    let rotated = sdk
        .db
        .rotate_refresh_hash(&row.id, &old_hash, &new_hash, &touch)
        .await?;
    if !rotated {
        return Ok(None);
    }
    Ok(Some(tokens_for_session(secret, &row.id, &new_wire)))
}

pub struct BearerIdentity {
    pub session_id: String,
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiProfile {
    pub id: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub bio: String,
    pub church_id: Option<String>,
}

pub async fn api_me_profile(
    sdk: &Sdk,
    secret: &str,
    access_token: &str,
) -> anyhow::Result<Option<ApiProfile>> {
    let Some(who) = resolve_bearer(sdk, secret, access_token).await? else {
        return Ok(None);
    };
    let Some(user) = sdk.db.user(&who.user_id).await? else {
        return Ok(None);
    };
    Ok(Some(ApiProfile {
        id: user.id,
        first_name: user.first_name,
        last_name: user.last_name,
        email: user.email,
        bio: user.bio,
        church_id: user.church_id,
    }))
}

pub async fn api_logout_bearer(
    sdk: &Sdk,
    secret: &str,
    access_token: &str,
) -> anyhow::Result<bool> {
    let Some(who) = resolve_bearer(sdk, secret, access_token).await? else {
        return Ok(false);
    };
    logout(sdk, &who.session_id).await?;
    Ok(true)
}

pub async fn resolve_bearer(
    sdk: &Sdk,
    secret: &str,
    access_token: &str,
) -> anyhow::Result<Option<BearerIdentity>> {
    let Some((session_id, exp_unix)) = decode_access(secret, access_token) else {
        return Ok(None);
    };
    if Utc::now().timestamp() > exp_unix {
        return Ok(None);
    }
    let Some(row) = sdk.db.session(&session_id).await? else {
        return Ok(None);
    };
    if SessionTransport::parse(&row.transport) != Some(SessionTransport::Api) {
        return Ok(None);
    }
    if idle(&row.last_seen_at) {
        return Ok(None);
    }
    if day_changed(&row.last_seen_at) {
        let _ = sdk.db.touch_session(&session_id, &now_iso()).await;
    }
    Ok(Some(BearerIdentity {
        session_id: row.id,
        user_id: row.user_id,
    }))
}

pub async fn change_password(
    sdk: &Sdk,
    user: &User,
    current: &str,
    new_password: &str,
    device: &DeviceMeta,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some(hash) = sdk.db.user_password_hash(&user.id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    if !verify_password(current, &hash) {
        return Ok(Err(DomainError::NotFound));
    }
    let strength = score(
        new_password,
        &display_name(&user.first_name, &user.last_name),
        &user.email,
    );
    if let Err(error) = accept_password(strength) {
        return Ok(Err(error));
    }
    let mut new_password = new_password.to_string();
    let hash = hash_and_wipe(&mut new_password)?;
    let extras = session_extras(
        &user.id,
        device,
        StoryExtras {
            password_hash: Some(PasswordHashWrite {
                user_id: user.id.clone(),
                hash,
            }),
            delete_sessions_user: Some(user.id.clone()),
            ..StoryExtras::default()
        },
    );
    sdk.db
        .apply_with(&ecclesia_domain::Effect::default(), &extras)
        .await?;
    let session = extras.session.as_ref().expect("session");
    Ok(Ok(StoryOk {
        user_id: Some(user.id.clone()),
        session_id: Some(session.id.clone()),
        csrf: Some(session.csrf.clone()),
        ..StoryOk::default()
    }))
}

pub async fn logout(sdk: &Sdk, session_id: &str) -> anyhow::Result<()> {
    sdk.db
        .apply_with(
            &ecclesia_domain::Effect::default(),
            &StoryExtras {
                delete_session_id: Some(session_id.to_string()),
                ..StoryExtras::default()
            },
        )
        .await?;
    Ok(())
}

pub async fn logout_all(sdk: &Sdk, user_id: &str) -> anyhow::Result<()> {
    sdk.db
        .apply_with(
            &ecclesia_domain::Effect::default(),
            &StoryExtras {
                delete_sessions_user: Some(user_id.to_string()),
                ..StoryExtras::default()
            },
        )
        .await?;
    Ok(())
}

pub async fn revoke_session(
    sdk: &Sdk,
    viewer_id: &str,
    session_id: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some(row) = sdk.db.session(session_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    if row.user_id != viewer_id {
        return Ok(Err(DomainError::NotFound));
    }
    logout(sdk, session_id).await?;
    Ok(Ok(StoryOk::default()))
}

pub struct MailOrigin {
    pub origin: String,
}

pub async fn resolve_session(sdk: &Sdk, session: Session) -> anyhow::Result<Session> {
    let Some(id) = session.session_id.as_deref() else {
        return Ok(session);
    };
    let Some(row) = sdk.db.session(id).await? else {
        return Ok(Session::guest(fresh_csrf()));
    };
    if SessionTransport::parse(&row.transport) != Some(SessionTransport::Cookie) {
        return Ok(Session::guest(fresh_csrf()));
    }
    if row.csrf != session.csrf || idle(&row.last_seen_at) {
        return Ok(Session::guest(fresh_csrf()));
    }
    if day_changed(&row.last_seen_at) {
        let _ = sdk.db.touch_session(id, &now_iso()).await;
    }
    Ok(Session::signed_in(row.id, row.user_id, row.csrf))
}

fn idle(last_seen: &str) -> bool {
    let Ok(seen) = chrono::DateTime::parse_from_rfc3339(last_seen) else {
        return true;
    };
    Utc::now() > seen.with_timezone(&Utc) + Duration::days(30)
}

fn day_changed(last_seen: &str) -> bool {
    let Ok(seen) = chrono::DateTime::parse_from_rfc3339(last_seen) else {
        return true;
    };
    seen.with_timezone(&Utc).date_naive() < Utc::now().date_naive()
}

pub async fn request_magic(
    sdk: &Sdk,
    email: &str,
    origin: &MailOrigin,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    request_token(sdk, email, origin, TokenKind::Magic).await
}

pub async fn request_reset(
    sdk: &Sdk,
    email: &str,
    origin: &MailOrigin,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    request_token(sdk, email, origin, TokenKind::Reset).await
}

enum TokenKind {
    Magic,
    Reset,
}

async fn request_token(
    sdk: &Sdk,
    email: &str,
    origin: &MailOrigin,
    kind: TokenKind,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Ok(normalized) = normalize_email(email) else {
        return Ok(Ok(StoryOk::default()));
    };
    let Some(user) = sdk.db.user_by_email(&normalized).await? else {
        return Ok(Ok(StoryOk::default()));
    };
    let secret = mint_token();
    let token_id = new_id();
    let now = now_iso();
    let expires = match kind {
        TokenKind::Magic => later_minutes(MAGIC_MINUTES),
        TokenKind::Reset => later_hours(RESET_HOURS),
    };
    let token = TokenWrite {
        id: token_id.clone(),
        user_id: user.id.clone(),
        token_hash: hash_token(&secret),
        expires_at: expires,
        created_at: now,
    };
    let href = match kind {
        TokenKind::Magic => format!("{}/session/link/{token_id}?t={secret}", origin.origin),
        TokenKind::Reset => format!("{}/session/reset/{token_id}?t={secret}", origin.origin),
    };
    let (subject, lead) = match kind {
        TokenKind::Magic => ("Your sign in link", "Open this link to sign in."),
        TokenKind::Reset => ("Set a new password", "Open this link to set a new password."),
    };
    let extras = StoryExtras {
        retire_magic_user: matches!(kind, TokenKind::Magic).then(|| user.id.clone()),
        retire_reset_user: matches!(kind, TokenKind::Reset).then(|| user.id.clone()),
        insert_magic: matches!(kind, TokenKind::Magic).then(|| token.clone()),
        insert_reset: matches!(kind, TokenKind::Reset).then(|| token),
        mail: Some(MailWrite {
            to: user.email,
            subject: subject.into(),
            text: format!("{lead}\n{href}"),
        }),
        ..StoryExtras::default()
    };
    sdk.db
        .apply_with(&ecclesia_domain::Effect::default(), &extras)
        .await?;
    Ok(Ok(StoryOk::default()))
}

pub async fn consume_magic(
    sdk: &Sdk,
    id: &str,
    secret: &str,
    device: &DeviceMeta,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some(row) = sdk.db.live_magic(id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    if !token_live(&row, secret) {
        return Ok(Err(DomainError::NotFound));
    }
    let extras = session_extras(
        &row.user_id,
        device,
        StoryExtras {
            consume_magic_id: Some(id.to_string()),
            ..StoryExtras::default()
        },
    );
    mint_from_extras(sdk, &row.user_id, extras).await
}

pub async fn reset_form_ok(sdk: &Sdk, id: &str, secret: &str) -> anyhow::Result<bool> {
    let Some(row) = sdk.db.live_reset(id).await? else {
        return Ok(false);
    };
    Ok(token_live(&row, secret))
}

pub async fn complete_reset(
    sdk: &Sdk,
    id: &str,
    secret: &str,
    new_password: &str,
    device: &DeviceMeta,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some(row) = sdk.db.live_reset(id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    if !token_live(&row, secret) {
        return Ok(Err(DomainError::NotFound));
    }
    let Some(user) = sdk.db.user(&row.user_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    let strength = score(
        new_password,
        &display_name(&user.first_name, &user.last_name),
        &user.email,
    );
    if let Err(error) = accept_password(strength) {
        return Ok(Err(error));
    }
    let mut new_password = new_password.to_string();
    let hash = hash_and_wipe(&mut new_password)?;
    let extras = session_extras(
        &user.id,
        device,
        StoryExtras {
            password_hash: Some(PasswordHashWrite {
                user_id: user.id.clone(),
                hash,
            }),
            delete_sessions_user: Some(user.id.clone()),
            consume_reset_id: Some(id.to_string()),
            ..StoryExtras::default()
        },
    );
    mint_from_extras(sdk, &user.id, extras).await
}

pub async fn invite_member(
    sdk: &Sdk,
    viewer: &Viewer,
    church_id: &str,
    email: &str,
    origin: &MailOrigin,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some(church) = sdk.db.church(church_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    let email = match parse_invite_email(email) {
        Ok(email) => email,
        Err(error) => return Ok(Err(error)),
    };
    let Some(invitee) = sdk.db.user_by_email(&email).await? else {
        return Ok(Ok(StoryOk {
            church_id: Some(church.id),
            ..StoryOk::default()
        }));
    };
    let effect = domain_invite_member(viewer, &church, &invitee);
    let extras = StoryExtras {
        mail: Some(MailWrite {
            to: invitee.email,
            subject: format!("Invite to {}", church.name),
            text: format!(
                "You're invited to {}. Code {}.\n{}/churches",
                church.name, church.invite_code, origin.origin
            ),
        }),
        ..StoryExtras::default()
    };
    finish_with(sdk, effect, extras).await
}

fn token_live(row: &crate::db::TokenRow, secret: &str) -> bool {
    if row.consumed_at.is_some() {
        return false;
    }
    if row.expires_at.as_str() <= now_iso().as_str() {
        return false;
    }
    crate::password::hash_token(secret) == row.token_hash
}

fn later_minutes(minutes: i64) -> String {
    Utc::now()
        .checked_add_signed(Duration::minutes(minutes))
        .unwrap_or_else(Utc::now)
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn later_hours(hours: i64) -> String {
    Utc::now()
        .checked_add_signed(Duration::hours(hours))
        .unwrap_or_else(Utc::now)
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn truncate_agent(value: &str) -> String {
    value.chars().take(80).collect()
}
