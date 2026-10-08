//! Posting needs and receiving offers.

use super::flags::{CatalogPresence, Posture, PriorOffer};
use super::media::{Attachment, AttachmentRef, accept_attachments};
use super::model::{
    Application, ApplicationCard, ApplicationStatus, Church, DomainError, Effect, Need, NeedReply,
    NeedShelf, NeedSight, NeedStatus, ReplyKind, User, Viewer, Write,
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
    post_need_with_attachments(
        viewer,
        church_id,
        title,
        body,
        gift_id,
        gift,
        scope,
        posture,
        &[],
        id,
        now,
    )
}

/// US-NEED-01 — post a need with up to five photos.
///
/// # Parameters
/// - `attachments`: staged photo ids and optional descriptions, in display order.
///
/// # Returns
/// An effect that inserts the need and then attaches every accepted photo, or an error.
///
/// # Notes
/// An empty slice posts the need alone. `post_need` does that.
/// A sixth photo, a blank id, a repeated id, or a long description refuses the whole post.
pub fn post_need_with_attachments(
    viewer: &Viewer,
    church_id: &str,
    title: &str,
    body: &str,
    gift_id: Option<&str>,
    gift: CatalogPresence,
    scope: &str,
    posture: Posture,
    attachments: &[AttachmentRef<'_>],
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    super::flags::require_uplifting(posture)?;
    if !viewer.is_active_in(church_id) {
        return Err(DomainError::NotInTheBody);
    }
    refuse_unknown_named_gift(gift_id, gift)?;
    let (title, body, parsed_scope) = need_fields(title, body, scope)?;
    let attachments = accept_attachments(attachments)?;
    let need_id = id.clone();
    let mut effect = Effect::write(Write::InsertNeed(Need {
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
    }));
    push_need_media(&mut effect, &need_id, attachments);
    Ok(effect)
}

fn push_need_media(effect: &mut Effect, need_id: &str, attachments: Vec<Attachment>) {
    if attachments.is_empty() {
        return;
    }
    effect.push(Write::AttachNeedMedia {
        need_id: need_id.to_string(),
        attachments,
    });
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
///
/// # Returns
/// An effect that marks the need open and clears `closing_reply_id`.
///
/// # Notes
/// Praise is cleared with the met status, as before. A completion reply stays
/// in the conversation. This write does not delete that reply.
pub fn reopen_need(viewer: &Viewer, need: &Need) -> Result<Effect, DomainError> {
    require_need_author(viewer, need)?;
    if need.shelf == NeedShelf::Archived {
        return Err(DomainError::NeedArchived);
    }
    if need.status() != Some(NeedStatus::Closed) {
        return Err(open_need_status(need));
    }
    let mut effect = Effect::write(Write::SetNeedStatus {
        id: need.id.clone(),
        status: NeedStatus::Open.as_str(),
        closed_at: None,
        praise: None,
    });
    effect.push(Write::SetClosingReply {
        need_id: need.id.clone(),
        reply_id: None,
    });
    Ok(effect)
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
    reply_to_need_with_attachments(viewer, need, church, approach, body, posture, &[], id, now)
}

/// A public reply with up to five photos.
///
/// # Parameters
/// - `attachments`: staged photo ids and optional descriptions, in display order.
///
/// # Returns
/// An effect that inserts a [`ReplyKind::Message`] and then attaches its photos.
///
/// # Notes
/// Membership, audience, and posture are the same rules as [`reply_to_need`].
/// An empty slice inserts the reply alone.
pub fn reply_to_need_with_attachments(
    viewer: &Viewer,
    need: &Need,
    church: &Church,
    approach: NeedApproach,
    body: &str,
    posture: Posture,
    attachments: &[AttachmentRef<'_>],
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    super::flags::require_uplifting(posture)?;
    can_reply(viewer, need.sight(), church, approach)?;
    let body = note_field(body)?;
    let attachments = accept_attachments(attachments)?;
    let reply_id = id.clone();
    let mut effect = Effect::write(Write::InsertNeedReply(NeedReply {
        id,
        need_id: need.id.clone(),
        author_id: viewer.user.id.clone(),
        kind: ReplyKind::Message,
        body,
        created_at: now,
    }));
    push_reply_media(&mut effect, &reply_id, attachments);
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

/// The author closes an open need with a completion reply.
///
/// # Parameters
/// - `viewer`: must be the need's author.
/// - `need`: the need being closed. It must be open and not archived.
/// - `body`: what happened. Required, with the same limits as a reply.
/// - `posture`: the word gate's judgment of that text.
/// - `attachments`: zero to five photos of the result.
/// - `id`: reply id minted by the SDK.
/// - `now`: timestamp minted by the SDK.
///
/// # Returns
/// One effect, or an error that carries no writes.
///
/// # Notes
/// On success the writes are ordered: insert the completion reply, attach its
/// photos (an empty list when there are none), mark the need met, then store
/// `closing_reply_id`. Met is [`NeedStatus::Closed`] (`"closed"`). The reply
/// body is not copied into `praise`. A legacy praise report still uses [`close_need`].
pub fn complete_need(
    viewer: &Viewer,
    need: &Need,
    body: &str,
    posture: Posture,
    attachments: &[AttachmentRef<'_>],
    id: String,
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
    let body = note_field(body)?;
    let attachments = accept_attachments(attachments)?;
    Ok(completion_effect(need, viewer, body, attachments, id, now))
}

fn completion_effect(
    need: &Need,
    viewer: &Viewer,
    body: String,
    attachments: Vec<Attachment>,
    id: String,
    now: String,
) -> Effect {
    let reply_id = id.clone();
    let mut effect = Effect::write(Write::InsertNeedReply(NeedReply {
        id,
        need_id: need.id.clone(),
        author_id: viewer.user.id.clone(),
        kind: ReplyKind::Completion,
        body,
        created_at: now.clone(),
    }));
    effect.push(Write::AttachReplyMedia {
        reply_id: reply_id.clone(),
        attachments,
    });
    effect.push(Write::SetNeedStatus {
        id: need.id.clone(),
        status: NeedStatus::Closed.as_str(),
        closed_at: Some(now),
        praise: None,
    });
    effect.push(Write::SetClosingReply {
        need_id: need.id.clone(),
        reply_id: Some(reply_id),
    });
    effect
}

fn push_reply_media(effect: &mut Effect, reply_id: &str, attachments: Vec<Attachment>) {
    if attachments.is_empty() {
        return;
    }
    effect.push(Write::AttachReplyMedia {
        reply_id: reply_id.to_string(),
        attachments,
    });
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
        writes.push(move_open_need(actor, need, closed_church_ids, destination)?);
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
            effect.writes[1],
            Write::SetClosingReply {
                need_id: "need_dinners".into(),
                reply_id: None,
            }
        );
        assert!(effect.writes.iter().all(|write| !matches!(
            write,
            Write::DetachReplyMedia { .. } | Write::InsertNeedReply(_)
        )));
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

    fn photo(media_id: &'static str, description: Option<&'static str>) -> AttachmentRef<'static> {
        AttachmentRef {
            media_id,
            description,
        }
    }

    fn counted_photos(count: usize) -> Vec<AttachmentRef<'static>> {
        let ids = ["m1", "m2", "m3", "m4", "m5", "m6"];
        ids[..count]
            .iter()
            .map(|media_id| photo(media_id, None))
            .collect()
    }

    fn author() -> Viewer {
        viewer_of(
            user_in_church("miriam", "grace", "owner", "active"),
            Some(church("grace")),
        )
    }

    #[test]
    fn a_need_accepts_zero_one_or_five_photos_and_refuses_six() {
        let miriam = author();
        let none = post_need_with_attachments(
            &miriam,
            "grace",
            "Dinners",
            "This week.",
            None,
            CatalogPresence::Listed,
            "church",
            Posture::Lifts,
            &[],
            "n0".into(),
            "t".into(),
        )
        .unwrap();
        assert!(matches!(none.writes[0], Write::InsertNeed(_)));
        assert_eq!(none.writes.len(), 1);

        let one = post_need_with_attachments(
            &miriam,
            "grace",
            "Dinners",
            "This week.",
            None,
            CatalogPresence::Listed,
            "church",
            Posture::Lifts,
            &[photo("m1", Some("  porch "))],
            "n1".into(),
            "t".into(),
        )
        .unwrap();
        let Write::AttachNeedMedia {
            need_id,
            attachments,
        } = &one.writes[1]
        else {
            panic!("expected photos");
        };
        assert_eq!(need_id, "n1");
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].position, 0);
        assert_eq!(attachments[0].description.as_deref(), Some("porch"));

        let five = post_need_with_attachments(
            &miriam,
            "grace",
            "Dinners",
            "This week.",
            None,
            CatalogPresence::Listed,
            "church",
            Posture::Lifts,
            &counted_photos(5),
            "n5".into(),
            "t".into(),
        )
        .unwrap();
        let Write::AttachNeedMedia { attachments, .. } = &five.writes[1] else {
            panic!("expected photos");
        };
        assert_eq!(attachments.len(), 5);

        assert_eq!(
            post_need_with_attachments(
                &miriam,
                "grace",
                "Dinners",
                "This week.",
                None,
                CatalogPresence::Listed,
                "church",
                Posture::Lifts,
                &counted_photos(6),
                "n6".into(),
                "t".into(),
            ),
            Err(DomainError::TooManyAttachments)
        );
    }

    #[test]
    fn a_reply_keeps_audience_rules_and_accepts_up_to_five_photos() {
        let grace = church("grace");
        let miriam = author();
        let need = dinner("miriam", "open", NeedShelf::Listed);
        let effect = reply_to_need_with_attachments(
            &miriam,
            &need,
            &grace,
            NeedApproach::Membership,
            "Thursday is covered.",
            Posture::Lifts,
            &counted_photos(5),
            "r5".into(),
            "t1".into(),
        )
        .unwrap();
        let Write::InsertNeedReply(reply) = &effect.writes[0] else {
            panic!("expected reply");
        };
        assert_eq!(reply.kind, ReplyKind::Message);
        assert_eq!(reply.kind.as_str(), "message");
        let Write::AttachReplyMedia { attachments, .. } = &effect.writes[1] else {
            panic!("expected photos");
        };
        assert_eq!(attachments.len(), 5);
        assert!(effect.notices.is_empty());

        assert_eq!(
            reply_to_need_with_attachments(
                &miriam,
                &need,
                &grace,
                NeedApproach::Membership,
                "Thursday is covered.",
                Posture::Lifts,
                &counted_photos(6),
                "r6".into(),
                "t1".into(),
            ),
            Err(DomainError::TooManyAttachments)
        );
        assert_eq!(
            reply_to_need_with_attachments(
                &miriam,
                &dinner("miriam", "closed", NeedShelf::Listed),
                &grace,
                NeedApproach::Membership,
                "Thursday is covered.",
                Posture::Lifts,
                &[photo("m1", None)],
                "r7".into(),
                "t1".into(),
            ),
            Err(DomainError::NeedClosed)
        );
    }

    #[test]
    fn completion_is_author_only_and_one_ordered_effect() {
        let grace = church("grace");
        let miriam = author();
        let pastor = viewer_of(
            user_in_church("peter", "grace", "owner", "active"),
            Some(grace),
        );
        let open = dinner("miriam", "open", NeedShelf::Listed);
        assert_eq!(
            complete_need(
                &pastor,
                &open,
                "Thursday's meals are covered.",
                Posture::Lifts,
                &[],
                "c0".into(),
                "t2".into(),
            ),
            Err(DomainError::NotAuthor)
        );
        assert_eq!(
            complete_need(
                &miriam,
                &dinner("miriam", "closed", NeedShelf::Listed),
                "Thursday's meals are covered.",
                Posture::Lifts,
                &[],
                "c1".into(),
                "t2".into(),
            ),
            Err(DomainError::NeedClosed)
        );
        assert_eq!(
            complete_need(
                &miriam,
                &dinner("miriam", "closed", NeedShelf::Archived),
                "Thursday's meals are covered.",
                Posture::Lifts,
                &[],
                "c2".into(),
                "t2".into(),
            ),
            Err(DomainError::NeedArchived)
        );

        let none = complete_need(
            &miriam,
            &open,
            "Thursday's meals are covered.",
            Posture::Lifts,
            &[],
            "c0".into(),
            "t2".into(),
        )
        .unwrap();
        assert_completion(&none, "c0", 0);
        assert!(none.notices.is_empty());

        let five = complete_need(
            &miriam,
            &open,
            "Thursday's meals are covered.",
            Posture::Lifts,
            &counted_photos(5),
            "c5".into(),
            "t2".into(),
        )
        .unwrap();
        assert_completion(&five, "c5", 5);

        assert_eq!(
            complete_need(
                &miriam,
                &open,
                "Thursday's meals are covered.",
                Posture::Lifts,
                &counted_photos(6),
                "c6".into(),
                "t2".into(),
            ),
            Err(DomainError::TooManyAttachments)
        );
        assert_eq!(
            complete_need(
                &miriam,
                &open,
                "   ",
                Posture::Lifts,
                &[photo("m1", None)],
                "c7".into(),
                "t2".into(),
            ),
            Err(DomainError::InvalidInput)
        );
        assert_eq!(
            complete_need(
                &miriam,
                &open,
                "Thursday's meals are covered.",
                Posture::TearsDown,
                &[],
                "c8".into(),
                "t2".into(),
            ),
            Err(DomainError::TearsDown)
        );
    }

    fn assert_completion(effect: &Effect, reply_id: &str, photos: usize) {
        assert_eq!(effect.writes.len(), 4);
        let Write::InsertNeedReply(reply) = &effect.writes[0] else {
            panic!("expected reply");
        };
        assert_eq!(reply.id, reply_id);
        assert_eq!(reply.kind, ReplyKind::Completion);
        assert_eq!(reply.kind.as_str(), "completion");
        assert_eq!(reply.body, "Thursday's meals are covered.");
        let Write::AttachReplyMedia {
            reply_id: attached_to,
            attachments,
        } = &effect.writes[1]
        else {
            panic!("expected photos");
        };
        assert_eq!(attached_to, reply_id);
        assert_eq!(attachments.len(), photos);
        let Write::SetNeedStatus {
            id,
            status,
            closed_at,
            praise,
        } = &effect.writes[2]
        else {
            panic!("expected status");
        };
        assert_eq!(id, "need_dinners");
        assert_eq!(*status, "closed");
        assert_eq!(closed_at.as_deref(), Some("t2"));
        assert!(praise.is_none());
        assert_eq!(
            effect.writes[3],
            Write::SetClosingReply {
                need_id: "need_dinners".into(),
                reply_id: Some(reply_id.into()),
            }
        );
    }

    #[test]
    fn legacy_praise_still_closes_without_a_reply() {
        let effect = close_need(
            &author(),
            &dinner("miriam", "open", NeedShelf::Listed),
            "Thursday's meals are covered.",
            Posture::Lifts,
            "t2".into(),
        )
        .unwrap();
        assert_eq!(effect.writes.len(), 1);
        let Write::SetNeedStatus { praise, status, .. } = &effect.writes[0] else {
            panic!("expected status");
        };
        assert_eq!(*status, "closed");
        assert_eq!(praise.as_deref(), Some("Thursday's meals are covered."));
    }
}
