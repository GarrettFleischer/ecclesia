use super::geo::{Place, distance_km, nearby_km, place_is_near};
use super::model::{
    Church, ChurchLinkStatus, DomainError, NeedCard, NeedScope, NeedSight, User, Viewer,
};

pub fn churches_are_neighbors(a: &Church, b: &Church) -> bool {
    a.id != b.id
        && distance_km(a.latitude, a.longitude, b.latitude, b.longitude) <= nearby_km()
}

pub fn is_need_steward(viewer: &Viewer, need: NeedSight<'_>) -> bool {
    viewer.user.id == need.author_id || viewer.can_govern(need.church_id)
}

pub fn can_view_need(viewer: &Viewer, need: NeedSight<'_>, church: &Church) -> bool {
    if is_need_steward(viewer, need) {
        return true;
    }
    match need.scope {
        Some(NeedScope::Body) => viewer.is_active_anywhere(),
        Some(NeedScope::Church) => viewer.is_active_in(need.church_id),
        Some(NeedScope::Neighboring) => {
            viewer.is_active_in(need.church_id)
                || viewer
                    .active_church()
                    .is_some_and(|mine| churches_are_neighbors(mine, church))
        }
        None => false,
    }
}

pub fn can_view_need_near(
    viewer: &Viewer,
    need: NeedSight<'_>,
    church: &Church,
    place: Place,
) -> bool {
    if !place_is_near(church.latitude, church.longitude, place) {
        return false;
    }
    can_view_need(viewer, need, church) || viewer.is_active_anywhere()
}

pub enum NeedApproach {
    Membership,
    Near(Place),
}

pub fn can_reply(
    viewer: &Viewer,
    need: NeedSight<'_>,
    church: &Church,
    approach: NeedApproach,
) -> Result<(), DomainError> {
    if !need.is_open() {
        return Err(DomainError::NeedClosed);
    }
    let allowed = match approach {
        NeedApproach::Membership => can_view_need(viewer, need, church),
        NeedApproach::Near(place) => can_view_need_near(viewer, need, church, place),
    };
    if !allowed {
        return Err(need_hidden(need));
    }
    if !viewer.is_active_anywhere() {
        return Err(DomainError::NotInTheBody);
    }
    Ok(())
}

pub fn can_apply(viewer: &Viewer, need: NeedSight<'_>, church: &Church) -> Result<(), DomainError> {
    if !need.is_open() {
        return Err(DomainError::NeedClosed);
    }
    if viewer.user.id == need.author_id {
        return Err(DomainError::OwnNeed);
    }
    if !can_view_need(viewer, need, church) {
        return match need.scope {
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

pub fn require_need_view(
    viewer: &Viewer,
    need: NeedSight<'_>,
    church: &Church,
) -> Result<(), DomainError> {
    if can_view_need(viewer, need, church) {
        Ok(())
    } else {
        Err(need_hidden(need))
    }
}

fn need_hidden(need: NeedSight<'_>) -> DomainError {
    match need.scope {
        Some(NeedScope::Church) => DomainError::OutsideChurch,
        Some(NeedScope::Neighboring) => DomainError::OutsideNeighborhood,
        _ => DomainError::NotInTheBody,
    }
}

pub fn can_endorse(from_id: &str, to_id: &str) -> Result<(), DomainError> {
    if from_id == to_id {
        Err(DomainError::SelfAction)
    } else {
        Ok(())
    }
}

pub fn can_decide_membership(actor: &User, church_id: &str) -> Result<(), DomainError> {
    if actor.governs(church_id) {
        Ok(())
    } else {
        Err(DomainError::NotGovernor)
    }
}

pub fn link_after_approval(current: ChurchLinkStatus) -> Result<ChurchLinkStatus, DomainError> {
    if current.is_waiting() {
        Ok(ChurchLinkStatus::Active)
    } else {
        Err(DomainError::NothingPending)
    }
}

pub fn visible_needs_near<'a>(
    viewer: &'a Viewer,
    cards: &'a [NeedCard],
    churches: &'a [Church],
    place: Place,
) -> impl Iterator<Item = &'a NeedCard> + 'a {
    cards.iter().filter(move |card| {
        church_for_card(card, churches).is_some_and(|church| {
            card.is_open() && can_view_need_near(viewer, card.sight(), church, place)
        })
    })
}

pub fn visible_need_cards<'a>(
    viewer: &'a Viewer,
    cards: &'a [NeedCard],
    churches: &'a [Church],
) -> impl Iterator<Item = &'a NeedCard> + 'a {
    cards
        .iter()
        .filter(move |card| card_is_visible(viewer, card, churches))
}

fn card_is_visible(viewer: &Viewer, card: &NeedCard, churches: &[Church]) -> bool {
    church_for_card(card, churches)
        .is_some_and(|church| card.is_open() && can_view_need(viewer, card.sight(), church))
}

fn church_for_card<'a>(card: &NeedCard, churches: &'a [Church]) -> Option<&'a Church> {
    churches.iter().find(|church| church.id == card.church_id)
}

