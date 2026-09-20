use super::model::{
    Application, Church, DomainError, Effect, Endorsement, Membership, MembershipStatus, Need,
    NoticeDraft, User, Viewer, Write,
};
use super::rules::{
    can_apply, can_decide_membership, can_endorse, invite_code_for, next_membership_after_decision,
};
use super::validate::{
    church_fields, need_fields, normalize_email, note_field, optional_note, person_fields,
    profile_fields, require_text,
};

fn notice(user_id: &str, kind: &str, title: String, body: &str, href: String) -> NoticeDraft {
    NoticeDraft {
        user_id: user_id.into(),
        kind: kind.into(),
        title,
        body: body.into(),
        href,
    }
}

/// US-AUTH-01 — a new person takes a seat at the table.
pub fn register(
    name: &str,
    email: &str,
    city: &str,
    region: &str,
    bio: &str,
    email_taken: bool,
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    if email_taken {
        return Err(DomainError::EmailTaken);
    }
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

/// US-AUTH-02 — sitting in another person's seat is a demo skin, not a household rule.
pub fn may_impersonate(demo_enabled: bool) -> Result<(), DomainError> {
    if demo_enabled {
        Ok(())
    } else {
        Err(DomainError::DemoDisabled)
    }
}

/// US-MEM-01 — ask to join a household.
pub fn request_join(
    actor: &User,
    church: &Church,
    existing: Option<&Membership>,
    governor_ids: &[String],
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    if existing.is_some() {
        return Err(DomainError::AlreadyMember);
    }
    let membership = Membership {
        id,
        church_id: church.id.clone(),
        user_id: actor.id.clone(),
        role: "member".into(),
        status: MembershipStatus::PendingRequest.as_str().into(),
        created_at: now,
    };
    let href = format!("/churches/{}", church.id);
    let mut effect = Effect::write(Write::InsertMembership(membership));
    for governor in governor_ids {
        effect.notices.push(notice(
            governor,
            "join_request",
            format!("{} asked to join {}", actor.name, church.name),
            "Approve them from the church page if they belong in this household.",
            href.clone(),
        ));
    }
    Ok(effect)
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
    let membership = actor
        .membership_in(&church.id)
        .ok_or(DomainError::NotGovernor)?;
    can_decide_membership(membership)?;
    if existing.is_some() {
        return Err(DomainError::AlreadyMember);
    }
    let created = Membership {
        id,
        church_id: church.id.clone(),
        user_id: invitee.id.clone(),
        role: "member".into(),
        status: MembershipStatus::PendingInvite.as_str().into(),
        created_at: now,
    };
    Ok(
        Effect::write(Write::InsertMembership(created)).with_notice(notice(
            &invitee.id,
            "invite",
            format!("{} invited you to {}", actor.user.name, church.name),
            "Accept from home or the church page. They already want you in.",
            format!("/churches/{}", church.id),
        )),
    )
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
        if existing.status() == Some(MembershipStatus::PendingInvite) {
            return Ok(Effect::default());
        }
        return Err(DomainError::AlreadyMember);
    }
    Ok(Effect::write(Write::InsertMembership(Membership {
        id,
        church_id: church.id.clone(),
        user_id: actor.id.clone(),
        role: "member".into(),
        status: MembershipStatus::PendingInvite.as_str().into(),
        created_at: now,
    })))
}

/// US-MEM-04 — pastor or steward opens or closes the door on a request.
pub fn decide_membership(
    actor: &Viewer,
    target: &Membership,
    church: &Church,
    approve: bool,
) -> Result<Effect, DomainError> {
    let actor_membership = actor
        .membership_in(&target.church_id)
        .ok_or(DomainError::NotGovernor)?;
    can_decide_membership(actor_membership)?;
    let current = target.status().ok_or(DomainError::NothingPending)?;
    let next = next_membership_after_decision(current, approve)?;
    let href = format!("/churches/{}", church.id);
    let (title, body) = if approve {
        (
            format!("You are in at {}", church.name),
            "Your gifts can now meet the needs of this household.",
        )
    } else {
        (
            format!("{} could not receive you just now", church.name),
            "You can ask again later, or look for another household.",
        )
    };
    Ok(Effect::write(Write::SetMembershipStatus {
        id: target.id.clone(),
        status: next.as_str().into(),
    })
    .with_notice(notice(&target.user_id, "membership", title, body, href)))
}

