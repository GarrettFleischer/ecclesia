use super::household::{Church, Membership};
use super::need::{Application, Need};
use super::person::{Endorsement, User};

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
    pub kind: &'static str,
    pub title: String,
    pub body: &'static str,
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
        status: &'static str,
    },
    InsertNeed(Need),
    SetNeedStatus {
        id: String,
        status: &'static str,
    },
    InsertApplication(Application),
    SetApplicationStatus {
        id: String,
        status: &'static str,
    },
    InsertEndorsement(Endorsement),
    SetEndorsementStatus {
        id: String,
        status: &'static str,
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

    pub fn inserted_user_id(&self) -> Option<&str> {
        self.writes.iter().find_map(|write| match write {
            Write::InsertUser(user) => Some(user.id.as_str()),
            _ => None,
        })
    }

    pub fn inserted_church_id(&self) -> Option<&str> {
        self.writes.iter().find_map(|write| match write {
            Write::InsertChurch(church) => Some(church.id.as_str()),
            _ => None,
        })
    }

    pub fn inserted_need_id(&self) -> Option<&str> {
        self.writes.iter().find_map(|write| match write {
            Write::InsertNeed(need) => Some(need.id.as_str()),
            _ => None,
        })
    }
}
