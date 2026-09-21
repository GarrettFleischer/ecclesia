//! Posting needs and receiving offers.

use super::flags::{CatalogPresence, OfferVerdict, PriorOffer};
use super::model::{Application, Church, DomainError, Effect, Need, Viewer, Write};
use super::notice::notice;
use super::rules::can_apply;
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
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
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
        status: "open".into(),
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
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    can_apply(viewer, need, church)?;
    refuse_duplicate_offer(prior)?;
    let message = note_field(message)?;
    Ok(Effect::write(Write::InsertApplication(Application {
        id,
        need_id: need.id.clone(),
        user_id: viewer.user.id.clone(),
        message,
        status: "pending".into(),
        created_at: now,
    }))
    .with_notice(notice(
        &need.author_id,
        "application",
        format!("{} offered to help: {}", viewer.user.name, need.title),
        "Receive them from the need if this is the right pair of hands.",
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
        status: "closed".into(),
    }))
}

fn require_need_steward(viewer: &Viewer, need: &Need) -> Result<(), DomainError> {
    if viewer.user.id == need.author_id || viewer.can_govern(&need.church_id) {
        Ok(())
    } else {
        Err(DomainError::NotGovernor)
    }
}

/// US-NEED-04 — receive an offer.
pub fn accept_application(
    viewer: &Viewer,
    need: &Need,
    application: &Application,
) -> Result<Effect, DomainError> {
    settle_application(viewer, need, application, OfferVerdict::Receive)
}

/// US-NEED-04 — decline an offer.
pub fn decline_application(
    viewer: &Viewer,
    need: &Need,
    application: &Application,
) -> Result<Effect, DomainError> {
    settle_application(viewer, need, application, OfferVerdict::Pass)
}

fn settle_application(
    viewer: &Viewer,
    need: &Need,
    application: &Application,
    verdict: OfferVerdict,
) -> Result<Effect, DomainError> {
    require_need_steward(viewer, need)?;
    if application.status != "pending" {
        return Err(DomainError::NothingPending);
    }
    Ok(Effect::write(Write::SetApplicationStatus {
        id: application.id.clone(),
        status: application_status(verdict).into(),
    })
    .with_notice(application_notice(need, application, verdict)))
}

fn application_status(verdict: OfferVerdict) -> &'static str {
    match verdict {
        OfferVerdict::Receive => "accepted",
        OfferVerdict::Pass => "declined",
    }
}

fn application_notice(
    need: &Need,
    application: &Application,
    verdict: OfferVerdict,
) -> super::model::NoticeDraft {
    match verdict {
        OfferVerdict::Receive => notice(
            &application.user_id,
            "application",
            format!("{} received your offer", need.title),
            "Go be the hands.",
            format!("/needs/{}", need.id),
        ),
        OfferVerdict::Pass => notice(
            &application.user_id,
            "application",
            format!("{} could not receive this offer", need.title),
            "Thank you for offering. Another need will come.",
            format!("/needs/{}", need.id),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leaf::sample::{church, membership, user, viewer_of};
    use crate::leaf::Church;

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
            "a1".into(),
            "t1".into(),
        )
        .unwrap();
        assert_eq!(effect.notices[0].user_id, "miriam");
    }
}
