use std::sync::Arc;

use super::household::Church;
use super::need::{Application, Need, NeedReply, Prayer, Share};
use super::person::{Endorsement, User};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("You can't do that for yourself.")]
    SelfAction,
    #[error("Join a church to see this.")]
    NotInTheBody,
    #[error("You're not in a church.")]
    NoChurch,
    #[error("Close the church or name the next pastor.")]
    PastorHoldsChurch,
    #[error("That person already pastors this church. Pick someone else.")]
    AlreadyPastor,
    #[error("That church is still open.")]
    ChurchStillOpen,
    #[error("This need is only open to its church.")]
    OutsideChurch,
    #[error("This need is only open to churches nearby.")]
    OutsideNeighborhood,
    #[error("This need is closed.")]
    NeedClosed,
    #[error("This need is open.")]
    NeedOpen,
    #[error("This need is archived.")]
    NeedArchived,
    #[error("You already applied.")]
    AlreadyApplied,
    #[error("This is your need.")]
    OwnNeed,
    #[error("Only the pastor can do that.")]
    NotGovernor,
    #[error("Only the author or a pastor can do that.")]
    NotSteward,
    #[error("This endorsement is not yours.")]
    NotRecipient,
    #[error("This invite is not yours.")]
    NotInvitee,
    #[error("Nothing to decide.")]
    NothingPending,
    #[error("You're already in this church.")]
    AlreadyMember,
    #[error("You already endorsed them for this.")]
    DuplicateEndorsement,
    #[error("Fill in the required fields.")]
    InvalidInput,
    #[error("Check the employer identification number.")]
    InvalidEin,
    #[error("Check the state registration number.")]
    InvalidRegistry,
    #[error("Check the ZIP code.")]
    InvalidPostal,
    #[error("Check the service time.")]
    InvalidService,
    #[error("Check the email address.")]
    InvalidEmail,
    #[error("An account with that email already exists.")]
    EmailTaken,
    #[error("Pick a gift from the list.")]
    UnknownGift,
    #[error("Not found.")]
    NotFound,
    #[error("Pick a stronger password.")]
    WeakPassword,
    #[error("Write it so it lifts someone up.")]
    TearsDown,
    #[error("You already marked this prayer.")]
    AlreadyMarked,
    #[error("Only the author can do that.")]
    NotAuthor,
    #[error("This prayer is already answered.")]
    PrayerAnswered,
}

impl DomainError {
    pub fn flash_code(&self) -> &'static str {
        match self {
            Self::SelfAction => "self",
            Self::NotInTheBody | Self::NoChurch => "not_member",
            Self::PastorHoldsChurch => "pastor",
            Self::AlreadyPastor => "already_pastor",
            Self::ChurchStillOpen => "still_open",
            Self::OutsideChurch | Self::OutsideNeighborhood => "scope",
            Self::NeedClosed => "closed",
            Self::NeedOpen => "open",
            Self::NeedArchived => "archived",
            Self::AlreadyApplied | Self::AlreadyMember | Self::DuplicateEndorsement => "already",
            Self::OwnNeed => "own_need",
            Self::NotGovernor => "forbidden",
            Self::NotSteward => "steward",
            Self::NotRecipient | Self::NotInvitee => "not_yours",
            Self::NothingPending => "pending",
            Self::InvalidInput => "missing",
            Self::InvalidEin => "ein",
            Self::InvalidRegistry => "registry",
            Self::InvalidPostal => "postal",
            Self::InvalidService => "service",
            Self::InvalidEmail => "bad_email",
            Self::EmailTaken => "email",
            Self::UnknownGift => "missing",
            Self::NotFound => "not_found",
            Self::WeakPassword => "password",
            Self::TearsDown => "tone",
            Self::AlreadyMarked => "already",
            Self::NotAuthor => "not_yours",
            Self::PrayerAnswered => "prayer_answered",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeDraft {
    pub user_id: String,
    pub kind: &'static str,
    pub title: Arc<str>,
    pub body: &'static str,
    pub href: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Write {
    InsertUser(User),
    UpdateUser {
        id: String,
        first_name: String,
        last_name: String,
        bio: String,
    },
    InsertChurch(Church),
    UpsertMembership {
        user_id: String,
        church_id: String,
        status: String,
        role: String,
    },
    DeleteMembership {
        user_id: String,
        church_id: String,
    },
    SetChurchOwner {
        church_id: String,
        owner_id: String,
    },
    CloseChurch {
        id: String,
        deleted_at: String,
    },
    InsertNeed(Need),
    InsertShare(Share),
    DeleteNeedShare {
        target_id: String,
    },
    SetNeedStatus {
        id: String,
        status: &'static str,
        closed_at: Option<String>,
        praise: Option<String>,
    },
    MoveNeed {
        id: String,
        church_id: String,
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
    InsertNeedReply(NeedReply),
    InsertPrayer(Prayer),
    SetPrayerAnswered {
        id: String,
        praise: String,
        answered_at: String,
    },
    UpsertPrayerMark {
        user_id: String,
        prayer_id: String,
        day: String,
        kind: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq, Default)]
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

    /// Church id written by a join, a code, or a plant. A removed membership is not one.
    pub fn linked_church_id(&self) -> Option<&str> {
        self.writes.iter().find_map(|write| match write {
            Write::UpsertMembership { church_id, .. } => Some(church_id.as_str()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_church_id_ignores_a_removed_membership() {
        let mut effect = Effect::default();
        effect.push(Write::DeleteMembership {
            user_id: "u".into(),
            church_id: "grace".into(),
        });
        assert!(effect.linked_church_id().is_none());
        effect.push(Write::UpsertMembership {
            user_id: "u".into(),
            church_id: "grace".into(),
            status: "pending".into(),
            role: "member".into(),
        });
        assert_eq!(effect.linked_church_id(), Some("grace"));
    }
}
