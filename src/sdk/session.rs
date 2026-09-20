use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const COOKIE: &str = "ecclesia_sid";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub user_id: Option<String>,
    pub csrf: String,
}

impl Session {
    pub fn guest(csrf: String) -> Self {
        Self {
            user_id: None,
            csrf,
        }
    }

    pub fn signed_in(user_id: String, csrf: String) -> Self {
        Self {
            user_id: Some(user_id),
            csrf,
        }
    }

    pub fn encode(&self, secret: &str) -> String {
        let user = self.user_id.as_deref().unwrap_or("");
        let payload = format!("{user}.{}", self.csrf);
        let mac = sign(secret, &payload);
        format!("v1.{payload}.{mac}")
    }

    pub fn decode(secret: &str, raw: &str) -> Option<Self> {
        let rest = raw.strip_prefix("v1.")?;
        let (payload, mac) = rest.rsplit_once('.')?;
        if !verify(secret, payload, mac) {
            return None;
        }
        let (user, csrf) = payload.split_once('.')?;
        if csrf.len() != 32 || !csrf.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        Some(Self {
            user_id: if user.is_empty() {
                None
            } else {
                Some(user.to_string())
            },
            csrf: csrf.to_string(),
        })
    }

    pub fn check_csrf(&self, submitted: &str) -> bool {
        !submitted.is_empty() && constant_eq(&self.csrf, submitted)
    }
}

/// 16 bytes from a UUID → 32 hex chars.
pub fn fresh_csrf() -> String {
    hex::encode(uuid::Uuid::new_v4().as_bytes())
}

pub fn from_jar(secret: &str, jar: &CookieJar) -> Session {
    jar.get(COOKIE)
        .and_then(|cookie| Session::decode(secret, cookie.value()))
        .unwrap_or_else(|| Session::guest(fresh_csrf()))
}

pub fn put(jar: CookieJar, secret: &str, session: &Session) -> CookieJar {
    let cookie = Cookie::build((COOKIE, session.encode(secret)))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(cookie::time::Duration::days(30))
        .build();
    jar.add(cookie)
}

pub fn clear(jar: CookieJar) -> CookieJar {
    let cookie = Cookie::build((COOKIE, ""))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(cookie::time::Duration::seconds(0))
        .build();
    jar.add(cookie)
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
            "user_miriam".into(),
            "aabbccddeeff00112233445566778899".into(),
        );
        let raw = session.encode("secret");
        assert_eq!(Session::decode("secret", &raw), Some(session.clone()));
        assert_eq!(Session::decode("other", &raw), None);
        let tampered = raw.replace("user_miriam", "user_peter");
        assert_eq!(Session::decode("secret", &tampered), None);
    }

    #[test]
    fn us_sec_02_csrf_must_match() {
        let session = Session::guest("aabbccddeeff00112233445566778899".into());
        assert!(session.check_csrf("aabbccddeeff00112233445566778899"));
        assert!(!session.check_csrf("bbccddeeff00112233445566778899aa"));
        assert!(!session.check_csrf(""));
    }
}
