//! Human labels for stored status words. The database keeps snake_case.

use crate::leaf::{ApplicationStatus, Membership, MembershipRole, MembershipStatus};

pub fn census_line(members: i64, needs: i64) -> String {
    format!(
        "{} {} · {} {}",
        members,
        count_word(members, "member", "members"),
        needs,
        count_word(needs, "open need", "open needs")
    )
}

fn count_word(count: i64, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 { one } else { many }
}

pub fn role_word(role: Option<MembershipRole>) -> &'static str {
    role.map(MembershipRole::label).unwrap_or("Member")
}

pub fn household_line(membership: &Membership) -> &'static str {
    match membership.status() {
        Some(MembershipStatus::Active) => role_word(membership.role()),
        Some(status) => status.label(),
        None => role_word(membership.role()),
    }
}

pub fn offer_status_word(status: Option<ApplicationStatus>) -> &'static str {
    status.map(ApplicationStatus::label).unwrap_or("Offer")
}

pub fn category_label(category: &str) -> String {
    let mut chars = category.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
