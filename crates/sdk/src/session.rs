use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const COOKIE: &str = "ecclesia_sid";
const CSRF_LEN: usize = 32;
const USER_ID_MAX: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieTransport {
    Plain,
    Secure,
}

impl CookieTransport {
    pub fn from_env_value(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("1") => Self::Secure,
            Some(value) if value.eq_ignore_ascii_case("true") => Self::Secure,
            _ => Self::Plain,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub session_id: Option<String>,
    pub user_id: Option<String>,
    pub csrf: String,
}

impl Session {
    pub fn guest(csrf: String) -> Self {
        Self {
            session_id: None,
            user_id: None,
            csrf,
        }
    }

    pub fn signed_in(session_id: String, user_id: String, csrf: String) -> Self {
        Self {
            session_id: Some(session_id),
            user_id: Some(user_id),
            csrf,
        }
    }

    pub fn encode(&self, secret: &str) -> String {
        let sid = self.session_id.as_deref().unwrap_or("");
        let payload = format!("{sid}.{}", self.csrf);
        let mac = sign(secret, &payload);
        format!("v2.{payload}.{mac}")
    }

    pub fn decode(secret: &str, raw: &str) -> Option<Self> {
        let rest = raw.strip_prefix("v2.")?;
        let (payload, mac) = rest.rsplit_once('.')?;
        if !verify(secret, payload, mac) {
            return None;
        }
        let (sid, csrf) = payload.split_once('.')?;
        if !csrf_ok(csrf) {
            return None;
        }
        if sid.is_empty() {
            return Some(Self::guest(csrf.to_string()));
        }
        if !id_ok(sid) {
            return None;
        }
        Some(Self {
            session_id: Some(sid.to_string()),
            user_id: None,
            csrf: csrf.to_string(),
        })
    }

    pub fn check_csrf(&self, submitted: &str) -> bool {
        !submitted.is_empty() && constant_eq(&self.csrf, submitted)
    }
}

pub fn fresh_csrf() -> String {
    hex::encode(uuid::Uuid::new_v4().as_bytes())
}

pub fn mint_secret() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn from_jar(secret: &str, jar: &CookieJar) -> JarSession {
    match jar
        .get(COOKIE)
        .and_then(|cookie| Session::decode(secret, cookie.value()))
    {
        Some(session) => JarSession::Known(session),
        None => JarSession::Minted(Session::guest(fresh_csrf())),
    }
}

pub enum JarSession {
    Known(Session),
    Minted(Session),
}

pub fn put(
    jar: CookieJar,
    secret: &str,
    session: &Session,
    transport: CookieTransport,
) -> CookieJar {
    jar.add(build_cookie(
        session.encode(secret),
        cookie::time::Duration::days(30),
        transport,
    ))
}

pub fn clear(jar: CookieJar, transport: CookieTransport) -> CookieJar {
    jar.add(build_cookie(
        String::new(),
        cookie::time::Duration::seconds(0),
        transport,
    ))
}

fn build_cookie(
    value: String,
    max_age: cookie::time::Duration,
    transport: CookieTransport,
) -> Cookie<'static> {
    let builder = Cookie::build((COOKIE, value))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(max_age);
    match transport {
        CookieTransport::Plain => builder.build(),
        CookieTransport::Secure => builder.secure(true).build(),
    }
}

fn sign(secret: &str, payload: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(payload.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn verify(secret: &str, payload: &str, mac_hex: &str) -> bool {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(payload.as_bytes());
    match hex::decode(mac_hex) {
        Ok(bytes) => mac.verify_slice(&bytes).is_ok(),
        Err(_) => false,
    }
}

fn csrf_ok(csrf: &str) -> bool {
    csrf.len() == CSRF_LEN && csrf.chars().all(|c| c.is_ascii_hexdigit())
}

fn id_ok(value: &str) -> bool {
    (1..=USER_ID_MAX).contains(&value.len())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn constant_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_sec_01_session_round_trips_and_rejects_tampering() {
        let session = Session::signed_in(
            "sess_1".into(),
            "user_miriam".into(),
            "aabbccddeeff00112233445566778899".into(),
        );
        let raw = session.encode("secret");
        let decoded = Session::decode("secret", &raw).expect("decode");
        assert_eq!(decoded.session_id.as_deref(), Some("sess_1"));
        assert_eq!(decoded.csrf, session.csrf);
        assert_eq!(Session::decode("other", &raw), None);
        let tampered = raw.replace("sess_1", "sess_2");
        assert_eq!(Session::decode("secret", &tampered), None);
    }

    #[test]
    fn us_sec_01_guest_has_empty_session_id() {
        let session = Session::guest("aabbccddeeff00112233445566778899".into());
        let raw = session.encode("secret");
        assert_eq!(Session::decode("secret", &raw), Some(session));
    }

    #[test]
    fn us_sec_01_dotted_session_id_cannot_hide_in_the_cookie() {
        let session = Session::signed_in(
            "sess.one".into(),
            "user_miriam".into(),
            "aabbccddeeff00112233445566778899".into(),
        );
        let raw = session.encode("secret");
        assert_eq!(Session::decode("secret", &raw), None);
    }

    #[test]
    fn us_sec_02_csrf_must_match() {
        let session = Session::guest("aabbccddeeff00112233445566778899".into());
        assert!(session.check_csrf("aabbccddeeff00112233445566778899"));
        assert!(!session.check_csrf("bbccddeeff00112233445566778899aa"));
        assert!(!session.check_csrf(""));
    }

    #[test]
    fn us_sec_01_minted_secret_is_long() {
        assert_eq!(mint_secret().len(), 64);
        assert_ne!(mint_secret(), mint_secret());
    }
}
