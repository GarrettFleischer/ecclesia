//! Join requests, invites, and planting a church.

use super::flags::Posture;
use super::model::{
    Church, ChurchLinkStatus, DomainError, Effect, MembershipRole, NoticeDraft, User, Viewer, Write,
};
use super::notice::{notice, notice_each_governor};
use super::person::display_name;
use super::rules::{can_decide_membership, invite_code_for};
use super::validate::{church_fields, registration_fields};

/// Ask to join. Other churches stay. This church, if already related, is refused.
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
    if actor.membership_in(church_id).is_some() {
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
    Write::UpsertMembership {
        user_id: user_id.into(),
        church_id: church_id.into(),
        status: status.as_str().into(),
        role: role.as_str().into(),
    }
}

fn clear_link(user_id: &str, church_id: &str) -> Write {
    Write::DeleteMembership {
        user_id: user_id.into(),
        church_id: church_id.into(),
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
    Ok(Effect::write(set_link(
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
    )))
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
    Ok(
        Effect::write(clear_link(&target.id, &church.id)).with_notice(notice(
            &target.id,
            "membership",
            format!("{} declined your request", church.name),
            "You can ask again later.",
            format!("/churches/{}", church.id),
        )),
    )
}

fn waiting_role(target: &User, church_id: &str) -> Result<MembershipRole, DomainError> {
    let Some(link) = target.membership_in(church_id) else {
        return Err(DomainError::NothingPending);
    };
    match link.status() {
        Some(ChurchLinkStatus::Pending | ChurchLinkStatus::Invited) => {
            Ok(link.role().unwrap_or(MembershipRole::Member))
        }
        _ => Err(DomainError::NothingPending),
    }
}

/// Drop one church. A pastor closes that church or names the next pastor.
pub fn leave_church(actor: &User, church_id: &str) -> Result<Effect, DomainError> {
    let Some(link) = actor.membership_in(church_id) else {
        return Err(DomainError::NoChurch);
    };
    if link.role() == Some(MembershipRole::Owner) {
        return Err(DomainError::PastorHoldsChurch);
    }
    Ok(Effect::write(clear_link(&actor.id, church_id)))
}

/// A member when their church closes. Sole members pick another church.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberRelease {
    OnlyHere(String),
    AlsoElsewhere(String),
}

impl MemberRelease {
    pub fn user_id(&self) -> &str {
        match self {
            Self::OnlyHere(user_id) | Self::AlsoElsewhere(user_id) => user_id,
        }
    }
}

/// The pastor closes the church. Every member is released. The row stays, marked closed.
pub fn close_church(
    actor: &User,
    church: &Church,
    members: &[MemberRelease],
    deleted_at: String,
) -> Result<Effect, DomainError> {
    require_pastor(actor, church)?;
    let mut effect = Effect::write(Write::CloseChurch {
        id: church.id.clone(),
        deleted_at,
    });
    release_members(&mut effect, &church.id, members, &actor.id);
    notify_closed(&mut effect, church, members, &actor.id);
    Ok(effect)
}

/// The pastor names the next pastor and stays on as a member.
pub fn transfer_church(actor: &User, church: &Church, next: &User) -> Result<Effect, DomainError> {
    require_pastor(actor, church)?;
    if actor.id == next.id {
        return Err(DomainError::SelfAction);
    }
    if next
        .membership_in(&church.id)
        .is_some_and(|link| link.role() == Some(MembershipRole::Owner))
    {
        return Err(DomainError::AlreadyPastor);
    }
    let mut effect = Effect::write(Write::SetChurchOwner {
        church_id: church.id.clone(),
        owner_id: next.id.clone(),
    });
    effect.push(set_link(
        &next.id,
        &church.id,
        ChurchLinkStatus::Active,
        MembershipRole::Owner,
    ));
    effect.push(set_link(
        &actor.id,
        &church.id,
        ChurchLinkStatus::Active,
        MembershipRole::Member,
    ));
    effect.notices.push(notice(
        &next.id,
        "church",
        format!(
            "{} named you pastor of {}",
            display_name(&actor.first_name, &actor.last_name),
            church.name
        ),
        "Open the church page.",
        format!("/churches/{}", church.id),
    ));
    Ok(effect)
}

