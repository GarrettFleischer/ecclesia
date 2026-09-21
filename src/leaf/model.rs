use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub city: String,
    pub region: String,
    pub bio: String,
    pub created_at: String,
}

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

    pub fn can_govern(&self) -> bool {
        self.is_active() && self.role().is_some_and(MembershipRole::can_govern)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct Gift {
    pub id: String,
    pub name: String,
    pub category: String,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct MemberGift {
    pub user_id: String,
    pub gift_id: String,
    pub note: String,
    pub gift_name: String,
    pub category: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeedScope {
    Church,
    Neighboring,
    Body,
}

impl NeedScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Church => "church",
            Self::Neighboring => "neighboring",
            Self::Body => "body",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "church" => Some(Self::Church),
            "neighboring" => Some(Self::Neighboring),
            "body" => Some(Self::Body),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Church => "This church",
            Self::Neighboring => "Neighboring churches",
            Self::Body => "The whole body",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct Need {
    pub id: String,
    pub church_id: String,
    pub author_id: String,
    pub title: String,
    pub body: String,
    pub gift_id: Option<String>,
    pub scope: String,
    pub status: String,
    pub created_at: String,
}

impl Need {
    pub fn scope(&self) -> Option<NeedScope> {
        NeedScope::parse(&self.scope)
    }

    pub fn is_open(&self) -> bool {
        self.status == "open"
    }

    pub fn from_card(card: &NeedCard) -> Self {
        Self {
            id: card.id.clone(),
            church_id: card.church_id.clone(),
            author_id: card.author_id.clone(),
            title: card.title.clone(),
            body: card.body.clone(),
            gift_id: card.gift_id.clone(),
            scope: card.scope.clone(),
            status: card.status.clone(),
            created_at: card.created_at.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct NeedCard {
    pub id: String,
    pub church_id: String,
    pub church_name: String,
    pub church_city: String,
    pub church_region: String,
    pub author_id: String,
    pub author_name: String,
    pub title: String,
    pub body: String,
    pub gift_id: Option<String>,
    pub gift_name: Option<String>,
    pub scope: String,
    pub status: String,
    pub created_at: String,
}

impl NeedCard {
    pub fn scope(&self) -> Option<NeedScope> {
        NeedScope::parse(&self.scope)
    }

    pub fn is_open(&self) -> bool {
        self.status == "open"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct Application {
    pub id: String,
    pub need_id: String,
    pub user_id: String,
    pub message: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct ApplicationCard {
    pub id: String,
    pub need_id: String,
    pub user_id: String,
    pub user_name: String,
    pub message: String,
    pub status: String,
    pub created_at: String,
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
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct Endorsement {
    pub id: String,
    pub from_user_id: String,
    pub to_user_id: String,
    pub gift_id: String,
    pub note: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
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
        self.churches.iter().filter(|church| {
            self.memberships
                .iter()
                .any(|membership| membership.church_id == church.id && membership.is_active())
        })
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
        self.memberships.iter().filter(|membership| {
            matches!(
                membership.status.as_str(),
                "pending_request" | "pending_invite"
            )
        })
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("You cannot do that for yourself.")]
    SelfAction,
    #[error("Only an active member of a church can do that.")]
    NotInTheBody,
    #[error("This need stays inside its own church.")]
    OutsideChurch,
    #[error("This need is open to neighboring churches in the same city or region.")]
    OutsideNeighborhood,
    #[error("That need is no longer open.")]
    NeedClosed,
    #[error("You already asked to help.")]
    AlreadyApplied,
    #[error("The author of a need cannot apply to it.")]
    OwnNeed,
    #[error("Only a pastor or steward of this church can do that.")]
    NotGovernor,
    #[error("There is nothing pending to decide.")]
    NothingPending,
    #[error("You are already part of this church.")]
    AlreadyMember,
    #[error("An endorsement for that gift is already waiting.")]
    DuplicateEndorsement,
    #[error("A few required fields are empty or too long.")]
    InvalidInput,
    #[error("That email is not a usable address.")]
    InvalidEmail,
    #[error("That email is already at the table.")]
    EmailTaken,
    #[error("That gift is not in the catalog.")]
    UnknownGift,
    #[error("We could not find that.")]
    NotFound,
    #[error("Demo impersonation is turned off.")]
    DemoDisabled,
}

impl DomainError {
    pub fn flash_code(self) -> &'static str {
        match self {
            Self::SelfAction => "self",
            Self::NotInTheBody => "not_member",
            Self::OutsideChurch | Self::OutsideNeighborhood => "scope",
            Self::NeedClosed => "closed",
            Self::AlreadyApplied | Self::AlreadyMember | Self::DuplicateEndorsement => "already",
            Self::OwnNeed => "own_need",
            Self::NotGovernor => "forbidden",
            Self::NothingPending => "pending",
            Self::InvalidInput => "missing",
            Self::InvalidEmail => "bad_email",
            Self::EmailTaken => "email",
            Self::UnknownGift => "missing",
            Self::NotFound => "not_found",
            Self::DemoDisabled => "forbidden",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeDraft {
    pub user_id: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub href: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Write {
    InsertUser(User),
    UpdateUser {
        id: String,
        name: String,
        city: String,
        region: String,
        bio: String,
    },
    InsertChurch(Church),
    InsertMembership(Membership),
    SetMembershipStatus {
        id: String,
        status: String,
    },
    InsertNeed(Need),
    SetNeedStatus {
        id: String,
        status: String,
    },
    InsertApplication(Application),
    SetApplicationStatus {
        id: String,
        status: String,
    },
    InsertEndorsement(Endorsement),
    SetEndorsementStatus {
        id: String,
        status: String,
    },
    UpsertMemberGift {
        user_id: String,
        gift_id: String,
        note: String,
    },
    RemoveMemberGift {
        user_id: String,
        gift_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Effect {
    pub writes: Vec<Write>,
    pub notices: Vec<NoticeDraft>,
}

impl Effect {
    pub fn write(write: Write) -> Self {
        Self {
            writes: vec![write],
            notices: vec![],
        }
    }

    pub fn with_notice(mut self, notice: NoticeDraft) -> Self {
        self.notices.push(notice);
        self
    }

    pub fn push(&mut self, write: Write) {
        self.writes.push(write);
    }

    pub fn memberships(&self) -> impl Iterator<Item = &Membership> {
        self.writes.iter().filter_map(|write| match write {
            Write::InsertMembership(membership) => Some(membership),
            _ => None,
        })
    }

    pub fn inserted_user_id(&self) -> Option<String> {
        self.writes.iter().find_map(|write| match write {
            Write::InsertUser(user) => Some(user.id.clone()),
            _ => None,
        })
    }

    pub fn inserted_church_id(&self) -> Option<String> {
        self.writes.iter().find_map(|write| match write {
            Write::InsertChurch(church) => Some(church.id.clone()),
            _ => None,
        })
    }

    pub fn inserted_need_id(&self) -> Option<String> {
        self.writes.iter().find_map(|write| match write {
            Write::InsertNeed(need) => Some(need.id.clone()),
            _ => None,
        })
    }
}
