//! Shared sample records for Domain and SDK tests.

use super::model::{Church, Membership, User, Viewer};

pub fn user(id: &str) -> User {
    user_named(id, id)
}

pub fn user_named(id: &str, name: &str) -> User {
    User {
        id: id.into(),
        name: name.into(),
        email: format!("{id}@ecclesia.test"),
        city: "Cedar Falls".into(),
        region: "Iowa".into(),
        bio: String::new(),
        created_at: "t0".into(),
    }
}

pub fn church(id: &str) -> Church {
    church_at(id, "Cedar Falls", "Iowa")
}

pub fn church_at(id: &str, city: &str, region: &str) -> Church {
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
        created_at: "t0".into(),
    }
}

pub fn membership(
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

pub fn viewer_of(user: User, memberships: Vec<Membership>, churches: Vec<Church>) -> Viewer {
    Viewer {
        user,
        memberships,
        churches,
        gift_ids: vec![],
    }
}
