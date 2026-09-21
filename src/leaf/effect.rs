use super::household::{Church, Membership};
use super::need::{Application, Need};
use super::person::{Endorsement, User};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("You can't do that for yourself.")]
    SelfAction,
    #[error("Join a church to apply.")]
    NotInTheBody,
    #[error("Only members of this church can apply.")]
    OutsideChurch,
    #[error("Only churches in the same city or region can apply.")]
    OutsideNeighborhood,
    #[error("This need is closed.")]
    NeedClosed,
    #[error("You already applied.")]
    AlreadyApplied,
    #[error("This is your need.")]
    OwnNeed,
    #[error("Only the pastor can do that.")]
    NotGovernor,
    #[error("Nothing to decide.")]
    NothingPending,
    #[error("You're already in this church.")]
    AlreadyMember,
    #[error("You already endorsed them for this.")]
    DuplicateEndorsement,
    #[error("Fill in the required fields.")]
    InvalidInput,
    #[error("Check the email address.")]
    InvalidEmail,
    #[error("An account with that email already exists.")]
    EmailTaken,
    #[error("Pick a gift from the list.")]
    UnknownGift,
    #[error("Not found.")]
    NotFound,
    #[error("Demo sign-in is off.")]
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
