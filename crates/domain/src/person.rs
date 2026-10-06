use serde::{Deserialize, Serialize};

use super::household::{Church, ChurchLinkStatus, MembershipRole};

pub fn display_name(first: &str, last: &str) -> String {
    let mut name = String::with_capacity(first.len() + last.len() + 1);
    name.push_str(first);
    name.push(' ');
    name.push_str(last);
    name
}

/// One row in the membership join: this person and this church.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Membership {
    pub church_id: String,
    pub status: String,
    pub role: String,
}

impl Membership {
    pub fn status(&self) -> Option<ChurchLinkStatus> {
        ChurchLinkStatus::parse(&self.status)
    }

    pub fn role(&self) -> Option<MembershipRole> {
        MembershipRole::parse(&self.role)
    }

    pub fn is_active(&self) -> bool {
        self.status() == Some(ChurchLinkStatus::Active)
    }

    pub fn is_waiting(&self) -> bool {
        self.status().is_some_and(ChurchLinkStatus::is_waiting)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct User {
    pub id: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub bio: String,
    pub created_at: String,
    pub memberships: Vec<Membership>,
}

impl User {
    pub fn membership_in(&self, church_id: &str) -> Option<&Membership> {
        self.memberships
            .iter()
            .find(|link| link.church_id == church_id)
    }

    pub fn has_church(&self) -> bool {
        !self.memberships.is_empty()
    }

    pub fn is_active(&self) -> bool {
        self.memberships.iter().any(Membership::is_active)
    }

    pub fn is_waiting(&self) -> bool {
        self.memberships.iter().any(Membership::is_waiting)
    }

    pub fn is_active_in(&self, church_id: &str) -> bool {
        self.membership_in(church_id)
            .is_some_and(Membership::is_active)
    }

    pub fn governs(&self, church_id: &str) -> bool {
        self.membership_in(church_id).is_some_and(|link| {
            link.is_active() && link.role().is_some_and(MembershipRole::can_govern)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gift {
    pub id: String,
    pub name: String,
    pub category: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MemberGift {
    pub user_id: String,
    pub gift_id: String,
    pub note: String,
    pub gift_name: String,
    pub category: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndorsementStatus {
    Pending,
    Accepted,
    Declined,
}

impl EndorsementStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Declined => "declined",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "accepted" => Some(Self::Accepted),
            "declined" => Some(Self::Declined),
            _ => None,
        }
    }
}

impl Endorsement {
    pub fn status(&self) -> Option<EndorsementStatus> {
        EndorsementStatus::parse(&self.status)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Endorsement {
    pub id: String,
    pub from_user_id: String,
    pub to_user_id: String,
    pub gift_id: String,
    pub skill: String,
    pub note: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EndorsementCard {
    pub id: String,
    pub from_user_id: String,
    pub from_user_name: String,
    pub to_user_id: String,
    pub to_user_name: String,
    pub gift_id: String,
    pub gift_name: String,
    pub note: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Notification {
    pub id: String,
    pub user_id: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub href: String,
    pub read: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChurchMember {
    pub user_id: String,
    pub first_name: String,
    pub last_name: String,
    pub role: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Viewer {
    pub user: User,
    pub churches: Vec<Church>,
    pub gift_ids: Vec<String>,
}

impl Viewer {
    pub fn active_churches(&self) -> impl Iterator<Item = &Church> {
        self.churches
            .iter()
            .filter(|church| self.user.is_active_in(&church.id))
    }

    pub fn waiting_churches(&self) -> impl Iterator<Item = &Church> {
        self.churches.iter().filter(|church| {
            self.user
                .membership_in(&church.id)
                .is_some_and(Membership::is_waiting)
        })
    }

    pub fn is_active_anywhere(&self) -> bool {
        self.user.is_active()
    }

    pub fn is_active_in(&self, church_id: &str) -> bool {
        self.user.is_active_in(church_id)
    }

    pub fn can_govern(&self, church_id: &str) -> bool {
        self.user.governs(church_id)
    }

    pub fn has_gift(&self, gift_id: &str) -> bool {
        self.gift_ids.iter().any(|id| id == gift_id)
    }
}

impl ChurchMember {
    pub fn role(&self) -> Option<MembershipRole> {
        MembershipRole::parse(&self.role)
    }

    pub fn status(&self) -> Option<ChurchLinkStatus> {
        ChurchLinkStatus::parse(&self.status)
    }

    pub fn is_active(&self) -> bool {
        self.status() == Some(ChurchLinkStatus::Active)
    }

    pub fn is_pending(&self) -> bool {
        matches!(
            self.status(),
            Some(ChurchLinkStatus::Pending | ChurchLinkStatus::Invited)
        )
    }
}