pub fn invite_code_for(name: &str, nonce: &str) -> String {
    format!("{}-{}", slug_from(name, 8), nonce_from(nonce, 8)).to_ascii_lowercase()
}

fn slug_from(name: &str, take: usize) -> String {
    name.chars()
        .flat_map(|c| c.to_lowercase())
        .filter(|c| c.is_ascii_alphanumeric())
        .take(take)
        .collect()
}

fn nonce_from(nonce: &str, take: usize) -> String {
    nonce
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(take)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Need, NeedCard, User};

    const CEDAR_FALLS: (f64, f64) = (42.5349, -92.4453);
    const WATERLOO: (f64, f64) = (42.4928, -92.3426);
    const AUSTIN: (f64, f64) = (30.2672, -97.7431);

    fn user(id: &str) -> User {
        User {
            id: id.into(),
            first_name: id.into(),
            last_name: "Lane".into(),
            email: format!("{id}@ecclesia.test"),
            bio: String::new(),
            created_at: "2026-01-01T00:00:00Z".into(),
            church_id: None,
            church_status: None,
            church_role: None,
        }
    }

    fn linked(id: &str, church_id: &str, status: &str) -> User {
        let mut person = user(id);
        person.church_id = Some(church_id.into());
        person.church_status = Some(status.into());
        person.church_role = Some("member".into());
        person
    }

    fn church(id: &str, latitude: f64, longitude: f64) -> Church {
        Church {
            id: id.into(),
            name: id.into(),
            address: "100 Main Street".into(),
            latitude,
            longitude,
            country: "US".into(),
            description: String::new(),
            gathering: String::new(),
            owner_id: "owner".into(),
            invite_code: "code".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn viewer(user: User, church: Option<Church>) -> Viewer {
        Viewer {
            user,
            church,
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
    fn us_body_01_neighbors_are_within_40_km() {
        let grace = church("grace", CEDAR_FALLS.0, CEDAR_FALLS.1);
        let luke = church("luke", CEDAR_FALLS.0, CEDAR_FALLS.1);
        let mercy = church("mercy", WATERLOO.0, WATERLOO.1);
        let far = church("far", AUSTIN.0, AUSTIN.1);
        assert!(churches_are_neighbors(&grace, &luke));
        assert!(churches_are_neighbors(&grace, &mercy));
        assert!(!churches_are_neighbors(&grace, &far));
        assert!(!churches_are_neighbors(&grace, &grace));
    }

    #[test]
    fn us_need_07_a_shared_point_opens_a_church_need() {
        let grace = church("grace", CEDAR_FALLS.0, CEDAR_FALLS.1);
        let luke = church("luke", CEDAR_FALLS.0, CEDAR_FALLS.1);
        let far = church("far", AUSTIN.0, AUSTIN.1);
        let james = viewer(linked("james", "luke", "active"), Some(luke));
        let n = need(NeedScope::Church, "grace", "miriam");
        let here = Place {
            latitude: CEDAR_FALLS.0,
            longitude: CEDAR_FALLS.1,
        };
        let away = Place {
            latitude: AUSTIN.0,
            longitude: AUSTIN.1,
        };
        assert!(can_view_need_near(&james, n.sight(), &grace, here));
        assert!(!can_view_need_near(&james, n.sight(), &grace, away));
        assert!(can_reply(&james, n.sight(), &grace, NeedApproach::Near(here)).is_ok());
        assert_eq!(
            can_reply(&james, n.sight(), &far, NeedApproach::Near(here)),
            Err(DomainError::OutsideChurch)
        );
    }

    #[test]
    fn us_need_05_church_scope_hides_need_from_neighbors() {
        let grace = church("grace", CEDAR_FALLS.0, CEDAR_FALLS.1);
        let luke = church("luke", CEDAR_FALLS.0, CEDAR_FALLS.1);
        let james = viewer(linked("james", "luke", "active"), Some(luke));
        let n = need(NeedScope::Church, "grace", "miriam");
        assert!(!can_view_need(&james, n.sight(), &grace));
        assert_eq!(
            can_apply(&james, n.sight(), &grace),
            Err(DomainError::OutsideChurch)
        );
    }

    #[test]
    fn us_need_05_neighboring_scope_opens_need_to_same_region() {
        let grace = church("grace", CEDAR_FALLS.0, CEDAR_FALLS.1);
        let mercy = church("mercy", WATERLOO.0, WATERLOO.1);
        let elena = viewer(linked("elena", "mercy", "active"), Some(mercy));
        let n = need(NeedScope::Neighboring, "grace", "miriam");
        assert!(can_view_need(&elena, n.sight(), &grace));
        assert_eq!(can_apply(&elena, n.sight(), &grace), Ok(()));
    }

    #[test]
    fn us_need_05_pending_member_is_not_yet_in_the_body() {
        let grace = church("grace", CEDAR_FALLS.0, CEDAR_FALLS.1);
        let peter = viewer(
            linked("peter", "grace", "pending"),
            Some(grace.clone()),
        );
        let n = need(NeedScope::Body, "grace", "miriam");
        assert!(!can_view_need(&peter, n.sight(), &grace));
        assert_eq!(
            can_apply(&peter, n.sight(), &grace),
            Err(DomainError::NotInTheBody)
        );
    }

    #[test]
    fn us_need_02_author_cannot_apply_to_own_need() {
        let grace = church("grace", CEDAR_FALLS.0, CEDAR_FALLS.1);
        let miriam = viewer(
            linked("miriam", "grace", "active"),
            Some(grace.clone()),
        );
        let n = need(NeedScope::Church, "grace", "miriam");
        assert_eq!(
            can_apply(&miriam, n.sight(), &grace),
            Err(DomainError::OwnNeed)
        );
    }

    #[test]
    fn us_end_01_endorsement_cannot_target_self() {
        assert_eq!(can_endorse("a", "a"), Err(DomainError::SelfAction));
        assert_eq!(can_endorse("a", "b"), Ok(()));
    }

    #[test]
    fn us_mem_04_only_pending_links_can_be_decided() {
        assert_eq!(
            link_after_approval(ChurchLinkStatus::Pending),
            Ok(ChurchLinkStatus::Active)
        );
        assert_eq!(
            link_after_approval(ChurchLinkStatus::Active),
            Err(DomainError::NothingPending)
        );
    }

    #[test]
    fn invite_code_is_pure_given_a_nonce() {
        assert_eq!(
            invite_code_for("Grace Covenant", "k2m9p4r1"),
            "gracecov-k2m9p4r1"
        );
        assert_eq!(invite_code_for("Grace", "Ab12CdEf"), "grace-ab12cdef");
    }

    #[test]
    fn us_need_05_visible_cards_are_borrowed_not_cloned() {
        let grace = church("grace", CEDAR_FALLS.0, CEDAR_FALLS.1);
        let luke = church("luke", CEDAR_FALLS.0, CEDAR_FALLS.1);
        let james = viewer(linked("james", "luke", "active"), Some(luke));
        let hidden = NeedCard {
            id: "n1".into(),
            church_id: "grace".into(),
            church_name: "Grace".into(),
            church_address: "100 Main Street".into(),
            author_id: "miriam".into(),
            author_name: "Miriam".into(),
            title: "Meals".into(),
            body: "Tuesday".into(),
            gift_id: None,
            gift_name: None,
            scope: NeedScope::Church.as_str().into(),
            status: "open".into(),
            created_at: "t0".into(),
        };
        let open = NeedCard {
            id: "n2".into(),
            church_id: "grace".into(),
            church_name: "Grace".into(),
            church_address: "100 Main Street".into(),
            author_id: "miriam".into(),
            author_name: "Miriam".into(),
            title: "Spanish".into(),
            body: "Thursday".into(),
            gift_id: None,
            gift_name: None,
            scope: NeedScope::Neighboring.as_str().into(),
            status: "open".into(),
            created_at: "t0".into(),
        };
        let cards = [hidden, open];
        let churches = [grace];
        let visible: Vec<&NeedCard> = visible_need_cards(&james, &cards, &churches).collect();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, "n2");
        assert!(std::ptr::eq(visible[0], &cards[1]));
    }
}
