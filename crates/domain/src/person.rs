use serde::{Deserialize, Serialize};

use super::household::{Church, Membership};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub city: String,
    pub region: String,
    pub bio: String,
    pub created_at: String,
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
    pub membership_id: String,
    pub user_id: String,
    pub name: String,
    pub city: String,
    pub role: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viewer {
    pub user: User,
    pub memberships: Vec<Membership>,
    pub churches: Vec<Church>,
    pub gift_ids: Vec<String>,
}

impl Viewer {
    pub fn active_churches(&self) -> impl Iterator<Item = &Church> {
        self.churches
            .iter()
            .filter(|church| self.is_active_in(&church.id))
    }

    pub fn is_active_anywhere(&self) -> bool {
        self.memberships.iter().any(Membership::is_active)
    }

    pub fn is_active_in(&self, church_id: &str) -> bool {
        self.memberships
            .iter()
            .any(|membership| membership.church_id == church_id && membership.is_active())
    }

    pub fn membership_in(&self, church_id: &str) -> Option<&Membership> {
        self.memberships
            .iter()
            .find(|membership| membership.church_id == church_id)
    }

    pub fn can_govern(&self, church_id: &str) -> bool {
        self.membership_in(church_id)
            .is_some_and(Membership::can_govern)
    }

    pub fn has_gift(&self, gift_id: &str) -> bool {
        self.gift_ids.iter().any(|id| id == gift_id)
    }

    pub fn pending_memberships(&self) -> impl Iterator<Item = &Membership> {
        self.memberships
            .iter()
            .filter(|membership| membership.is_pending())
    }
}

impl ChurchMember {
    pub fn role(&self) -> Option<super::household::MembershipRole> {
        super::household::MembershipRole::parse(&self.role)
    }

    pub fn status(&self) -> Option<super::household::MembershipStatus> {
        super::household::MembershipStatus::parse(&self.status)
    }

    pub fn is_active(&self) -> bool {
        self.status() == Some(super::household::MembershipStatus::Active)
    }

    pub fn is_pending(&self) -> bool {
        matches!(
            self.status(),
            Some(
                super::household::MembershipStatus::PendingRequest
                    | super::household::MembershipStatus::PendingInvite
            )
        )
    }
}
