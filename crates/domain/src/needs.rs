//! Posting needs and receiving offers.

use super::flags::{CatalogPresence, Posture, PriorOffer};
use super::model::{
    Application, ApplicationCard, ApplicationStatus, Church, DomainError, Effect, Need, NeedSight,
    NeedStatus, Viewer, Write,
};
use super::notice::notice;
use super::rules::{can_apply, is_need_steward};
use super::validate::{need_fields, note_field};

/// US-NEED-01 — post a need from a household you already belong to.
pub fn post_need(
    viewer: &Viewer,
    church_id: &str,
    title: &str,
    body: &str,
    gift_id: Option<&str>,
    gift: CatalogPresence,
    scope: &str,
    posture: Posture,
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    super::flags::require_uplifting(posture)?;
    if !viewer.is_active_in(church_id) {
        return Err(DomainError::NotInTheBody);
    }
    refuse_unknown_named_gift(gift_id, gift)?;
    let (title, body, parsed_scope) = need_fields(title, body, scope)?;
    Ok(Effect::write(Write::InsertNeed(Need {
        id,
        church_id: church_id.into(),
        author_id: viewer.user.id.clone(),
        title,
        body,
        gift_id: gift_id.map(ToOwned::to_owned),
        scope: parsed_scope.as_str().into(),
        status: NeedStatus::Open.as_str().into(),
        created_at: now,
    })))
}

fn refuse_unknown_named_gift(
    gift_id: Option<&str>,
    gift: CatalogPresence,
) -> Result<(), DomainError> {
    match (gift_id, gift) {
        (Some(_), CatalogPresence::Unknown) => Err(DomainError::UnknownGift),
        _ => Ok(()),
    }
}

/// US-NEED-02 — offer to carry a need you are allowed to see.
pub fn apply_to_need(
    viewer: &Viewer,
    need: &Need,
    church: &Church,
    prior: PriorOffer,
    message: &str,
    posture: Posture,
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    super::flags::require_uplifting(posture)?;
    can_apply(viewer, need.sight(), church)?;
    refuse_duplicate_offer(prior)?;
    let message = note_field(message)?;
    Ok(Effect::write(Write::InsertApplication(Application {
        id,
        need_id: need.id.clone(),
        user_id: viewer.user.id.clone(),
        message,
        status: ApplicationStatus::Pending.as_str().into(),
        created_at: now,
    }))
    .with_notice(notice(
        &need.author_id,
        "application",
        format!("{} offered to help with {}", viewer.user.name, need.title),
        "Accept or decline on the need.",
        format!("/needs/{}", need.id),
    )))
}

fn refuse_duplicate_offer(prior: PriorOffer) -> Result<(), DomainError> {
    match prior {
        PriorOffer::Fresh => Ok(()),
        PriorOffer::AlreadyMade => Err(DomainError::AlreadyApplied),
    }
}

/// US-NEED-03 — the author or a governor closes a need.
pub fn close_need(viewer: &Viewer, need: &Need) -> Result<Effect, DomainError> {
    require_need_steward(viewer, need)?;
    if !need.is_open() {
        return Err(DomainError::NeedClosed);
    }
    Ok(Effect::write(Write::SetNeedStatus {
        id: need.id.clone(),
        status: NeedStatus::Closed.as_str(),
    }))
}

fn require_need_steward(viewer: &Viewer, need: &Need) -> Result<(), DomainError> {
    if is_need_steward(viewer, need.sight()) {
        Ok(())
    } else {
        Err(DomainError::NotSteward)
    }
}

/// Offers stay between the steward and the person who wrote them.
pub fn visible_offers<'a>(
    viewer: &'a Viewer,
    need: NeedSight<'_>,
    cards: &'a [ApplicationCard],
) -> impl Iterator<Item = &'a ApplicationCard> + 'a {
    let steward = is_need_steward(viewer, need);
    cards
        .iter()
        .filter(move |card| steward || card.user_id == viewer.user.id)
}

/// US-NEED-04 — receive an offer.
pub fn accept_application(
    viewer: &Viewer,
    need: &Need,
    application: &Application,
) -> Result<Effect, DomainError> {
    require_need_steward(viewer, need)?;
    require_pending_application(application)?;
    Ok(
        set_application(application, ApplicationStatus::Accepted.as_str())
            .with_notice(received_application_notice(need, application)),
    )
}

