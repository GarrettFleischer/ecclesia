//! Score and hash secrets. Domain never sees the raw value or the PHC string.

use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use ecclesia_domain::Strength;
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

const MAX_LEN: usize = 128;

pub fn score(password: &str, name: &str, email: &str) -> Strength {
    if password.is_empty() || password.chars().count() > MAX_LEN {
        return Strength::TooGuessable;
    }
    let entropy = zxcvbn::zxcvbn(password, &[name, email]);
    if u8::from(entropy.score()) < 2 {
        Strength::TooGuessable
    } else {
        Strength::Acceptable
    }
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hashed| hashed.to_string())
        .map_err(|error| anyhow::anyhow!("{error}"))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn hash_and_wipe(password: &mut String) -> anyhow::Result<String> {
    let hashed = hash_password(password)?;
    password.zeroize();
    Ok(hashed)
}

pub fn mint_token() -> String {
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut bytes);
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

pub fn hash_token(secret: &str) -> String {
    hex::encode(Sha256::digest(secret.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_auth_weak_score_is_too_guessable() {
        assert_eq!(score("password", "Ada", "ada@x.test"), Strength::TooGuessable);
        assert_eq!(score("", "Ada", "ada@x.test"), Strength::TooGuessable);
        let long = "a".repeat(129);
        assert_eq!(score(&long, "Ada", "ada@x.test"), Strength::TooGuessable);
    }

    #[test]
    fn us_auth_hash_round_trips() {
        let hash = hash_password("Thursday dinners at six oclock").unwrap();
        assert!(verify_password("Thursday dinners at six oclock", &hash));
        assert!(!verify_password("other", &hash));
    }
}