fn require_pastor(actor: &User, church: &Church) -> Result<(), DomainError> {
    if is_pastor_of(actor, church) {
        Ok(())
    } else {
        Err(DomainError::NotGovernor)
    }
}

fn is_pastor_of(actor: &User, church: &Church) -> bool {
    actor.id == church.owner_id
        && actor
            .membership_in(&church.id)
            .is_some_and(|link| link.is_active() && link.role() == Some(MembershipRole::Owner))
}

fn release_members(
    effect: &mut Effect,
    church_id: &str,
    members: &[MemberRelease],
    actor_id: &str,
) {
    let mut released_actor = false;
    for member in members {
        if member.user_id() == actor_id {
            released_actor = true;
        }
        effect.push(clear_link(member.user_id(), church_id));
    }
    if !released_actor {
        effect.push(clear_link(actor_id, church_id));
    }
}

fn notify_closed(effect: &mut Effect, church: &Church, members: &[MemberRelease], actor_id: &str) {
    let title: std::sync::Arc<str> = std::sync::Arc::from(format!("{} is closed", church.name));
    for member in members {
        if member.user_id() == actor_id {
            continue;
        }
        let (body, href) = match member {
            MemberRelease::OnlyHere(_) => ("Find a church.", "/churches/join"),
            MemberRelease::AlsoElsewhere(_) => ("See your churches.", "/home"),
        };
        effect.notices.push(NoticeDraft {
            user_id: member.user_id().to_string(),
            kind: "church",
            title: title.clone(),
            body,
            href: href.into(),
        });
    }
}

/// The invited person accepts that church. Other churches stay.
pub fn accept_invite(actor: &User, church_id: &str) -> Result<Effect, DomainError> {
    let Some(link) = actor.membership_in(church_id) else {
        return Err(DomainError::NothingPending);
    };
    if link.status() != Some(ChurchLinkStatus::Invited) {
        return Err(DomainError::NothingPending);
    }
    let role = link.role().unwrap_or(MembershipRole::Member);
    Ok(Effect::write(set_link(
        &actor.id,
        church_id,
        ChurchLinkStatus::Active,
        role,
    )))
}

/// Plant a church. The planter becomes pastor there. Other churches stay.
pub fn plant_church(
    owner: &User,
    name: &str,
    address: &str,
    latitude: f64,
    longitude: f64,
    description: &str,
    gathering: &str,
    ein: &str,
    registry_state: &str,
    registry_number: &str,
    posture: Posture,
    church_id: String,
    nonce: &str,
    now: String,
) -> Result<Effect, DomainError> {
    super::flags::require_uplifting(posture)?;
    let (name, address, latitude, longitude, description, gathering) =
        church_fields(name, address, latitude, longitude, description, gathering)?;
    let (ein, registry_state, registry_number) =
        registration_fields(ein, registry_state, registry_number)?;
    let church = planted_church(
        owner,
        name,
        address,
        latitude,
        longitude,
        description,
        gathering,
        ein,
        registry_state,
        registry_number,
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
    ein: String,
    registry_state: String,
    registry_number: String,
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
        ein,
        registry_state,
        registry_number,
        owner_id: owner.id.clone(),
        created_at: now,
    }
}

pub fn parse_invite_email(email: &str) -> Result<String, DomainError> {
    super::validate::normalize_email(email)
}

/// Blank rows are skipped. A repeated address is kept once. An empty list is refused.
pub fn invite_addresses(raw: &[String]) -> Result<Vec<String>, DomainError> {
    let mut addresses = Vec::new();
    collect_invite_addresses(&mut addresses, raw)?;
    if addresses.is_empty() {
        Err(DomainError::InvalidEmail)
    } else {
        Ok(addresses)
    }
}

