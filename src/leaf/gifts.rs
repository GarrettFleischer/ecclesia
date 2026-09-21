//! Gifts, endorsements, and how a person is known.

use super::flags::{CatalogPresence, EndorsementQueue, GiftOnProfile};
use super::model::{
    DomainError, Effect, Endorsement, EndorsementCard, EndorsementStatus, Gift, User, Write,
};
use super::notice::notice;
use super::rules::can_endorse;
use super::validate::{note_field, optional_note, profile_fields, require_text, skill_field};

/// A skill someone else names. Catalog gifts stay linkable to needs;
/// anything else is stored as the words they typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillSource<'a> {
    Catalog { id: &'a str, name: &'a str },
    Spoken(&'a str),
}

impl<'a> SkillSource<'a> {
    pub fn from_catalog(catalog: &'a [Gift], typed: &'a str) -> Result<Self, DomainError> {
        let typed = typed.trim();
        if typed.is_empty() {
            return Err(DomainError::InvalidInput);
        }
        if let Some(gift) = catalog.iter().find(|gift| skill_matches(gift, typed)) {
            return Ok(Self::Catalog {
                id: gift.id.as_str(),
                name: gift.name.as_str(),
            });
        }
        Ok(Self::Spoken(typed))
    }

    pub fn display(self) -> &'a str {
        match self {
            Self::Catalog { name, .. } => name,
            Self::Spoken(name) => name,
        }
    }

    pub fn gift_id(self) -> &'a str {
        match self {
            Self::Catalog { id, .. } => id,
            Self::Spoken(_) => "",
        }
    }
}

fn skill_matches(gift: &Gift, typed: &str) -> bool {
    gift.id == typed || same_skill(&gift.name, typed)
}

fn same_skill(left: &str, right: &str) -> bool {
    let mut left = left.split_whitespace().flat_map(str::chars);
    let mut right = right.split_whitespace().flat_map(str::chars);
    loop {
        match (left.next(), right.next()) {
            (None, None) => return true,
            (Some(a), Some(b)) if a.eq_ignore_ascii_case(&b) => {}
            _ => return false,
        }
    }
}

/// US-END-01 — name a skill on someone else. They still decide whether to accept it.
pub fn endorse(
    from: &User,
    to: &User,
    skill: SkillSource<'_>,
    queue: EndorsementQueue,
    note: &str,
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    can_endorse(&from.id, &to.id)?;
    refuse_waiting_endorsement(queue)?;
    let skill_name = skill_field(skill.display())?;
    let note = note_field(note)?;
    let title = format!("{} endorsed you for {skill_name}", from.name);
    Ok(Effect::write(Write::InsertEndorsement(Endorsement {
        id,
        from_user_id: from.id.clone(),
        to_user_id: to.id.clone(),
        gift_id: skill.gift_id().into(),
        skill: skill_name,
        note,
        status: "pending".into(),
        created_at: now,
    }))
    .with_notice(notice(
        &to.id,
        "endorsement",
        title,
        "Accept it from your inbox, or decline.",
        "/inbox".into(),
    )))
}

fn refuse_waiting_endorsement(queue: EndorsementQueue) -> Result<(), DomainError> {
    match queue {
        EndorsementQueue::Clear => Ok(()),
        EndorsementQueue::Waiting => Err(DomainError::DuplicateEndorsement),
    }
}

/// US-END-02 — the named person accepts the message onto their profile.
pub fn accept_endorsement(
    actor: &User,
    endorsement: &Endorsement,
    held: GiftOnProfile,
) -> Result<Effect, DomainError> {
    require_open_recipient(actor, endorsement)?;
    let mut effect = set_endorsement(endorsement, EndorsementStatus::Accepted.as_str());
    if let Some(write) = gift_to_add(endorsement, held) {
        effect.push(write);
    }
    effect
        .notices
        .push(accepted_endorsement_notice(actor, endorsement));
    Ok(effect)
}

