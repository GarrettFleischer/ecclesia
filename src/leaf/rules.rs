use super::model::{
    Church, DomainError, Membership, MembershipStatus, Need, NeedCard, NeedScope, Viewer,
};

pub fn churches_are_neighbors(a: &Church, b: &Church) -> bool {
    a.id != b.id
        && (a.city.eq_ignore_ascii_case(&b.city) || a.region.eq_ignore_ascii_case(&b.region))
}

pub fn can_view_need(viewer: &Viewer, need: &Need, church: &Church) -> bool {
    if viewer.user.id == need.author_id || viewer.can_govern(&need.church_id) {
        return true;
    }
    match need.scope() {
        Some(NeedScope::Body) => viewer.is_active_anywhere(),
        Some(NeedScope::Church) => viewer.is_active_in(&need.church_id),
        Some(NeedScope::Neighboring) => {
            viewer.is_active_in(&need.church_id)
                || viewer
                    .active_churches()
                    .any(|mine| churches_are_neighbors(mine, church))
        }
        None => false,
    }
}

pub fn can_apply(viewer: &Viewer, need: &Need, church: &Church) -> Result<(), DomainError> {
    if !need.is_open() {
        return Err(DomainError::NeedClosed);
    }
    if viewer.user.id == need.author_id {
        return Err(DomainError::OwnNeed);
    }
    if !can_view_need(viewer, need, church) {
        return match need.scope() {
            Some(NeedScope::Church) => Err(DomainError::OutsideChurch),
            Some(NeedScope::Neighboring) => Err(DomainError::OutsideNeighborhood),
            _ => Err(DomainError::NotInTheBody),
        };
    }
    if !viewer.is_active_anywhere() {
        return Err(DomainError::NotInTheBody);
    }
    Ok(())
}

pub fn can_endorse(from_id: &str, to_id: &str) -> Result<(), DomainError> {
    if from_id == to_id {
        Err(DomainError::SelfAction)
    } else {
        Ok(())
    }
}

pub fn can_decide_membership(actor: &Membership) -> Result<(), DomainError> {
    if actor.can_govern() {
        Ok(())
    } else {
        Err(DomainError::NotGovernor)
    }
}

pub fn membership_after_approval(
    current: MembershipStatus,
) -> Result<MembershipStatus, DomainError> {
    require_pending_membership(current)?;
    Ok(MembershipStatus::Active)
}

pub fn membership_after_decline(
    current: MembershipStatus,
) -> Result<MembershipStatus, DomainError> {
    require_pending_membership(current)?;
    Ok(MembershipStatus::Declined)
}

fn require_pending_membership(current: MembershipStatus) -> Result<(), DomainError> {
    match current {
        MembershipStatus::PendingRequest | MembershipStatus::PendingInvite => Ok(()),
        MembershipStatus::Active | MembershipStatus::Declined => Err(DomainError::NothingPending),
    }
}

pub fn visible_need_cards<'a>(
    viewer: &Viewer,
    cards: &'a [NeedCard],
    churches: &[Church],
) -> Vec<&'a NeedCard> {
    cards
        .iter()
        .filter(|card| card_is_visible(viewer, card, churches))
        .collect()
}

fn card_is_visible(viewer: &Viewer, card: &NeedCard, churches: &[Church]) -> bool {
    let Some(church) = church_for_card(card, churches) else {
        return false;
    };
    card.is_open() && can_view_need(viewer, &Need::from_card(card), church)
}

fn church_for_card<'a>(card: &NeedCard, churches: &'a [Church]) -> Option<&'a Church> {
    churches.iter().find(|church| church.id == card.church_id)
}

pub fn invite_code_for(name: &str, nonce4: &str) -> String {
    format!("{}-{}", slug_from(name, 8), nonce_from(nonce4, 4))
}

fn slug_from(name: &str, take: usize) -> String {
    name.chars()
        .flat_map(|c| c.to_lowercase())
        .filter(|c| c.is_ascii_alphanumeric())
        .take(take)
        .collect()
}

