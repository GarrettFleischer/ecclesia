use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct Church {
    pub id: String,
    pub name: String,
    pub city: String,
    pub region: String,
    pub country: String,
    pub description: String,
    pub gathering: String,
    pub owner_id: String,
    pub invite_code: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MembershipRole {
    Owner,
    Steward,
    Member,
}

impl MembershipRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Steward => "steward",
            Self::Member => "member",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "owner" => Some(Self::Owner),
            "steward" => Some(Self::Steward),
            "member" => Some(Self::Member),
            _ => None,
        }
    }

    pub fn can_govern(self) -> bool {
        matches!(self, Self::Owner | Self::Steward)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MembershipStatus {
    PendingRequest,
    PendingInvite,
    Active,
    Declined,
}

impl MembershipStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PendingRequest => "pending_request",
            Self::PendingInvite => "pending_invite",
            Self::Active => "active",
            Self::Declined => "declined",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending_request" => Some(Self::PendingRequest),
            "pending_invite" => Some(Self::PendingInvite),
            "active" => Some(Self::Active),
            "declined" => Some(Self::Declined),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct Membership {
    pub id: String,
    pub church_id: String,
    pub user_id: String,
    pub role: String,
    pub status: String,
    pub created_at: String,
}

impl Membership {
    pub fn role(&self) -> Option<MembershipRole> {
        MembershipRole::parse(&self.role)
    }

    pub fn status(&self) -> Option<MembershipStatus> {
        MembershipStatus::parse(&self.status)
    }

    pub fn is_active(&self) -> bool {
        self.status() == Some(MembershipStatus::Active)
    }

    pub fn is_pending(&self) -> bool {
        matches!(
            self.status(),
            Some(MembershipStatus::PendingRequest | MembershipStatus::PendingInvite)
        )
    }

    pub fn can_govern(&self) -> bool {
        self.is_active() && self.role().is_some_and(MembershipRole::can_govern)
    }
}

/// Pair each membership with the church it names. Both sides stay borrowed.
pub fn pair_memberships<'a>(
    memberships: impl IntoIterator<Item = &'a Membership>,
    churches: &'a [Church],
) -> impl Iterator<Item = (&'a Church, &'a Membership)> {
    memberships.into_iter().filter_map(move |membership| {
        church_named(churches, &membership.church_id).map(|church| (church, membership))
    })
}

fn church_named<'a>(churches: &'a [Church], id: &str) -> Option<&'a Church> {
    churches.iter().find(|church| church.id == id)
}

pub fn unique_church_ids(memberships: &[Membership]) -> Vec<&str> {
    let mut ids = Vec::new();
    for membership in memberships {
        push_unique_id(&mut ids, membership.church_id.as_str());
    }
    ids
}

fn push_unique_id<'a>(ids: &mut Vec<&'a str>, id: &'a str) {
    if ids.iter().any(|existing| *existing == id) {
        return;
    }
    ids.push(id);
}