/// US-END-02 — the named person keeps the message off the public profile.
/// They and the endorser can still see it, and they can accept it later.
pub fn decline_endorsement(actor: &User, endorsement: &Endorsement) -> Result<Effect, DomainError> {
    require_pending_recipient(actor, endorsement)?;
    Ok(
        set_endorsement(endorsement, EndorsementStatus::Declined.as_str())
            .with_notice(declined_endorsement_notice(actor, endorsement)),
    )
}

/// A declined endorsement stays between the pair until they accept it.
pub fn declined_visible_to<'a>(
    viewer_id: &'a str,
    cards: impl IntoIterator<Item = &'a EndorsementCard>,
) -> impl Iterator<Item = &'a EndorsementCard> {
    cards
        .into_iter()
        .filter(move |card| viewer_id == card.to_user_id || viewer_id == card.from_user_id)
}

fn gift_to_add(endorsement: &Endorsement, held: GiftOnProfile) -> Option<Write> {
    if endorsement.gift_id.is_empty() {
        return None;
    }
    match held {
        GiftOnProfile::Named => None,
        GiftOnProfile::Absent => Some(Write::UpsertMemberGift {
            user_id: endorsement.to_user_id.clone(),
            gift_id: endorsement.gift_id.clone(),
            note: String::new(),
        }),
    }
}

fn require_pending_recipient(actor: &User, endorsement: &Endorsement) -> Result<(), DomainError> {
    require_recipient(actor, endorsement)?;
    match endorsement.status() {
        Some(EndorsementStatus::Pending) => Ok(()),
        _ => Err(DomainError::NothingPending),
    }
}

fn require_open_recipient(actor: &User, endorsement: &Endorsement) -> Result<(), DomainError> {
    require_recipient(actor, endorsement)?;
    match endorsement.status() {
        Some(EndorsementStatus::Pending | EndorsementStatus::Declined) => Ok(()),
        _ => Err(DomainError::NothingPending),
    }
}

fn require_recipient(actor: &User, endorsement: &Endorsement) -> Result<(), DomainError> {
    if endorsement.to_user_id == actor.id {
        Ok(())
    } else {
        Err(DomainError::NotRecipient)
    }
}

fn set_endorsement(endorsement: &Endorsement, status: &'static str) -> Effect {
    Effect::write(Write::SetEndorsementStatus {
        id: endorsement.id.clone(),
        status,
    })
}

fn accepted_endorsement_notice(
    actor: &User,
    endorsement: &Endorsement,
) -> super::model::NoticeDraft {
    notice(
        &endorsement.from_user_id,
        "endorsement",
        format!(
            "{} accepted your endorsement for {}",
            actor.name, endorsement.skill
        ),
        "It's on their profile.",
        format!("/members/{}", actor.id),
    )
}

