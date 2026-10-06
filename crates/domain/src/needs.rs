//! Posting needs and receiving offers.

use super::flags::{CatalogPresence, Posture, PriorOffer};
use super::model::{
    Application, ApplicationCard, ApplicationStatus, Church, DomainError, Effect, Need, NeedReply,
    NeedShelf, NeedSight, NeedStatus, User, Viewer, Write,
};
use super::notice::notice;
use super::person::display_name;
use super::rules::{NeedApproach, can_apply, can_reply, is_need_steward};
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
        closed_at: None,
        praise: None,
        shelf: NeedShelf::Listed,
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
        format!(
            "{} offered to help with {}",
            display_name(&viewer.user.first_name, &viewer.user.last_name),
            need.title
        ),
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

/// US-NEED-03 — the author marks a need met and writes how it was met.
pub fn close_need(
    viewer: &Viewer,
    need: &Need,
    praise: &str,
    posture: Posture,
    now: String,
) -> Result<Effect, DomainError> {
    require_need_author(viewer, need)?;
    if need.shelf == NeedShelf::Archived {
        return Err(DomainError::NeedArchived);
    }
    if !need.is_open() {
        return Err(DomainError::NeedClosed);
    }
    super::flags::require_uplifting(posture)?;
    let praise = note_field(praise)?;
    Ok(Effect::write(Write::SetNeedStatus {
        id: need.id.clone(),
        status: NeedStatus::Closed.as_str(),
        closed_at: Some(now),
        praise: Some(praise),
    }))
}

/// The author opens a met need again, until it is archived.
pub fn reopen_need(viewer: &Viewer, need: &Need) -> Result<Effect, DomainError> {
    require_need_author(viewer, need)?;
    if need.shelf == NeedShelf::Archived {
        return Err(DomainError::NeedArchived);
    }
    if need.status() != Some(NeedStatus::Closed) {
        return Err(open_need_status(need));
    }
    Ok(Effect::write(Write::SetNeedStatus {
        id: need.id.clone(),
        status: NeedStatus::Open.as_str(),
        closed_at: None,
        praise: None,
    }))
}

fn open_need_status(need: &Need) -> DomainError {
    if need.is_open() {
        DomainError::NeedOpen
    } else {
        DomainError::NeedClosed
    }
}

/// A public reply on a need the person can see.
pub fn reply_to_need(
    viewer: &Viewer,
    need: &Need,
    church: &Church,
    approach: NeedApproach,
    body: &str,
    posture: Posture,
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    super::flags::require_uplifting(posture)?;
    can_reply(viewer, need.sight(), church, approach)?;
    let body = note_field(body)?;
    let mut effect = Effect::write(Write::InsertNeedReply(NeedReply {
        id,
        need_id: need.id.clone(),
        author_id: viewer.user.id.clone(),
        body,
        created_at: now,
    }));
    if viewer.user.id != need.author_id {
        effect = effect.with_notice(notice(
            &need.author_id,
            "reply",
            format!(
                "{} replied on {}",
                display_name(&viewer.user.first_name, &viewer.user.last_name),
                need.title
            ),
            "Open the need.",
            format!("/needs/{}", need.id),
        ));
    }
    Ok(effect)
}

/// Move the author's open needs from a closed church onto a church they belong to.
pub fn import_open_needs(
    actor: &User,
    needs: &[Need],
    closed_church_ids: &[String],
    destination: &Church,
) -> Result<Effect, DomainError> {
    if !actor.is_active_in(&destination.id) {
        return Err(DomainError::NotInTheBody);
    }
    if needs.is_empty() {
        return Err(DomainError::NothingPending);
    }
    Ok(Effect {
        writes: moved_needs(actor, needs, closed_church_ids, destination)?,
        notices: Vec::new(),
    })
}

fn moved_needs(
    actor: &User,
    needs: &[Need],
    closed_church_ids: &[String],
    destination: &Church,
) -> Result<Vec<Write>, DomainError> {
    let mut writes = Vec::with_capacity(needs.len());
    for need in needs {
        writes.push(move_open_need(
            actor,
            need,
            closed_church_ids,
            destination,
        )?);
    }
    Ok(writes)
}

