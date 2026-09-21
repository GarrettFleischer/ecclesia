//! Named states that used to travel as boolean arguments.
//!
//! A caller names the situation (`EmailAvailability::Taken`) instead of
//! passing `true` and hoping the next reader remembers what it meant.

/// Whether an email may still take a seat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailAvailability {
    Free,
    Taken,
}

impl EmailAvailability {
    pub fn of_existing<T>(existing: Option<T>) -> Self {
        match existing {
            Some(_) => Self::Taken,
            None => Self::Free,
        }
    }
}

/// Whether a catalog gift id is real.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogPresence {
    Listed,
    Unknown,
}

impl CatalogPresence {
    pub fn of_lookup<T>(found: Option<T>) -> Self {
        match found {
            Some(_) => Self::Listed,
            None => Self::Unknown,
        }
    }
}

/// Whether someone has already offered on this need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorOffer {
    Fresh,
    AlreadyMade,
}

impl PriorOffer {
    pub fn of_existing<T>(existing: Option<T>) -> Self {
        match existing {
            Some(_) => Self::AlreadyMade,
            None => Self::Fresh,
        }
    }
}

/// Whether an endorsement for this pair and gift is already waiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndorsementQueue {
    Clear,
    Waiting,
}

impl EndorsementQueue {
    pub fn of_existing<T>(existing: Option<T>) -> Self {
        match existing {
            Some(_) => Self::Waiting,
            None => Self::Clear,
        }
    }
}

/// Demo impersonation is a skin setting, not a household rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoSeat {
    Open,
    Sealed,
}

impl DemoSeat {
    pub fn from_env_value(value: Option<&str>) -> Self {
        match value {
            Some("0") => Self::Sealed,
            Some(value) if value.eq_ignore_ascii_case("false") => Self::Sealed,
            _ => Self::Open,
        }
    }

    pub fn is_open(self) -> bool {
        matches!(self, Self::Open)
    }
}

/// Pastor decision on a membership request or invite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MembershipDoor {
    Open,
    Shut,
}

impl MembershipDoor {
    pub fn flash_code(self) -> &'static str {
        match self {
            Self::Open => "approved",
            Self::Shut => "declined",
        }
    }
}

/// Author or governor decision on an offer to help.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferVerdict {
    Receive,
    Pass,
}

impl OfferVerdict {
    pub fn flash_code(self) -> &'static str {
        match self {
            Self::Receive => "application_accepted",
            Self::Pass => "declined",
        }
    }
}

/// The named person deciding whether to wear an endorsement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndorsementVerdict {
    Wear,
    Decline,
}

impl EndorsementVerdict {
    pub fn flash_code(self) -> &'static str {
        match self {
            Self::Wear => "endorsement_accepted",
            Self::Decline => "endorsement_declined",
        }
    }
}

/// Whether this viewer has already applied to the need being shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferState {
    NotYet,
    AlreadyOffered,
}

impl OfferState {
    pub fn of_existing<T>(existing: Option<T>) -> Self {
        match existing {
            Some(_) => Self::AlreadyOffered,
            None => Self::NotYet,
        }
    }
}
