//! Gifts, endorsements, and how a person is known.

use super::flags::{CatalogPresence, EndorsementQueue};
use super::model::{DomainError, Effect, Endorsement, User, Write};
use super::notice::notice;
use super::rules::can_endorse;
use super::validate::{note_field, optional_note, profile_fields, require_text};

/// US-END-01 — name a gift on someone else. They still have to wear it.
pub fn endorse(
    from: &User,
    to: &User,
    gift_id: &str,
    gift: CatalogPresence,
    queue: EndorsementQueue,
    note: &str,
    gift_name: &str,
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    can_endorse(&from.id, &to.id)?;
    require_listed_gift(gift)?;
    refuse_waiting_endorsement(queue)?;
    let note = note_field(note)?;
    Ok(Effect::write(Write::InsertEndorsement(Endorsement {
        id,
        from_user_id: from.id.clone(),
        to_user_id: to.id.clone(),
        gift_id: gift_id.into(),
        note,
        status: "pending".into(),
        created_at: now,
    }))
    .with_notice(notice(
        &to.id,
        "endorsement",
        format!("{} endorsed you for {gift_name}", from.name),
        "Accept it onto your profile from Inbox, or decline.",
        "/inbox".into(),
    )))
}

fn require_listed_gift(gift: CatalogPresence) -> Result<(), DomainError> {
    match gift {
        CatalogPresence::Listed => Ok(()),
        CatalogPresence::Unknown => Err(DomainError::UnknownGift),
    }
}

fn refuse_waiting_endorsement(queue: EndorsementQueue) -> Result<(), DomainError> {
    match queue {
        EndorsementQueue::Clear => Ok(()),
        EndorsementQueue::Waiting => Err(DomainError::DuplicateEndorsement),
    }
}

/// US-END-02 — the named person wears the word.
pub fn accept_endorsement(
    actor: &User,
    endorsement: &Endorsement,
    gift_name: &str,
) -> Result<Effect, DomainError> {
    require_pending_recipient(actor, endorsement)?;
    let mut effect = set_endorsement(endorsement, "accepted");
    effect.push(Write::UpsertMemberGift {
        user_id: actor.id.clone(),
        gift_id: endorsement.gift_id.clone(),
        note: String::new(),
    });
    effect
        .notices
        .push(worn_endorsement_notice(actor, endorsement, gift_name));
    Ok(effect)
}

/// US-END-02 — the named person lets the word go.
pub fn decline_endorsement(
    actor: &User,
    endorsement: &Endorsement,
    gift_name: &str,
) -> Result<Effect, DomainError> {
    require_pending_recipient(actor, endorsement)?;
    Ok(
        set_endorsement(endorsement, "declined").with_notice(declined_endorsement_notice(
            actor,
            endorsement,
            gift_name,
        )),
    )
}

fn require_pending_recipient(actor: &User, endorsement: &Endorsement) -> Result<(), DomainError> {
    if endorsement.to_user_id != actor.id || endorsement.status != "pending" {
        Err(DomainError::NotGovernor)
    } else {
        Ok(())
    }
}

fn set_endorsement(endorsement: &Endorsement, status: &'static str) -> Effect {
    Effect::write(Write::SetEndorsementStatus {
        id: endorsement.id.clone(),
        status,
    })
}

fn worn_endorsement_notice(
    actor: &User,
    endorsement: &Endorsement,
    gift_name: &str,
) -> super::model::NoticeDraft {
    notice(
        &endorsement.from_user_id,
        "endorsement",
        format!("{} received your endorsement for {gift_name}", actor.name),
        "It is on their profile now.",
        format!("/members/{}", actor.id),
    )
}

fn declined_endorsement_notice(
    actor: &User,
    endorsement: &Endorsement,
    gift_name: &str,
) -> super::model::NoticeDraft {
    notice(
        &endorsement.from_user_id,
        "endorsement",
        format!("{} declined your endorsement for {gift_name}", actor.name),
        "They chose not to wear it. That is theirs to decide.",
        format!("/members/{}", actor.id),
    )
}

/// US-GIFT-01 — name a gift you practice.
pub fn add_gift(
    user_id: &str,
    gift_id: &str,
    gift: CatalogPresence,
    note: &str,
) -> Result<Effect, DomainError> {
    require_listed_gift(gift)?;
    Ok(Effect::write(Write::UpsertMemberGift {
        user_id: user_id.into(),
        gift_id: gift_id.into(),
        note: optional_note(note)?,
    }))
}

pub fn remove_gift(user_id: &str, gift_id: &str) -> Result<Effect, DomainError> {
    require_text(gift_id, 80)?;
    Ok(Effect::write(Write::RemoveMemberGift {
        user_id: user_id.into(),
        gift_id: gift_id.into(),
    }))
}

/// US-PROF-01 — update how you are known.
pub fn update_profile(
    user_id: &str,
    name: &str,
    city: &str,
    region: &str,
    bio: &str,
) -> Result<Effect, DomainError> {
    let (name, city, region, bio) = profile_fields(name, city, region, bio)?;
    Ok(Effect::write(Write::UpdateUser {
        id: user_id.into(),
        name,
        city,
        region,
        bio,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leaf::sample::user;

    #[test]
    fn us_end_02_accept_adds_the_gift() {
        let endorsement = Endorsement {
            id: "e1".into(),
            from_user_id: "james".into(),
            to_user_id: "ruth".into(),
            gift_id: "gift_hospitality".into(),
            note: "She stayed.".into(),
            status: "pending".into(),
            created_at: "t0".into(),
        };
        let effect = accept_endorsement(&user("ruth"), &endorsement, "Hospitality").unwrap();
        assert!(effect.writes.iter().any(
            |write| matches!(write, Write::UpsertMemberGift { gift_id, .. } if gift_id == "gift_hospitality")
        ));
    }
}
