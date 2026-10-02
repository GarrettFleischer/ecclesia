//! Join requests, invites, and planting a church.

use super::flags::Posture;
use super::model::{
    Church, ChurchLinkStatus, DomainError, Effect, MembershipRole, User, Viewer, Write,
};
use super::notice::{notice, notice_each_governor};
use super::person::display_name;
use super::rules::{can_decide_membership, invite_code_for};
use super::validate::church_fields;

/// Ask to join. Replaces any previous church. Same church is already related.
pub fn replace_with_pending(
    actor: &User,
    church: &Church,
    governor_ids: &[String],
) -> Result<Effect, DomainError> {
    refuse_same_church(actor, &church.id)?;
    let mut effect = Effect::write(set_link(
        &actor.id,
        &church.id,
        ChurchLinkStatus::Pending,
        MembershipRole::Member,
    ));
    let href = format!("/churches/{}", church.id);
    notice_each_governor(
        &mut effect,
        governor_ids,
        "join_request",
        format!(
            "{} asked to join {}",
            display_name(&actor.first_name, &actor.last_name),
            church.name
        ),
        "Approve or decline from the church page.",
        &href,
    );
    Ok(effect)
}

/// A church code makes the person an active member. No join request notice.
pub fn replace_with_code(actor: &User, church: &Church) -> Result<Effect, DomainError> {
    refuse_same_church(actor, &church.id)?;
    Ok(Effect::write(set_link(
        &actor.id,
        &church.id,
        ChurchLinkStatus::Active,
        MembershipRole::Member,
    )))
}

fn refuse_same_church(actor: &User, church_id: &str) -> Result<(), DomainError> {
    if actor.church_id.as_deref() == Some(church_id) && actor.link_status().is_some() {
        Err(DomainError::AlreadyMember)
    } else {
        Ok(())
    }
}

fn set_link(
    user_id: &str,
    church_id: &str,
    status: ChurchLinkStatus,
    role: MembershipRole,
) -> Write {
    Write::SetChurchLink {
        user_id: user_id.into(),
        church_id: Some(church_id.into()),
        church_status: Some(status.as_str().into()),
        church_role: Some(role.as_str().into()),
    }
}

fn clear_link(user_id: &str) -> Write {
    Write::SetChurchLink {
        user_id: user_id.into(),
        church_id: None,
        church_status: None,
        church_role: None,
    }
}

/// A pastor or steward invites someone already in Ecclesia.
pub fn invite_member(
    actor: &Viewer,
    church: &Church,
    invitee: &User,
) -> Result<Effect, DomainError> {
    can_decide_membership(&actor.user, &church.id)?;
    refuse_self_invite(&actor.user, invitee)?;
    refuse_same_church(invitee, &church.id)?;
    Ok(
        Effect::write(set_link(
            &invitee.id,
            &church.id,
            ChurchLinkStatus::Invited,
            MembershipRole::Member,
        ))
        .with_notice(notice(
            &invitee.id,
            "invite",
            format!(
                "{} invited you to {}",
                display_name(&actor.user.first_name, &actor.user.last_name),
                church.name
            ),
            "Accept from your home page or the church page.",
            format!("/churches/{}", church.id),
        )),
    )
}

fn refuse_self_invite(actor: &User, invitee: &User) -> Result<(), DomainError> {
    if actor.id == invitee.id {
        Err(DomainError::SelfAction)
    } else {
        Ok(())
    }
}

/// Pastor or steward opens the door on a request or an invite.
pub fn approve_membership(
    actor: &Viewer,
    target: &User,
    church: &Church,
) -> Result<Effect, DomainError> {
    can_decide_membership(&actor.user, &church.id)?;
    let role = waiting_role(target, &church.id)?;
    Ok(Effect::write(set_link(
        &target.id,
        &church.id,
        ChurchLinkStatus::Active,
        role,
    ))
    .with_notice(notice(
        &target.id,
        "membership",
        format!("{} approved your request", church.name),
        "You can see and post needs there now.",
        format!("/churches/{}", church.id),
    )))
}

/// Pastor or steward clears a waiting link.
pub fn decline_membership(
    actor: &Viewer,
    target: &User,
    church: &Church,
) -> Result<Effect, DomainError> {
    can_decide_membership(&actor.user, &church.id)?;
    waiting_role(target, &church.id)?;
    Ok(Effect::write(clear_link(&target.id)).with_notice(notice(
        &target.id,
        "membership",
        format!("{} declined your request", church.name),
        "You can ask again later.",
        format!("/churches/{}", church.id),
    )))
}

fn waiting_role(target: &User, church_id: &str) -> Result<MembershipRole, DomainError> {
    if target.church_id.as_deref() != Some(church_id) {
        return Err(DomainError::NothingPending);
    }
    match target.link_status() {
        Some(ChurchLinkStatus::Pending | ChurchLinkStatus::Invited) => {
            Ok(target.link_role().unwrap_or(MembershipRole::Member))
        }
        _ => Err(DomainError::NothingPending),
    }
}

/// The invited person accepts.
pub fn accept_invite(actor: &User) -> Result<Effect, DomainError> {
    let church_id = actor
        .church_id
        .clone()
        .ok_or(DomainError::NothingPending)?;
    if actor.link_status() != Some(ChurchLinkStatus::Invited) {
        return Err(DomainError::NothingPending);
    }
    let role = actor.link_role().unwrap_or(MembershipRole::Member);
    Ok(Effect::write(set_link(
        &actor.id,
        &church_id,
        ChurchLinkStatus::Active,
        role,
    )))
}

