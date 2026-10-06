//! Short lived access bearers and refresh wire strings for JSON clients.

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::password::{hash_token, mint_token};

type HmacSha256 = Hmac<Sha256>;

pub const ACCESS_SECONDS: i64 = 900;
const SESSION_ID_MAX: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiSessionTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

pub fn mint_refresh_wire() -> String {
    format!("rt.{}", mint_token())
}

pub fn hash_refresh_wire(wire: &str) -> String {
    hash_token(wire)
}

pub fn encode_access(secret: &str, session_id: &str, exp_unix: i64) -> String {
    let payload = format!("at.{session_id}.{exp_unix}");
    let mac = sign(secret, &payload);
    format!("v3.at.{session_id}.{exp_unix}.{mac}")
}

pub fn decode_access(secret: &str, raw: &str) -> Option<(String, i64)> {
    let body = raw.strip_prefix("v3.at.")?;
    let (head, mac) = body.rsplit_once('.')?;
    let (session_id, exp_str) = head.rsplit_once('.')?;
    if !session_id_ok(session_id) {
        return None;
    }
    let exp_unix = exp_str.parse::<i64>().ok()?;
    let payload = format!("at.{session_id}.{exp_unix}");
    if !verify(secret, &payload, mac) {
        return None;
    }
    Some((session_id.to_string(), exp_unix))
}

pub fn access_exp_unix(now_unix: i64) -> i64 {
    now_unix + ACCESS_SECONDS
}

fn session_id_ok(value: &str) -> bool {
    (1..=SESSION_ID_MAX).contains(&value.len())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && !value.contains('.')
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_api_01_access_round_trips() {
        let exp = 1_700_000_000_i64;
        let raw = encode_access("secret", "sess_1", exp);
        let (id, got) = decode_access("secret", &raw).expect("decode");
        assert_eq!(id, "sess_1");
        assert_eq!(got, exp);
        assert_eq!(
            decode_access("secret", &raw.replace("sess_1", "sess_2")),
            None
        );
    }

    #[test]
    fn us_api_01_dotted_session_id_rejected() {
        let exp = 1_700_000_000_i64;
        let raw = encode_access("secret", "sess.one", exp);
        assert!(decode_access("secret", &raw).is_none());
    }
}
