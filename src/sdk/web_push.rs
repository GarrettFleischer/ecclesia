//! VAPID + aes128gcm. Leaves never call this; the push skin does.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes128Gcm, Key, Nonce};
use anyhow::{anyhow, Context};
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use hkdf::Hkdf;
use p256::ecdh::EphemeralSecret;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use p256::elliptic_curve::rand_core::OsRng;
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::{PublicKey, SecretKey};
use sha2::Sha256;

const RS: u32 = 4096;
const DELIM: u8 = 2;

pub fn public_from_pem(pem: &str) -> anyhow::Result<String> {
    let secret = SecretKey::from_sec1_pem(pem).context("vapid pem")?;
    Ok(b64url(secret.public_key().to_sec1_bytes().as_ref()))
}

pub fn encrypt(p256dh: &str, auth: &str, payload: &[u8]) -> anyhow::Result<Vec<u8>> {
    let ua_public = decode_point(p256dh)?;
    let auth_secret = decode_short(auth)?;
    let local = EphemeralSecret::random(&mut OsRng);
    let as_public = PublicKey::from(&local);
    let as_bytes = as_public.to_encoded_point(false);
    let ua_bytes = ua_public.to_encoded_point(false);
    let shared = local.diffie_hellman(&ua_public);
    let ikm = expand_ikm(
        shared.raw_secret_bytes().as_ref(),
        &auth_secret,
        ua_bytes.as_bytes(),
        as_bytes.as_bytes(),
    )?;
    let mut salt = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut salt);
    let mut cek = [0u8; 16];
    let mut nonce = [0u8; 12];
    hkdf_expand(&salt, &ikm, b"Content-Encoding: aes128gcm\0", &mut cek)?;
    hkdf_expand(&salt, &ikm, b"Content-Encoding: nonce\0", &mut nonce)?;
    let mut plain = Vec::with_capacity(payload.len() + 1);
    plain.extend_from_slice(payload);
    plain.push(DELIM);
    let cipher = Aes128Gcm::new(Key::<Aes128Gcm>::from_slice(&cek));
    let sealed = cipher
        .encrypt(Nonce::from_slice(&nonce), plain.as_ref())
        .map_err(|error| anyhow!("aes-gcm: {error}"))?;
    Ok(frame(&salt, as_bytes.as_bytes(), &sealed))
}

pub fn vapid_jwt(pem: &str, audience: &str, exp: u64) -> anyhow::Result<String> {
    let secret = SecretKey::from_sec1_pem(pem).context("vapid pem")?;
    let signing = SigningKey::from(secret);
    let header = b64url(br#"{"typ":"JWT","alg":"ES256"}"#);
    let claims = format!(r#"{{"aud":"{audience}","exp":{exp},"sub":"mailto:admin@jmfent.com"}}"#);
    let body = b64url(claims.as_bytes());
    let mut signing_input = String::with_capacity(header.len() + body.len() + 1);
    signing_input.push_str(&header);
    signing_input.push('.');
    signing_input.push_str(&body);
    let signature: Signature = signing.sign(signing_input.as_bytes());
    signing_input.push('.');
    signing_input.push_str(&b64url(&signature.to_bytes()));
    Ok(signing_input)
}

pub fn audience(endpoint: &str) -> anyhow::Result<String> {
    let rest = endpoint
        .split_once("://")
        .map(|(_, rest)| rest)
        .ok_or_else(|| anyhow!("push endpoint is missing a scheme"))?;
    let host = rest
        .split('/')
        .next()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| anyhow!("push endpoint is missing a host"))?;
    let scheme = endpoint
        .split_once("://")
        .map(|(scheme, _)| scheme)
        .unwrap_or("https");
    Ok(format!("{scheme}://{host}"))
}

fn expand_ikm(
    ecdh: &[u8],
    auth: &[u8],
    ua_public: &[u8],
    as_public: &[u8],
) -> anyhow::Result<[u8; 32]> {
    let mut info = Vec::with_capacity(14 + ua_public.len() + as_public.len());
    info.extend_from_slice(b"WebPush: info\0");
    info.extend_from_slice(ua_public);
    info.extend_from_slice(as_public);
    let mut ikm = [0u8; 32];
    hkdf_expand(auth, ecdh, &info, &mut ikm)?;
    Ok(ikm)
}

fn hkdf_expand(salt: &[u8], ikm: &[u8], info: &[u8], out: &mut [u8]) -> anyhow::Result<()> {
    Hkdf::<Sha256>::new(Some(salt), ikm)
        .expand(info, out)
        .map_err(|error| anyhow!("hkdf: {error}"))
}

fn frame(salt: &[u8; 16], keyid: &[u8], sealed: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(21 + keyid.len() + sealed.len());
    out.extend_from_slice(salt);
    out.extend_from_slice(&RS.to_be_bytes());
    out.push(u8::try_from(keyid.len()).unwrap_or(65));
    out.extend_from_slice(keyid);
    out.extend_from_slice(sealed);
    out
}

fn decode_point(value: &str) -> anyhow::Result<PublicKey> {
    let bytes = decode_bytes(value)?;
    PublicKey::from_sec1_bytes(&bytes).context("p256dh")
}

fn decode_short(value: &str) -> anyhow::Result<[u8; 16]> {
    let bytes = decode_bytes(value)?;
    <[u8; 16]>::try_from(bytes.as_slice()).map_err(|_| anyhow!("auth key must be 16 bytes"))
}

fn decode_bytes(value: &str) -> anyhow::Result<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(value)
        .or_else(|_| URL_SAFE.decode(value))
        .map_err(|error| anyhow!("base64: {error}"))
}

fn b64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEV_PEM: &str = "-----BEGIN EC PRIVATE KEY-----
MHcCAQEEIPZV5ms+9lrG/t3p8EXrHFKffRogwU8SYlDfGf23AFV0oAoGCCqGSM49
AwEHoUQDQgAETfITf09T65xXoc0eyNhQXYRdIaH7AZQzW4wbvJ5oe456rDQHgLy2
kWtVDCNi2zgMdsVAHl4Q3f7K4/cvzQvWTw==
-----END EC PRIVATE KEY-----
";

    #[test]
    fn us_push_01_dev_public_matches_pem() {
        let public = public_from_pem(DEV_PEM).expect("pem");
        assert_eq!(
            public,
            "BE3yE39PU-ucV6HNHsjYUF2EXSGh-wGUM1uMG7yeaHuOeqw0B4C8tpFrVQwjYts4DHbFQB5eEN3-yuP3L80L1k8"
        );
    }

    #[test]
    fn us_push_01_vapid_jwt_has_three_parts() {
        let token = vapid_jwt(DEV_PEM, "https://push.example", 1_900_000_000).expect("jwt");
        assert_eq!(token.split('.').count(), 3);
    }

    #[test]
    fn us_push_01_audience_is_the_origin() {
        assert_eq!(
            audience("https://fcm.googleapis.com/fcm/send/abc").expect("aud"),
            "https://fcm.googleapis.com"
        );
    }

    #[test]
    fn us_push_01_aes128gcm_frame_has_a_header() {
        let ua = EphemeralSecret::random(&mut OsRng);
        let point = PublicKey::from(&ua).to_encoded_point(false);
        let p256dh = b64url(point.as_bytes());
        let auth = b64url(&[7u8; 16]);
        let body = encrypt(&p256dh, &auth, br#"{"title":"Hi"}"#).expect("encrypt");
        assert!(body.len() > 21 + 65);
        assert_eq!(body[16..20], RS.to_be_bytes());
        assert_eq!(body[20], 65);
    }
}