/// Plant a church. The planter becomes owner, replacing any previous church.
pub fn plant_church(
    owner: &User,
    name: &str,
    address: &str,
    latitude: f64,
    longitude: f64,
    description: &str,
    gathering: &str,
    posture: Posture,
    church_id: String,
    nonce: &str,
    now: String,
) -> Result<Effect, DomainError> {
    super::flags::require_uplifting(posture)?;
    let (name, address, latitude, longitude, description, gathering) =
        church_fields(name, address, latitude, longitude, description, gathering)?;
    let church = planted_church(
        owner,
        name,
        address,
        latitude,
        longitude,
        description,
        gathering,
        church_id,
        nonce,
        now,
    );
    let mut effect = Effect::write(Write::InsertChurch(church.clone()));
    effect.push(set_link(
        &owner.id,
        &church.id,
        ChurchLinkStatus::Active,
        MembershipRole::Owner,
    ));
    Ok(effect)
}

fn planted_church(
    owner: &User,
    name: String,
    address: String,
    latitude: f64,
    longitude: f64,
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
        address,
        latitude,
        longitude,
        country: "US".into(),
        description,
        gathering,
        owner_id: owner.id.clone(),
        created_at: now,
    }
}

pub fn parse_invite_email(email: &str) -> Result<String, DomainError> {
    super::validate::normalize_email(email)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::{church, user, user_in_church, viewer_of};

    #[test]
    fn replace_with_pending_emits_one_link_and_notifies() {
        let peter = user("peter");
        let grace = church("grace");
        let effect = replace_with_pending(&peter, &grace, &["miriam".into()]).unwrap();
        match &effect.writes[0] {
            Write::SetChurchLink {
                church_id,
                church_status,
                church_role,
                ..
            } => {
                assert_eq!(church_id.as_deref(), Some("grace"));
                assert_eq!(church_status.as_deref(), Some("pending"));
                assert_eq!(church_role.as_deref(), Some("member"));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(effect.writes.len(), 1);
        assert_eq!(effect.notices[0].user_id, "miriam");
        assert!(effect.notices[0].title.contains("peter"));
    }

    #[test]
    fn replace_with_code_emits_one_active_link() {
        let peter = user("peter");
        let grace = church("grace");
        let effect = replace_with_code(&peter, &grace).unwrap();
        match &effect.writes[0] {
            Write::SetChurchLink {
                church_status,
                church_role,
                ..
            } => {
                assert_eq!(church_status.as_deref(), Some("active"));
                assert_eq!(church_role.as_deref(), Some("member"));
            }
            other => panic!("{other:?}"),
        }
        assert!(effect.notices.is_empty());
    }

    #[test]
    fn second_join_still_emits_one_link() {
        let peter = user_in_church("peter", "luke", "member", "active");
        let effect = replace_with_pending(&peter, &church("grace"), &[]).unwrap();
        assert_eq!(effect.writes.len(), 1);
        match &effect.writes[0] {
            Write::SetChurchLink { church_id, .. } => {
                assert_eq!(church_id.as_deref(), Some("grace"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn same_church_is_already_related() {
        let peter = user_in_church("peter", "grace", "member", "pending");
        assert_eq!(
            replace_with_pending(&peter, &church("grace"), &[]),
            Err(DomainError::AlreadyMember)
        );
    }

    #[test]
    fn cannot_invite_yourself() {
        let miriam = viewer_of(
            user_in_church("miriam", "grace", "owner", "active"),
            Some(church("grace")),
        );
        assert_eq!(
            invite_member(&miriam, &church("grace"), &user("miriam")),
            Err(DomainError::SelfAction)
        );
    }

    #[test]
    fn only_governors_invite() {
        let member = viewer_of(
            user_in_church("ruth", "grace", "member", "active"),
            Some(church("grace")),
        );
        assert_eq!(
            invite_member(&member, &church("grace"), &user("james")),
            Err(DomainError::NotGovernor)
        );
    }

    #[test]
    fn approve_writes_active_and_notifies() {
        let miriam = viewer_of(
            user_in_church("miriam", "grace", "owner", "active"),
            Some(church("grace")),
        );
        let peter = user_in_church("peter", "grace", "member", "pending");
        let effect = approve_membership(&miriam, &peter, &church("grace")).unwrap();
        match &effect.writes[0] {
            Write::SetChurchLink { church_status, .. } => {
                assert_eq!(church_status.as_deref(), Some("active"));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(effect.notices[0].user_id, "peter");
    }

    #[test]
    fn only_the_invitee_can_accept() {
        let miriam = user_in_church("miriam", "grace", "member", "active");
        assert_eq!(accept_invite(&miriam), Err(DomainError::NothingPending));
        let peter = user_in_church("peter", "grace", "member", "invited");
        let effect = accept_invite(&peter).unwrap();
        match &effect.writes[0] {
            Write::SetChurchLink { church_status, .. } => {
                assert_eq!(church_status.as_deref(), Some("active"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn planter_becomes_owner() {
        let effect = plant_church(
            &user("keisha"),
            "House of Bread",
            "100 Main Street\nCedar Falls IA 50613",
            42.53,
            -92.45,
            "A church in the north end.",
            "Sundays",
            Posture::Lifts,
            "c1".into(),
            "ab12",
            "t1".into(),
        )
        .unwrap();
        assert_eq!(effect.writes.len(), 2);
        match &effect.writes[1] {
            Write::SetChurchLink {
                church_role,
                church_status,
                ..
            } => {
                assert_eq!(church_role.as_deref(), Some("owner"));
                assert_eq!(church_status.as_deref(), Some("active"));
            }
            other => panic!("{other:?}"),
        }
    }
}
