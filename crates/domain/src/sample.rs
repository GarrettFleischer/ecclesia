//! Shared sample records for Domain and SDK tests.

use super::model::{Church, User, Viewer};

pub fn user(id: &str) -> User {
    user_named(id, id, "Lane")
}

pub fn user_named(id: &str, first_name: &str, last_name: &str) -> User {
    User {
        id: id.into(),
        first_name: first_name.into(),
        last_name: last_name.into(),
        email: format!("{id}@ecclesia.test"),
        bio: String::new(),
        created_at: "t0".into(),
        church_id: None,
        church_status: None,
        church_role: None,
    }
}

pub fn user_in_church(id: &str, church_id: &str, role: &str, status: &str) -> User {
    let mut person = user(id);
    person.church_id = Some(church_id.into());
    person.church_role = Some(role.into());
    person.church_status = Some(status.into());
    person
}

pub fn church(id: &str) -> Church {
    church_at(id, 42.5349, -92.4453)
}

pub fn church_at(id: &str, latitude: f64, longitude: f64) -> Church {
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
        created_at: "t0".into(),
    }
}

pub fn viewer_of(user: User, church: Option<Church>) -> Viewer {
    Viewer {
        user,
        church,
        gift_ids: vec![],
    }
}