fn declined_endorsement_notice(
    actor: &User,
    endorsement: &Endorsement,
) -> super::model::NoticeDraft {
    notice(
        &endorsement.from_user_id,
        "endorsement",
        format!(
            "{} declined your endorsement for {}",
            actor.name, endorsement.skill
        ),
        "You can still see it on their profile.",
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

fn require_listed_gift(gift: CatalogPresence) -> Result<(), DomainError> {
    match gift {
        CatalogPresence::Listed => Ok(()),
        CatalogPresence::Unknown => Err(DomainError::UnknownGift),
    }
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

    fn hospitality() -> Gift {
        Gift {
            id: "gift_hospitality".into(),
            name: "Hospitality".into(),
            category: "spiritual".into(),
        }
    }

    fn pending(skill: &str, gift_id: &str) -> Endorsement {
        Endorsement {
            id: "e1".into(),
            from_user_id: "james".into(),
            to_user_id: "ruth".into(),
            gift_id: gift_id.into(),
            skill: skill.into(),
            note: "She stayed until the last parent came.".into(),
            status: "pending".into(),
            created_at: "t0".into(),
        }
    }

    #[test]
    fn us_end_01_catalog_match_is_case_and_space_insensitive() {
        let catalog = [hospitality()];
        let source = SkillSource::from_catalog(&catalog, "  hospitality ").unwrap();
        assert_eq!(
            source,
            SkillSource::Catalog {
                id: "gift_hospitality",
                name: "Hospitality"
            }
        );
    }

    #[test]
    fn us_end_01_spoken_skill_does_not_need_the_catalog() {
        let catalog = [hospitality()];
        let source = SkillSource::from_catalog(&catalog, "Sitting still with people").unwrap();
        assert_eq!(source, SkillSource::Spoken("Sitting still with people"));
        assert_eq!(source.gift_id(), "");
    }

    #[test]
    fn us_end_01_endorsement_is_not_limited_to_claimed_gifts() {
        let effect = endorse(
            &user("james"),
            &user("daniel"),
            SkillSource::Catalog {
                id: "gift_counseling",
                name: "Counseling",
            },
            EndorsementQueue::Clear,
            "He sat with my cousin and didn't try to fill the silence.",
            "e2".into(),
            "t1".into(),
        )
        .unwrap();
        let Write::InsertEndorsement(endorsement) = &effect.writes[0] else {
            panic!("expected insert");
        };
        assert_eq!(endorsement.to_user_id, "daniel");
        assert_eq!(endorsement.gift_id, "gift_counseling");
        assert_eq!(endorsement.skill, "Counseling");
        assert_eq!(effect.notices[0].user_id, "daniel");
        assert!(effect.notices[0].title.contains("Counseling"));
    }

    #[test]
    fn us_end_02_accept_adds_a_catalog_gift_they_had_not_claimed() {
        let endorsement = pending("Counseling", "gift_counseling");
        let effect =
            accept_endorsement(&user("ruth"), &endorsement, GiftOnProfile::Absent).unwrap();
        assert!(effect.writes.iter().any(
            |write| matches!(write, Write::UpsertMemberGift { gift_id, .. } if gift_id == "gift_counseling")
        ));
    }

    #[test]
    fn us_end_02_accept_does_not_overwrite_a_gift_they_already_named() {
        let endorsement = pending("Hospitality", "gift_hospitality");
        let effect = accept_endorsement(&user("ruth"), &endorsement, GiftOnProfile::Named).unwrap();
        assert!(!effect
            .writes
            .iter()
            .any(|write| matches!(write, Write::UpsertMemberGift { .. })));
    }

    #[test]
    fn us_end_02_accept_spoken_skill_adds_no_catalog_gift() {
        let endorsement = pending("Sitting still with people", "");
        let effect =
            accept_endorsement(&user("ruth"), &endorsement, GiftOnProfile::Absent).unwrap();
        assert!(!effect
            .writes
            .iter()
            .any(|write| matches!(write, Write::UpsertMemberGift { .. })));
        assert!(effect.notices[0].title.contains("Sitting still"));
        assert!(effect.notices[0].title.contains("accepted"));
    }

    #[test]
    fn us_end_02_declined_can_be_accepted_later() {
        let mut endorsement = pending("Hospitality", "gift_hospitality");
        endorsement.status = EndorsementStatus::Declined.as_str().into();
        let effect =
            accept_endorsement(&user("ruth"), &endorsement, GiftOnProfile::Absent).unwrap();
        assert!(effect.writes.iter().any(
            |write| matches!(write, Write::SetEndorsementStatus { status, .. } if *status == "accepted")
        ));
    }

    #[test]
    fn us_end_02_declined_stays_between_the_pair() {
        let card = EndorsementCard {
            id: "e1".into(),
            from_user_id: "james".into(),
            from_user_name: "James".into(),
            to_user_id: "ruth".into(),
            to_user_name: "Ruth".into(),
            gift_id: "gift_hospitality".into(),
            gift_name: "Hospitality".into(),
            note: "She stayed.".into(),
            status: "declined".into(),
            created_at: "t0".into(),
        };
        let cards = [card];
        assert_eq!(declined_visible_to("ruth", &cards).count(), 1);
        assert_eq!(declined_visible_to("james", &cards).count(), 1);
        assert_eq!(declined_visible_to("peter", &cards).count(), 0);
    }

    #[test]
    fn us_end_02_only_the_named_person_can_accept() {
        let endorsement = pending("Hospitality", "gift_hospitality");
        assert_eq!(
            accept_endorsement(&user("james"), &endorsement, GiftOnProfile::Absent),
            Err(DomainError::NotRecipient)
        );
    }
}
