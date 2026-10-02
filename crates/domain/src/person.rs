use serde::{Deserialize, Serialize};

use super::household::{Church, ChurchLinkStatus, MembershipRole};

pub fn display_name(first: &str, last: &str) -> String {
    let mut name = String::with_capacity(first.len() + last.len() + 1);
    name.push_str(first);
    name.push(' ');
    name.push_str(last);
    name
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct User {
    pub id: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub bio: String,
    pub created_at: String,
    pub church_id: Option<String>,
    pub church_status: Option<String>,
    pub church_role: Option<String>,
}

impl User {
    pub fn link_status(&self) -> Option<ChurchLinkStatus> {
        self.church_status
            .as_deref()
            .and_then(ChurchLinkStatus::parse)
    }

    pub fn link_role(&self) -> Option<MembershipRole> {
        self.church_role.as_deref().and_then(MembershipRole::parse)
    }

    pub fn is_active(&self) -> bool {
        self.link_status() == Some(ChurchLinkStatus::Active)
    }

    pub fn is_waiting(&self) -> bool {
        matches!(
            self.link_status(),
            Some(ChurchLinkStatus::Pending | ChurchLinkStatus::Invited)
        )
    }

    pub fn is_active_in(&self, church_id: &str) -> bool {
        self.is_active() && self.church_id.as_deref() == Some(church_id)
    }

    pub fn governs(&self, church_id: &str) -> bool {
        self.is_active_in(church_id) && self.link_role().is_some_and(MembershipRole::can_govern)
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
    pub church: Option<Church>,
    pub gift_ids: Vec<String>,
}

impl Viewer {
    pub fn active_church(&self) -> Option<&Church> {
        self.church.as_ref().filter(|_| self.user.is_active())
    }

    pub fn waiting_church(&self) -> Option<&Church> {
        self.church.as_ref().filter(|_| self.user.is_waiting())
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
