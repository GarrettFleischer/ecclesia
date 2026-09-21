//! Join requests, invites, and planting a household.

use super::flags::Posture;
use super::model::{
    Church, DomainError, Effect, Membership, MembershipStatus, User, Viewer, Write,
};
use super::notice::{notice, notice_each_governor};
use super::rules::{
    can_decide_membership, invite_code_for, membership_after_approval, membership_after_decline,
};
use super::validate::{church_fields, normalize_email};

/// US-MEM-01 — ask to join a household.
pub fn request_join(
    actor: &User,
    church: &Church,
    existing: Option<&Membership>,
    governor_ids: &[String],
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    refuse_if_already_related(existing)?;
    let membership = pending_request(actor, church, id, now);
    let href = format!("/churches/{}", church.id);
    let mut effect = Effect::write(Write::InsertMembership(membership));
    notice_each_governor(
        &mut effect,
        governor_ids,
        "join_request",
        format!("{} asked to join {}", actor.name, church.name),
        "Approve or decline from the church page.",
        &href,
    );
    Ok(effect)
}

fn refuse_if_already_related(existing: Option<&Membership>) -> Result<(), DomainError> {
    match existing {
        Some(_) => Err(DomainError::AlreadyMember),
        None => Ok(()),
    }
}

fn pending_request(actor: &User, church: &Church, id: String, now: String) -> Membership {
    Membership {
        id,
        church_id: church.id.clone(),
        user_id: actor.id.clone(),
        role: "member".into(),
        status: MembershipStatus::PendingRequest.as_str().into(),
        created_at: now,
    }
}

/// US-MEM-02 — a pastor or steward invites someone already in Ecclesia.
pub fn invite_member(
    actor: &Viewer,
    church: &Church,
    invitee: &User,
    existing: Option<&Membership>,
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    require_governor(actor, &church.id)?;
    refuse_if_already_related(existing)?;
    let created = pending_invite(invitee, church, id, now);
    Ok(
        Effect::write(Write::InsertMembership(created)).with_notice(notice(
            &invitee.id,
            "invite",
            format!("{} invited you to {}", actor.user.name, church.name),
            "Accept from your home page or the church page.",
            format!("/churches/{}", church.id),
        )),
    )
}

fn require_governor(actor: &Viewer, church_id: &str) -> Result<(), DomainError> {
    let membership = actor
        .membership_in(church_id)
        .ok_or(DomainError::NotGovernor)?;
    can_decide_membership(membership)
}

fn pending_invite(invitee: &User, church: &Church, id: String, now: String) -> Membership {
    Membership {
        id,
        church_id: church.id.clone(),
        user_id: invitee.id.clone(),
        role: "member".into(),
        status: MembershipStatus::PendingInvite.as_str().into(),
        created_at: now,
    }
}

/// US-MEM-03 — redeem an invite code. The pastor already chose; the person still confirms.
pub fn redeem_invite(
    actor: &User,
    church: &Church,
    existing: Option<&Membership>,
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    if let Some(existing) = existing {
        return redeem_against_existing(existing);
    }
    Ok(Effect::write(Write::InsertMembership(pending_invite(
        actor, church, id, now,
    ))))
}

fn redeem_against_existing(existing: &Membership) -> Result<Effect, DomainError> {
    if existing.status() == Some(MembershipStatus::PendingInvite) {
        return Ok(Effect::default());
    }
    Err(DomainError::AlreadyMember)
}

/// US-MEM-04 — pastor or steward opens the door on a request.
pub fn approve_membership(
    actor: &Viewer,
    target: &Membership,
    church: &Church,
) -> Result<Effect, DomainError> {
    require_governor(actor, &target.church_id)?;
    let next = membership_after_approval(pending_status(target)?)?;
    Ok(set_membership(target, next).with_notice(approved_membership_notice(target, church)))
}

/// US-MEM-04 — pastor or steward closes the door on a request.
pub fn decline_membership(
    actor: &Viewer,
    target: &Membership,
    church: &Church,
) -> Result<Effect, DomainError> {
    require_governor(actor, &target.church_id)?;
    let next = membership_after_decline(pending_status(target)?)?;
    Ok(set_membership(target, next).with_notice(declined_membership_notice(target, church)))
}

fn pending_status(target: &Membership) -> Result<MembershipStatus, DomainError> {
    target.status().ok_or(DomainError::NothingPending)
}

fn set_membership(target: &Membership, next: MembershipStatus) -> Effect {
    Effect::write(Write::SetMembershipStatus {
        id: target.id.clone(),
        status: next.as_str(),
    })
}

fn approved_membership_notice(target: &Membership, church: &Church) -> super::model::NoticeDraft {
    notice(
        &target.user_id,
        "membership",
        format!("{} approved your request", church.name),
        "You can see and post needs there now.",
        format!("/churches/{}", church.id),
    )
}

fn declined_membership_notice(target: &Membership, church: &Church) -> super::model::NoticeDraft {
    notice(
        &target.user_id,
        "membership",
        format!("{} declined your request", church.name),
        "You can ask again later.",
        format!("/churches/{}", church.id),
    )
}

/// US-MEM-05 — the invited person accepts.
pub fn accept_invite(actor: &User, target: &Membership) -> Result<Effect, DomainError> {
    require_invitee(actor, target)?;
    require_pending_invite(target)?;
    Ok(Effect::write(Write::SetMembershipStatus {
        id: target.id.clone(),
        status: MembershipStatus::Active.as_str(),
    }))
}