/// US-NEED-04 — decline an offer.
pub fn decline_application(
    viewer: &Viewer,
    need: &Need,
    application: &Application,
) -> Result<Effect, DomainError> {
    require_need_steward(viewer, need)?;
    require_pending_application(application)?;
    Ok(
        set_application(application, ApplicationStatus::Declined.as_str())
            .with_notice(passed_application_notice(need, application)),
    )
}

fn require_pending_application(application: &Application) -> Result<(), DomainError> {
    match application.status() {
        Some(ApplicationStatus::Pending) => Ok(()),
        _ => Err(DomainError::NothingPending),
    }
}

fn set_application(application: &Application, status: &'static str) -> Effect {
    Effect::write(Write::SetApplicationStatus {
        id: application.id.clone(),
        status,
    })
}

fn received_application_notice(
    need: &Need,
    application: &Application,
) -> super::model::NoticeDraft {
    notice(
        &application.user_id,
        "application",
        format!("Your offer on {} was accepted", need.title),
        "Reach out to them and set a time.",
        format!("/needs/{}", need.id),
    )
}

fn passed_application_notice(need: &Need, application: &Application) -> super::model::NoticeDraft {
    notice(
        &application.user_id,
        "application",
        format!("Your offer on {} was declined", need.title),
        "Thanks for offering.",
        format!("/needs/{}", need.id),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Church;
    use crate::sample::{church, membership, user, viewer_of};

    #[test]
    fn us_need_01_requires_active_membership() {
        let peter = viewer_of(
            user("peter"),
            vec![membership(
                "m1",
                "grace",
                "peter",
                "member",
                "pending_request",
            )],
            vec![church("grace")],
        );
        assert_eq!(
            post_need(
                &peter,
                "grace",
                "Help",
                "Please",
                None,
                CatalogPresence::Listed,
                "church",
                Posture::Lifts,
                "n1".into(),
                "t".into()
            ),
            Err(DomainError::NotInTheBody)
        );
    }

    #[test]
    fn us_need_02_apply_notifies_author() {
        let elena = viewer_of(
            user("elena"),
            vec![membership("m1", "mercy", "elena", "member", "active")],
            vec![Church {
                id: "mercy".into(),
                name: "Mercy".into(),
                city: "Waterloo".into(),
                region: "Iowa".into(),
                country: "US".into(),
                description: String::new(),
                gathering: String::new(),
                owner_id: "keisha".into(),
                invite_code: "x".into(),
                created_at: "t0".into(),
            }],
        );
        let need = Need {
            id: "need_spanish".into(),
            church_id: "grace".into(),
            author_id: "miriam".into(),
            title: "Spanish interpreter".into(),
            body: "Thursday".into(),
            gift_id: Some("gift_translation".into()),
            scope: "neighboring".into(),
            status: "open".into(),
            created_at: "t0".into(),
        };
        let effect = apply_to_need(
            &elena,
            &need,
            &church("grace"),
            PriorOffer::Fresh,
            "I can hold Thursday.",
            Posture::Lifts,
            "a1".into(),
            "t1".into(),
        )
        .unwrap();
        assert_eq!(effect.notices[0].user_id, "miriam");
    }

    #[test]
    fn us_need_06_offers_stay_with_steward_or_applicant() {
        let author = viewer_of(
            user("miriam"),
            vec![membership("m0", "grace", "miriam", "owner", "active")],
            vec![church("grace")],
        );
        let neighbor = viewer_of(
            user("james"),
            vec![membership("m1", "luke", "james", "member", "active")],
            vec![church("luke")],
        );
        let applicant = viewer_of(
            user("elena"),
            vec![membership("m2", "mercy", "elena", "member", "active")],
            vec![church("mercy")],
        );
        let need = Need {
            id: "need_spanish".into(),
            church_id: "grace".into(),
            author_id: "miriam".into(),
            title: "Spanish interpreter".into(),
            body: "Thursday".into(),
            gift_id: None,
            scope: "neighboring".into(),
            status: "open".into(),
            created_at: "t0".into(),
        };
        let elena = ApplicationCard {
            id: "a1".into(),
            need_id: need.id.clone(),
            user_id: "elena".into(),
            user_name: "Elena".into(),
            message: "I can hold Thursday.".into(),
            status: "pending".into(),
            created_at: "t1".into(),
        };
        let cards = [elena];
        assert_eq!(visible_offers(&author, need.sight(), &cards).count(), 1);
        assert_eq!(visible_offers(&applicant, need.sight(), &cards).count(), 1);
        assert_eq!(visible_offers(&neighbor, need.sight(), &cards).count(), 0);
    }
}