fn collect_invite_addresses(
    addresses: &mut Vec<String>,
    raw: &[String],
) -> Result<(), DomainError> {
    for email in raw {
        push_invite_address(addresses, email)?;
    }
    Ok(())
}

fn push_invite_address(addresses: &mut Vec<String>, email: &str) -> Result<(), DomainError> {
    if email.trim().is_empty() {
        return Ok(());
    }
    let parsed = parse_invite_email(email)?;
    if addresses.iter().any(|have| have == &parsed) {
        return Ok(());
    }
    addresses.push(parsed);
    Ok(())
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
            Write::UpsertMembership {
                church_id,
                status,
                role,
                ..
            } => {
                assert_eq!(church_id, "grace");
                assert_eq!(status, "pending");
                assert_eq!(role, "member");
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
            Write::UpsertMembership { status, role, .. } => {
                assert_eq!(status, "active");
                assert_eq!(role, "member");
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
            Write::UpsertMembership { church_id, .. } => {
                assert_eq!(church_id, "grace");
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
            Write::UpsertMembership { status, .. } => {
                assert_eq!(status, "active");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(effect.notices[0].user_id, "peter");
    }

    #[test]
    fn leave_drops_that_church_only() {
        let peter = user_in_church("peter", "grace", "member", "pending");
        let effect = leave_church(&peter, "grace").unwrap();
        match &effect.writes[0] {
            Write::DeleteMembership { user_id, church_id } => {
                assert_eq!(user_id, "peter");
                assert_eq!(church_id, "grace");
            }
            other => panic!("{other:?}"),
        }
        assert!(effect.notices.is_empty());
        let owner = user_in_church("keisha", "grace", "owner", "active");
        assert_eq!(
            leave_church(&owner, "grace"),
            Err(DomainError::PastorHoldsChurch)
        );
        let steward = user_in_church("ruth", "grace", "steward", "active");
        assert_eq!(leave_church(&steward, "grace").unwrap().writes.len(), 1);
        assert_eq!(
            leave_church(&user("peter"), "grace"),
            Err(DomainError::NoChurch)
        );
    }

    #[test]
    fn pastor_closes_the_church_and_releases_members() {
        let mut grace = church("grace");
        grace.owner_id = "keisha".into();
        grace.name = "Grace Covenant".into();
        let keisha = user_in_church("keisha", "grace", "owner", "active");
        let members = vec![
            MemberRelease::OnlyHere("keisha".into()),
            MemberRelease::OnlyHere("peter".into()),
        ];
        let effect = close_church(&keisha, &grace, &members, "t2".into()).unwrap();
        assert!(matches!(
            &effect.writes[0],
            Write::CloseChurch { id, deleted_at }
                if id.as_str() == "grace" && deleted_at.as_str() == "t2"
        ));
        assert_eq!(effect.writes.len(), 3);
        assert!(effect.writes[1..].iter().all(|write| matches!(
            write,
            Write::DeleteMembership { church_id, .. } if church_id == "grace"
        )));
        assert_eq!(effect.notices.len(), 1);
        assert_eq!(effect.notices[0].user_id, "peter");
        assert_eq!(effect.notices[0].title.as_ref(), "Grace Covenant is closed");
        assert_eq!(effect.notices[0].body, "Find a church.");
        assert_eq!(effect.notices[0].href, "/churches/join");
        let member = user_in_church("peter", "grace", "member", "active");
        assert_eq!(
            close_church(&member, &grace, &members, "t2".into()),
            Err(DomainError::NotGovernor)
        );
    }

    #[test]
    fn close_keeps_a_member_who_still_has_a_church() {
        let mut grace = church("grace");
        grace.owner_id = "keisha".into();
        grace.name = "Grace Covenant".into();
        let keisha = user_in_church("keisha", "grace", "owner", "active");
        let members = vec![
            MemberRelease::OnlyHere("keisha".into()),
            MemberRelease::AlsoElsewhere("ruth".into()),
        ];
        let effect = close_church(&keisha, &grace, &members, "t2".into()).unwrap();
        assert_eq!(effect.notices.len(), 1);
        assert_eq!(effect.notices[0].user_id, "ruth");
        assert_eq!(effect.notices[0].body, "See your churches.");
        assert_eq!(effect.notices[0].href, "/home");
    }

    #[test]
    fn pastor_names_the_next_pastor_and_stays() {
        let mut grace = church("grace");
        grace.owner_id = "keisha".into();
        grace.name = "Grace Covenant".into();
        let keisha = user_in_church("keisha", "grace", "owner", "active");
        let peter = user("peter");
        let effect = transfer_church(&keisha, &grace, &peter).unwrap();
        assert!(matches!(
            &effect.writes[0],
            Write::SetChurchOwner { owner_id, .. } if owner_id.as_str() == "peter"
        ));
        match &effect.writes[1] {
            Write::UpsertMembership {
                user_id,
                role,
                status,
                ..
            } => {
                assert_eq!(user_id, "peter");
                assert_eq!(role, "owner");
                assert_eq!(status, "active");
            }
            other => panic!("{other:?}"),
        }
        match &effect.writes[2] {
            Write::UpsertMembership { user_id, role, .. } => {
                assert_eq!(user_id, "keisha");
                assert_eq!(role, "member");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(effect.notices[0].user_id, "peter");
        assert!(effect.notices[0].title.contains("keisha"));
        assert_eq!(
            transfer_church(&keisha, &grace, &keisha),
            Err(DomainError::SelfAction)
        );
        let other_pastor = user_in_church("ada", "luke", "owner", "active");
        assert!(transfer_church(&keisha, &grace, &other_pastor).is_ok());
        let already = user_in_church("noah", "grace", "owner", "active");
        assert_eq!(
            transfer_church(&keisha, &grace, &already),
            Err(DomainError::AlreadyPastor)
        );
    }

    #[test]
    fn only_the_invitee_can_accept() {
        let miriam = user_in_church("miriam", "grace", "member", "active");
        assert_eq!(
            accept_invite(&miriam, "grace"),
            Err(DomainError::NothingPending)
        );
        let peter = user_in_church("peter", "grace", "member", "invited");
        let effect = accept_invite(&peter, "grace").unwrap();
        match &effect.writes[0] {
            Write::UpsertMembership { status, .. } => {
                assert_eq!(status, "active");
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
            "12-3456789",
            "IA",
            "123456",
            Posture::Lifts,
            "c1".into(),
            "ab12",
            "t1".into(),
        )
        .unwrap();
        assert_eq!(effect.writes.len(), 2);
        match &effect.writes[0] {
            Write::InsertChurch(church) => {
                assert_eq!(church.ein, "12-3456789");
                assert_eq!(church.registry_state, "IA");
                assert_eq!(church.registry_number, "123456");
            }
            other => panic!("{other:?}"),
        }
        match &effect.writes[1] {
            Write::UpsertMembership { role, status, .. } => {
                assert_eq!(role, "owner");
                assert_eq!(status, "active");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn invite_addresses_skip_blanks_and_repeats() {
        let raw = vec![
            " Ada@Church.org ".into(),
            String::new(),
            "ada@church.org".into(),
            "james@stlukes.test".into(),
        ];
        assert_eq!(
            invite_addresses(&raw).unwrap(),
            vec![
                "ada@church.org".to_string(),
                "james@stlukes.test".to_string()
            ]
        );
    }

    #[test]
    fn invite_addresses_refuse_a_bad_or_empty_list() {
        assert_eq!(
            invite_addresses(&["not-an-email".into()]).unwrap_err(),
            DomainError::InvalidEmail
        );
        assert_eq!(
            invite_addresses(&[String::new(), "  ".into()]).unwrap_err(),
            DomainError::InvalidEmail
        );
    }
}