fn require_invitee(actor: &User, target: &Membership) -> Result<(), DomainError> {
    if target.user_id == actor.id {
        Ok(())
    } else {
        Err(DomainError::NotInvitee)
    }
}

fn require_pending_invite(target: &Membership) -> Result<(), DomainError> {
    match target.status() {
        Some(MembershipStatus::PendingInvite) => Ok(()),
        _ => Err(DomainError::NothingPending),
    }
}

/// US-CH-01 — plant a household. The planter becomes owner.
pub fn plant_church(
    owner: &User,
    name: &str,
    city: &str,
    region: &str,
    description: &str,
    gathering: &str,
    posture: Posture,
    church_id: String,
    membership_id: String,
    nonce: &str,
    now: String,
) -> Result<Effect, DomainError> {
    super::flags::require_uplifting(posture)?;
    let (name, city, region, description, gathering) =
        church_fields(name, city, region, description, gathering)?;
    let church = planted_church(
        owner,
        name,
        city,
        region,
        description,
        gathering,
        church_id,
        nonce,
        now.clone(),
    );
    let membership = owner_membership(owner, &church.id, membership_id, now);
    let mut effect = Effect::write(Write::InsertChurch(church));
    effect.push(Write::InsertMembership(membership));
    Ok(effect)
}

fn planted_church(
    owner: &User,
    name: String,
    city: String,
    region: String,
    description: String,
    gathering: String,
    church_id: String,
    nonce: &str,
    now: String,
) -> Church {
    Church {
        id: church_id,
        invite_code: invite_code_for(&name, nonce),
        name,
        city,
        region,
        country: "US".into(),
        description,
        gathering,
        owner_id: owner.id.clone(),
        created_at: now,
    }
}

fn owner_membership(
    owner: &User,
    church_id: &str,
    membership_id: String,
    now: String,
) -> Membership {
    Membership {
        id: membership_id,
        church_id: church_id.into(),
        user_id: owner.id.clone(),
        role: "owner".into(),
        status: MembershipStatus::Active.as_str().into(),
        created_at: now,
    }
}

pub fn parse_invite_email(email: &str) -> Result<String, DomainError> {
    normalize_email(email)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leaf::sample::{church, membership, user, viewer_of};

    #[test]
    fn us_mem_01_request_notifies_governors() {
        let peter = user("peter");
        let grace = church("grace");
        let effect = request_join(
            &peter,
            &grace,
            None,
            &["miriam".into()],
            "m1".into(),
            "t1".into(),
        )
        .unwrap();
        assert!(matches!(effect.writes[0], Write::InsertMembership(_)));
        assert_eq!(effect.notices[0].user_id, "miriam");
        assert!(effect.notices[0].title.contains("peter"));
    }

    #[test]
    fn us_mem_01_cannot_request_twice() {
        let peter = user("peter");
        let grace = church("grace");
        let existing = membership("m0", "grace", "peter", "member", "pending_request");
        assert_eq!(
            request_join(
                &peter,
                &grace,
                Some(&existing),
                &[],
                "m1".into(),
                "t".into()
            ),
            Err(DomainError::AlreadyMember)
        );
    }

    #[test]
    fn us_mem_02_only_governors_invite() {
        let member = viewer_of(
            user("ruth"),
            vec![membership("m1", "grace", "ruth", "member", "active")],
            vec![church("grace")],
        );
        assert_eq!(
            invite_member(
                &member,
                &church("grace"),
                &user("james"),
                None,
                "m2".into(),
                "t".into()
            ),
            Err(DomainError::NotGovernor)
        );
    }

    #[test]
    fn us_mem_04_approve_writes_active_and_notifies() {
        let miriam = viewer_of(
            user("miriam"),
            vec![membership("m0", "grace", "miriam", "owner", "active")],
            vec![church("grace")],
        );
        let peter = membership("m1", "grace", "peter", "member", "pending_request");
        let effect = approve_membership(&miriam, &peter, &church("grace")).unwrap();
        match &effect.writes[0] {
            Write::SetMembershipStatus { status, .. } => assert_eq!(*status, "active"),
            other => panic!("{other:?}"),
        }
        assert_eq!(effect.notices[0].user_id, "peter");
    }

    #[test]
    fn us_mem_05_only_the_invitee_can_accept() {
        let target = membership("m1", "grace", "peter", "member", "pending_invite");
        assert_eq!(
            accept_invite(&user("miriam"), &target),
            Err(DomainError::NotInvitee)
        );
        let effect = accept_invite(&user("peter"), &target).unwrap();
        match effect.writes[0] {
            Write::SetMembershipStatus { status, .. } => assert_eq!(status, "active"),
            _ => panic!(),
        }
    }

    #[test]
    fn us_ch_01_planter_becomes_owner() {
        let effect = plant_church(
            &user("keisha"),
            "House of Bread",
            "Cedar Falls",
            "Iowa",
            "A table in the north end.",
            "Sundays",
            Posture::Lifts,
            "c1".into(),
            "m1".into(),
            "ab12",
            "t1".into(),
        )
        .unwrap();
        assert_eq!(effect.writes.len(), 2);
        match &effect.writes[1] {
            Write::InsertMembership(membership) => {
                assert_eq!(membership.role, "owner");
                assert_eq!(membership.status, "active");
            }
            other => panic!("{other:?}"),
        }
    }
}