fn move_open_need(
    actor: &User,
    need: &Need,
    closed_church_ids: &[String],
    destination: &Church,
) -> Result<Write, DomainError> {
    if need.author_id != actor.id {
        return Err(DomainError::NotAuthor);
    }
    if need.shelf == NeedShelf::Archived {
        return Err(DomainError::NeedArchived);
    }
    if need.status() != Some(NeedStatus::Open) {
        return Err(DomainError::NeedClosed);
    }
    if !closed_church_ids.iter().any(|id| id == &need.church_id) {
        return Err(DomainError::ChurchStillOpen);
    }
    Ok(Write::MoveNeed {
        id: need.id.clone(),
        church_id: destination.id.clone(),
    })
}

fn require_need_steward(viewer: &Viewer, need: &Need) -> Result<(), DomainError> {
    if is_need_steward(viewer, need.sight()) {
        Ok(())
    } else {
        Err(DomainError::NotSteward)
    }
}

fn require_need_author(viewer: &Viewer, need: &Need) -> Result<(), DomainError> {
    if viewer.user.id == need.author_id {
        Ok(())
    } else {
        Err(DomainError::NotAuthor)
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
    use crate::sample::{church, church_at, user_in_church, viewer_of};

    #[test]
    fn us_need_01_requires_active_membership() {
        let peter = viewer_of(
            user_in_church("peter", "grace", "member", "pending"),
            Some(church("grace")),
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
        let mercy = church_at("mercy", 42.4928, -92.3426);
        let elena = viewer_of(
            user_in_church("elena", "mercy", "member", "active"),
            Some(mercy),
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
            closed_at: None,
            praise: None,
            shelf: NeedShelf::Listed,
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
    fn us_need_02_reply_notifies_the_author() {
        let mercy = church_at("mercy", 42.4928, -92.3426);
        let elena = viewer_of(
            user_in_church("elena", "mercy", "member", "active"),
            Some(mercy),
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
            closed_at: None,
            praise: None,
            shelf: NeedShelf::Listed,
        };
        let effect = reply_to_need(
            &elena,
            &need,
            &church("grace"),
            NeedApproach::Membership,
            "I can hold Thursday.",
            Posture::Lifts,
            "r1".into(),
            "t1".into(),
        )
        .unwrap();
        assert!(matches!(effect.writes[0], Write::InsertNeedReply(_)));
        assert_eq!(effect.notices[0].user_id, "miriam");
        assert_eq!(effect.notices[0].kind, "reply");
    }

    #[test]
    fn us_need_02_the_author_can_reply() {
        let grace = church("grace");
        let miriam = viewer_of(
            user_in_church("miriam", "grace", "owner", "active"),
            Some(grace.clone()),
        );
        let need = Need {
            id: "need_dinners".into(),
            church_id: "grace".into(),
            author_id: "miriam".into(),
            title: "Dinners".into(),
            body: "This week.".into(),
            gift_id: None,
            scope: "church".into(),
            status: "open".into(),
            created_at: "t0".into(),
            closed_at: None,
            praise: None,
            shelf: NeedShelf::Listed,
        };
        let effect = reply_to_need(
            &miriam,
            &need,
            &grace,
            NeedApproach::Membership,
            "I can bring Thursday.",
            Posture::Lifts,
            "r2".into(),
            "t1".into(),
        )
        .unwrap();
        assert!(effect.notices.is_empty());
    }

    #[test]
    fn us_need_06_offers_stay_with_steward_or_applicant() {
        let author = viewer_of(
            user_in_church("miriam", "grace", "owner", "active"),
            Some(church("grace")),
        );
        let neighbor = viewer_of(
            user_in_church("james", "luke", "member", "active"),
            Some(church("luke")),
        );
        let applicant = viewer_of(
            user_in_church("elena", "mercy", "member", "active"),
            Some(church_at("mercy", 42.4928, -92.3426)),
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
            closed_at: None,
            praise: None,
            shelf: NeedShelf::Listed,
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

    fn dinner(author: &str, status: &str, shelf: NeedShelf) -> Need {
        Need {
            id: "need_dinners".into(),
            church_id: "grace".into(),
            author_id: author.into(),
            title: "Dinners".into(),
            body: "This week.".into(),
            gift_id: None,
            scope: "church".into(),
            status: status.into(),
            created_at: "t0".into(),
            closed_at: None,
            praise: None,
            shelf,
        }
    }

    #[test]
    fn us_need_03_the_author_writes_how_it_was_met() {
        let grace = church("grace");
        let miriam = viewer_of(
            user_in_church("miriam", "grace", "owner", "active"),
            Some(grace),
        );
        let effect = close_need(
            &miriam,
            &dinner("miriam", "open", NeedShelf::Listed),
            "Thursday's meals are covered.",
            Posture::Lifts,
            "t2".into(),
        )
        .unwrap();
        let Write::SetNeedStatus {
            status,
            praise,
            closed_at,
            ..
        } = &effect.writes[0]
        else {
            panic!("expected status");
        };
        assert_eq!(*status, "closed");
        assert_eq!(praise.as_deref(), Some("Thursday's meals are covered."));
        assert_eq!(closed_at.as_deref(), Some("t2"));
    }

    #[test]
    fn us_need_03_only_the_author_can_mark_it_met() {
        let grace = church("grace");
        let pastor = viewer_of(
            user_in_church("peter", "grace", "owner", "active"),
            Some(grace),
        );
        let need = dinner("miriam", "open", NeedShelf::Listed);
        assert_eq!(
            close_need(&pastor, &need, "Done.", Posture::Lifts, "t2".into()),
            Err(DomainError::NotAuthor)
        );
        assert_eq!(
            close_need(
                &viewer_of(
                    user_in_church("miriam", "grace", "owner", "active"),
                    Some(church("grace")),
                ),
                &need,
                "   ",
                Posture::Lifts,
                "t2".into(),
            ),
            Err(DomainError::InvalidInput)
        );
    }

    #[test]
    fn us_need_03_the_author_can_reopen_until_it_is_archived() {
        let grace = church("grace");
        let miriam = viewer_of(
            user_in_church("miriam", "grace", "owner", "active"),
            Some(grace),
        );
        let effect = reopen_need(&miriam, &dinner("miriam", "closed", NeedShelf::Listed)).unwrap();
        let Write::SetNeedStatus {
            status,
            praise,
            closed_at,
            ..
        } = &effect.writes[0]
        else {
            panic!("expected status");
        };
        assert_eq!(*status, "open");
        assert!(praise.is_none());
        assert!(closed_at.is_none());
        assert_eq!(
            reopen_need(&miriam, &dinner("miriam", "open", NeedShelf::Listed)),
            Err(DomainError::NeedOpen)
        );
        assert_eq!(
            reopen_need(&miriam, &dinner("miriam", "closed", NeedShelf::Archived)),
            Err(DomainError::NeedArchived)
        );
    }

    #[test]
    fn author_moves_open_needs_from_a_closed_church() {
        let hope = church("hope");
        let ada = user_in_church("ada", "hope", "member", "active");
        let closed = vec!["grace".into()];
        let effect = import_open_needs(
            &ada,
            &[dinner("ada", "open", NeedShelf::Listed)],
            &closed,
            &hope,
        )
        .unwrap();
        match &effect.writes[0] {
            Write::MoveNeed { id, church_id } => {
                assert_eq!(id, "need_dinners");
                assert_eq!(church_id, "hope");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            import_open_needs(&ada, &[], &closed, &hope),
            Err(DomainError::NothingPending)
        );
        assert_eq!(
            import_open_needs(
                &ada,
                &[dinner("ada", "closed", NeedShelf::Listed)],
                &closed,
                &hope
            ),
            Err(DomainError::NeedClosed)
        );
        assert_eq!(
            import_open_needs(
                &ada,
                &[dinner("ada", "open", NeedShelf::Archived)],
                &closed,
                &hope
            ),
            Err(DomainError::NeedArchived)
        );
        assert_eq!(
            import_open_needs(
                &ada,
                &[dinner("ada", "open", NeedShelf::Listed)],
                &["hope".into()],
                &hope
            ),
            Err(DomainError::ChurchStillOpen)
        );
        let peter = user_in_church("peter", "hope", "member", "active");
        assert_eq!(
            import_open_needs(
                &peter,
                &[dinner("ada", "open", NeedShelf::Listed)],
                &closed,
                &hope
            ),
            Err(DomainError::NotAuthor)
        );
        let waiting = user_in_church("ada", "hope", "member", "pending");
        assert_eq!(
            import_open_needs(
                &waiting,
                &[dinner("ada", "open", NeedShelf::Listed)],
                &closed,
                &hope
            ),
            Err(DomainError::NotInTheBody)
        );
    }
}