fn nonce_from(nonce4: &str, take: usize) -> String {
    nonce4
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(take)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leaf::model::{Membership, Need, User};

    fn user(id: &str, city: &str, region: &str) -> User {
        User {
            id: id.into(),
            name: id.into(),
            email: format!("{id}@ecclesia.test"),
            city: city.into(),
            region: region.into(),
            bio: String::new(),
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn church(id: &str, city: &str, region: &str) -> Church {
        Church {
            id: id.into(),
            name: id.into(),
            city: city.into(),
            region: region.into(),
            country: "US".into(),
            description: String::new(),
            gathering: String::new(),
            owner_id: "owner".into(),
            invite_code: "code".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn membership(church_id: &str, user_id: &str, status: &str) -> Membership {
        Membership {
            id: format!("{church_id}-{user_id}"),
            church_id: church_id.into(),
            user_id: user_id.into(),
            role: "member".into(),
            status: status.into(),
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn viewer(user: User, churches: Vec<Church>, memberships: Vec<Membership>) -> Viewer {
        Viewer {
            user,
            memberships,
            churches,
            gift_ids: vec![],
        }
    }

    fn need(scope: NeedScope, church_id: &str, author_id: &str) -> Need {
        Need {
            id: "need".into(),
            church_id: church_id.into(),
            author_id: author_id.into(),
            title: "Help".into(),
            body: "Please".into(),
            gift_id: None,
            scope: scope.as_str().into(),
            status: "open".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn us_body_01_neighbors_share_city_or_region() {
        let grace = church("grace", "Cedar Falls", "Iowa");
        let luke = church("luke", "Cedar Falls", "Iowa");
        let mercy = church("mercy", "Waterloo", "Iowa");
        let far = church("far", "Austin", "Texas");
        assert!(churches_are_neighbors(&grace, &luke));
        assert!(churches_are_neighbors(&grace, &mercy));
        assert!(!churches_are_neighbors(&grace, &far));
        assert!(!churches_are_neighbors(&grace, &grace));
    }

    #[test]
    fn us_need_05_church_scope_hides_need_from_neighbors() {
        let grace = church("grace", "Cedar Falls", "Iowa");
        let luke = church("luke", "Cedar Falls", "Iowa");
        let james = viewer(
            user("james", "Cedar Falls", "Iowa"),
            vec![luke],
            vec![membership("luke", "james", "active")],
        );
        let n = need(NeedScope::Church, "grace", "miriam");
        assert!(!can_view_need(&james, &n, &grace));
        assert_eq!(
            can_apply(&james, &n, &grace),
            Err(DomainError::OutsideChurch)
        );
    }

    #[test]
    fn us_need_05_neighboring_scope_opens_need_to_same_region() {
        let grace = church("grace", "Cedar Falls", "Iowa");
        let mercy = church("mercy", "Waterloo", "Iowa");
        let elena = viewer(
            user("elena", "Waterloo", "Iowa"),
            vec![mercy],
            vec![membership("mercy", "elena", "active")],
        );
        let n = need(NeedScope::Neighboring, "grace", "miriam");
        assert!(can_view_need(&elena, &n, &grace));
        assert_eq!(can_apply(&elena, &n, &grace), Ok(()));
    }

    #[test]
    fn us_need_05_pending_member_is_not_yet_in_the_body() {
        let grace = church("grace", "Cedar Falls", "Iowa");
        let peter = viewer(
            user("peter", "Cedar Falls", "Iowa"),
            vec![grace.clone()],
            vec![membership("grace", "peter", "pending_request")],
        );
        let n = need(NeedScope::Body, "grace", "miriam");
        assert!(!can_view_need(&peter, &n, &grace));
        assert_eq!(
            can_apply(&peter, &n, &grace),
            Err(DomainError::NotInTheBody)
        );
    }

    #[test]
    fn us_need_02_author_cannot_apply_to_own_need() {
        let grace = church("grace", "Cedar Falls", "Iowa");
        let miriam = viewer(
            user("miriam", "Cedar Falls", "Iowa"),
            vec![grace.clone()],
            vec![membership("grace", "miriam", "active")],
        );
        let n = need(NeedScope::Church, "grace", "miriam");
        assert_eq!(can_apply(&miriam, &n, &grace), Err(DomainError::OwnNeed));
    }

    #[test]
    fn us_end_01_endorsement_cannot_target_self() {
        assert_eq!(can_endorse("a", "a"), Err(DomainError::SelfAction));
        assert_eq!(can_endorse("a", "b"), Ok(()));
    }

    #[test]
    fn us_mem_04_only_pending_memberships_can_be_decided() {
        assert_eq!(
            membership_after_approval(MembershipStatus::PendingRequest),
            Ok(MembershipStatus::Active)
        );
        assert_eq!(
            membership_after_approval(MembershipStatus::Active),
            Err(DomainError::NothingPending)
        );
    }

    #[test]
    fn invite_code_is_pure_given_a_nonce() {
        assert_eq!(invite_code_for("Grace Covenant", "k2m9"), "gracecov-k2m9");
    }
}
