//! Registration and demo-seat rules.

use super::flags::{DemoSeat, EmailAvailability, Posture};
use super::model::{DomainError, Effect, User, Write};
use super::validate::person_fields;

/// US-AUTH-01 — a new person takes a seat at the table.
pub fn register(
    name: &str,
    email: &str,
    city: &str,
    region: &str,
    bio: &str,
    availability: EmailAvailability,
    posture: Posture,
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
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

fn refuse_taken_email(availability: EmailAvailability) -> Result<(), DomainError> {
    match availability {
        EmailAvailability::Free => Ok(()),
        EmailAvailability::Taken => Err(DomainError::EmailTaken),
    }
}

/// US-AUTH-02 — sitting in another person's seat is a demo skin, not a household rule.
pub fn may_impersonate(seat: DemoSeat) -> Result<(), DomainError> {
    match seat {
        DemoSeat::Open => Ok(()),
        DemoSeat::Sealed => Err(DomainError::DemoDisabled),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leaf::model::Write;

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
                "u1".into(),
                "t".into()
            ),
            Err(DomainError::EmailTaken)
        );
    }

    #[test]
    fn us_auth_02_impersonation_is_a_skin_flag() {
        assert_eq!(may_impersonate(DemoSeat::Open), Ok(()));
        assert_eq!(
            may_impersonate(DemoSeat::Sealed),
            Err(DomainError::DemoDisabled)
        );
    }
}
