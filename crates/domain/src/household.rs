use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Church {
    pub id: String,
    pub name: String,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
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

    pub fn label(self) -> &'static str {
        match self {
            Self::Owner => "Pastor",
            Self::Steward => "Steward",
            Self::Member => "Member",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChurchLinkStatus {
    Pending,
    Invited,
    Active,
}

impl ChurchLinkStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Invited => "invited",
            Self::Active => "active",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "invited" => Some(Self::Invited),
            "active" => Some(Self::Active),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "Waiting",
            Self::Invited => "Invited",
            Self::Active => "Active",
        }
    }

    pub fn is_waiting(self) -> bool {
        matches!(self, Self::Pending | Self::Invited)
    }
}