/// US-MEM-05 — the invited person accepts.
pub fn accept_invite(actor: &User, target: &Membership) -> Result<Effect, DomainError> {
    if target.user_id != actor.id || target.status() != Some(MembershipStatus::PendingInvite) {
        return Err(DomainError::NotGovernor);
    }
    Ok(Effect::write(Write::SetMembershipStatus {
        id: target.id.clone(),
        status: MembershipStatus::Active.as_str().into(),
    }))
}

/// US-CH-01 — plant a household. The planter becomes owner.
pub fn plant_church(
    owner: &User,
    name: &str,
    city: &str,
    region: &str,
    description: &str,
    gathering: &str,
    church_id: String,
    membership_id: String,
    nonce4: &str,
    now: String,
) -> Result<Effect, DomainError> {
    let (name, city, region, description, gathering) =
        church_fields(name, city, region, description, gathering)?;
    let church = Church {
        id: church_id,
        invite_code: invite_code_for(&name, nonce4),
        name,
        city,
        region,
        country: "US".into(),
        description,
        gathering,
        owner_id: owner.id.clone(),
        created_at: now.clone(),
    };
    let membership = Membership {
        id: membership_id,
        church_id: church.id.clone(),
        user_id: owner.id.clone(),
        role: "owner".into(),
        status: MembershipStatus::Active.as_str().into(),
        created_at: now,
    };
    let mut effect = Effect::write(Write::InsertChurch(church));
    effect.push(Write::InsertMembership(membership));
    Ok(effect)
}

