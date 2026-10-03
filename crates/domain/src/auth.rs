//! Registration rules.

use super::flags::{EmailAvailability, Posture, Strength};
use super::model::{DomainError, Effect, User, Write};
use super::validate::person_fields;

/// US-AUTH-01 — a new person registers with a password the SDK already scored.
pub fn register(
    first_name: &str,
    last_name: &str,
    email: &str,
    availability: EmailAvailability,
    posture: Posture,
    strength: Strength,
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    refuse_weak(strength)?;
    super::flags::require_uplifting(posture)?;
    refuse_taken_email(availability)?;
    let (first_name, last_name, email) = person_fields(first_name, last_name, email)?;
    Ok(Effect::write(Write::InsertUser(User {
        id,
        first_name,
        last_name,
        email,
        bio: String::new(),
        created_at: now,
        church_id: None,
        church_status: None,
        church_role: None,
    })))
}

fn refuse_weak(strength: Strength) -> Result<(), DomainError> {
    match strength {
        Strength::Acceptable => Ok(()),
        Strength::TooGuessable => Err(DomainError::WeakPassword),
    }
}

fn refuse_taken_email(availability: EmailAvailability) -> Result<(), DomainError> {
    match availability {
        EmailAvailability::Free => Ok(()),
        EmailAvailability::Taken => Err(DomainError::EmailTaken),
    }
}

/// US-AUTH-03 — a new password must pass the same strength the register story uses.
pub fn accept_password(strength: Strength) -> Result<(), DomainError> {
    refuse_weak(strength)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Write;

    #[test]
    fn us_auth_01_register_is_an_insert() {
        let effect = register(
            "Ada",
            "Lovelace",
            "ada@newmercy.test",
            EmailAvailability::Free,
            Posture::Lifts,
            Strength::Acceptable,
            "u1".into(),
            "t1".into(),
        )
        .unwrap();
        match &effect.writes[0] {
            Write::InsertUser(user) => {
                assert_eq!(user.email, "ada@newmercy.test");
                assert_eq!(user.first_name, "Ada");
                assert_eq!(user.last_name, "Lovelace");
                assert!(user.church_id.is_none());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn us_auth_01_duplicate_email_is_rejected() {
        assert_eq!(
            register(
                "Ada",
                "Lovelace",
                "ada@x.test",
                EmailAvailability::Taken,
                Posture::Lifts,
                Strength::Acceptable,
                "u1".into(),
                "t".into()
            ),
            Err(DomainError::EmailTaken)
        );
    }

    #[test]
    fn us_auth_01_weak_password_is_rejected() {
        assert_eq!(
            register(
                "Ada",
                "Lovelace",
                "ada@x.test",
                EmailAvailability::Free,
                Posture::Lifts,
                Strength::TooGuessable,
                "u1".into(),
                "t".into()
            ),
            Err(DomainError::WeakPassword)
        );
    }

    #[test]
    fn us_auth_01_overlong_first_name_is_rejected() {
        let edge = "a".repeat(80);
        assert!(
            register(
                &edge,
                "Lovelace",
                "ada@x.test",
                EmailAvailability::Free,
                Posture::Lifts,
                Strength::Acceptable,
                "u1".into(),
                "t".into()
            )
            .is_ok()
        );
        let long = "a".repeat(81);
        assert_eq!(
            register(
                &long,
                "Lovelace",
                "ada@x.test",
                EmailAvailability::Free,
                Posture::Lifts,
                Strength::Acceptable,
                "u1".into(),
                "t".into()
            ),
            Err(DomainError::InvalidInput)
        );
    }
}
