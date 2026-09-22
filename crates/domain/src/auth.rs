//! Registration rules.

use super::flags::{EmailAvailability, Posture, Strength};
use super::model::{DomainError, Effect, User, Write};
use super::validate::person_fields;

/// US-AUTH-01 — a new person registers with a password the SDK already scored.
pub fn register(
    name: &str,
    email: &str,
    city: &str,
    region: &str,
    bio: &str,
    availability: EmailAvailability,
    posture: Posture,
    strength: Strength,
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    refuse_weak(strength)?;
    super::flags::require_uplifting(posture)?;
    refuse_taken_email(availability)?;
    let (name, email, city, region, bio) = person_fields(name, email, city, region, bio)?;
    Ok(Effect::write(Write::InsertUser(User {
        id,
        name,
        email,
        city,
        region,
        bio,
        created_at: now,
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
            "ada@newmercy.test",
            "Waterloo",
            "Iowa",
            "I cook",
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
                assert_eq!(user.name, "Ada");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn us_auth_01_duplicate_email_is_rejected() {
        assert_eq!(
            register(
                "Ada",
                "ada@x.test",
                "A",
                "B",
                "",
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
                "ada@x.test",
                "A",
                "B",
                "",
                EmailAvailability::Free,
                Posture::Lifts,
                Strength::TooGuessable,
                "u1".into(),
                "t".into()
            ),
            Err(DomainError::WeakPassword)
        );
    }
}