/// US-NEED-01 — post a need from a household you already belong to.
pub fn post_need(
    viewer: &Viewer,
    church_id: &str,
    title: &str,
    body: &str,
    gift_id: Option<&str>,
    gift_exists: bool,
    scope: &str,
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    if !viewer.is_active_in(church_id) {
        return Err(DomainError::NotInTheBody);
    }
    if gift_id.is_some() && !gift_exists {
        return Err(DomainError::UnknownGift);
    }
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

/// US-NEED-02 — offer to carry a need you are allowed to see.
pub fn apply_to_need(
    viewer: &Viewer,
    need: &Need,
    church: &Church,
    already: bool,
    message: &str,
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    can_apply(viewer, need, church)?;
    if already {
        return Err(DomainError::AlreadyApplied);
    }
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

/// US-NEED-03 — the author or a governor closes a need.
pub fn close_need(viewer: &Viewer, need: &Need) -> Result<Effect, DomainError> {
    if viewer.user.id != need.author_id && !viewer.can_govern(&need.church_id) {
        return Err(DomainError::NotGovernor);
    }
    if !need.is_open() {
        return Err(DomainError::NeedClosed);
    }
    Ok(Effect::write(Write::SetNeedStatus {
        id: need.id.clone(),
        status: "closed".into(),
    }))
}

/// US-NEED-04 — receive or decline an offer.
pub fn decide_application(
    viewer: &Viewer,
    need: &Need,
    application: &Application,
    accept: bool,
) -> Result<Effect, DomainError> {
    if viewer.user.id != need.author_id && !viewer.can_govern(&need.church_id) {
        return Err(DomainError::NotGovernor);
    }
    if application.status != "pending" {
        return Err(DomainError::NothingPending);
    }
    let status = if accept { "accepted" } else { "declined" };
    let (title, body) = if accept {
        (
            format!("{} received your offer", need.title),
            "Go be the hands.",
        )
    } else {
        (
            format!("{} could not receive this offer", need.title),
            "Thank you for offering. Another need will come.",
        )
    };
    Ok(Effect::write(Write::SetApplicationStatus {
        id: application.id.clone(),
        status: status.into(),
    })
    .with_notice(notice(
        &application.user_id,
        "application",
        title,
        body,
        format!("/needs/{}", need.id),
    )))
}

/// US-END-01 — name a gift on someone else. They still have to wear it.
pub fn endorse(
    from: &User,
    to: &User,
    gift_id: &str,
    gift_exists: bool,
    pending_already: bool,
    note: &str,
    gift_name: &str,
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    can_endorse(&from.id, &to.id)?;
    if !gift_exists {
        return Err(DomainError::UnknownGift);
    }
    if pending_already {
        return Err(DomainError::DuplicateEndorsement);
    }
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

/// US-END-02 — the named person accepts or declines.
pub fn decide_endorsement(
    actor: &User,
    endorsement: &Endorsement,
    accept: bool,
    gift_name: &str,
) -> Result<Effect, DomainError> {
    if endorsement.to_user_id != actor.id || endorsement.status != "pending" {
        return Err(DomainError::NotGovernor);
    }
    let status = if accept { "accepted" } else { "declined" };
    let mut effect = Effect::write(Write::SetEndorsementStatus {
        id: endorsement.id.clone(),
        status: status.into(),
    });
    if accept {
        effect.push(Write::UpsertMemberGift {
            user_id: actor.id.clone(),
            gift_id: endorsement.gift_id.clone(),
            note: String::new(),
        });
    }
    let title = format!(
        "{} {} your endorsement for {gift_name}",
        actor.name,
        if accept { "received" } else { "declined" }
    );
    effect.notices.push(notice(
        &endorsement.from_user_id,
        "endorsement",
        title,
        if accept {
            "It is on their profile now."
        } else {
            "They chose not to wear it. That is theirs to decide."
        },
        format!("/members/{}", actor.id),
    ));
    Ok(effect)
}

/// US-GIFT-01 — name a gift you practice.
pub fn add_gift(
    user_id: &str,
    gift_id: &str,
    gift_exists: bool,
    note: &str,
) -> Result<Effect, DomainError> {
    if !gift_exists {
        return Err(DomainError::UnknownGift);
    }
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

pub fn parse_invite_email(email: &str) -> Result<String, DomainError> {
    normalize_email(email)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leaf::model::{Church, Membership, User, Viewer};

    fn user(id: &str) -> User {
        User {
            id: id.into(),
            name: id.into(),
            email: format!("{id}@ecclesia.test"),
            city: "Cedar Falls".into(),
            region: "Iowa".into(),
            bio: String::new(),
            created_at: "t0".into(),
        }
    }

    fn church(id: &str) -> Church {
        Church {
            id: id.into(),
            name: "Grace".into(),
            city: "Cedar Falls".into(),
            region: "Iowa".into(),
            country: "US".into(),
            description: "A household".into(),
            gathering: String::new(),
            owner_id: "miriam".into(),
            invite_code: "grace-k2m9".into(),
            created_at: "t0".into(),
        }
    }

    fn membership(
        id: &str,
        church_id: &str,
        user_id: &str,
        role: &str,
        status: &str,
    ) -> Membership {
        Membership {
            id: id.into(),
            church_id: church_id.into(),
            user_id: user_id.into(),
            role: role.into(),
            status: status.into(),
            created_at: "t0".into(),
        }
    }

    fn viewer_of(user: User, memberships: Vec<Membership>, churches: Vec<Church>) -> Viewer {
        Viewer {
            user,
            memberships,
            churches,
            gift_ids: vec![],
        }
    }

    #[test]
    fn us_auth_01_register_is_an_insert() {
        let effect = register(
            "Ada",
            "ada@newmercy.test",
            "Waterloo",
            "Iowa",
            "I cook",
            false,
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
                true,
                "u1".into(),
                "t".into()
            ),
            Err(DomainError::EmailTaken)
        );
    }

    #[test]
    fn us_auth_02_impersonation_is_a_skin_flag() {
        assert_eq!(may_impersonate(true), Ok(()));
        assert_eq!(may_impersonate(false), Err(DomainError::DemoDisabled));
    }

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
        let effect = decide_membership(&miriam, &peter, &church("grace"), true).unwrap();
        match &effect.writes[0] {
            Write::SetMembershipStatus { status, .. } => assert_eq!(status, "active"),
            other => panic!("{other:?}"),
        }
        assert_eq!(effect.notices[0].user_id, "peter");
    }

    #[test]
    fn us_mem_05_only_the_invitee_can_accept() {
        let target = membership("m1", "grace", "peter", "member", "pending_invite");
        assert_eq!(
            accept_invite(&user("miriam"), &target),
            Err(DomainError::NotGovernor)
        );
        let effect = accept_invite(&user("peter"), &target).unwrap();
        match effect.writes[0] {
            Write::SetMembershipStatus { ref status, .. } => assert_eq!(status, "active"),
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
                true,
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
            false,
            "I can hold Thursday.",
            "a1".into(),
            "t1".into(),
        )
        .unwrap();
        assert_eq!(effect.notices[0].user_id, "miriam");
    }

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
        let effect = decide_endorsement(&user("ruth"), &endorsement, true, "Hospitality").unwrap();
        assert!(effect
            .writes
            .iter()
            .any(|write| matches!(write, Write::UpsertMemberGift { gift_id, .. } if gift_id == "gift_hospitality")));
    }
}
