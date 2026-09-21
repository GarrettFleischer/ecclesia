//! Named states that used to travel as boolean arguments.
//!
//! A caller names the situation (`EmailAvailability::Taken`) instead of
//! passing `true`. Two-way decisions are separate functions
//! (`approve_membership` / `decline_membership`), not a switch on a flag.

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

/// Whether an endorsement for this pair and skill is already waiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndorsementQueue {
    Clear,
    Waiting,
}

/// Whether the named person already lists this catalog gift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GiftOnProfile {
    Named,
    Absent,
}

impl GiftOnProfile {
    pub fn of_ids(ids: &[String], gift_id: &str) -> Self {
        if gift_id.is_empty() {
            return Self::Absent;
        }
        if ids.iter().any(|id| id == gift_id) {
            Self::Named
        } else {
            Self::Absent
        }
    }
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
        match value.map(str::trim) {
            Some("1") => Self::Open,
            Some(value) if value.eq_ignore_ascii_case("true") => Self::Open,
            Some(value) if value.eq_ignore_ascii_case("open") => Self::Open,
            _ => Self::Sealed,
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

/// Whether the words lift people up. The skin weighs them; the leaf only sees this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Posture {
    Lifts,
    TearsDown,
}

pub fn require_uplifting(posture: Posture) -> Result<(), super::model::DomainError> {
    match posture {
        Posture::Lifts => Ok(()),
        Posture::TearsDown => Err(super::model::DomainError::TearsDown),
    }
}

/// What kind of words the person is writing. The skin uses this to ask the right question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceKind {
    Need,
    Offer,
    Endorsement,
    GiftNote,
    Bio,
    Church,
}

impl VoiceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Need => "need",
            Self::Offer => "offer",
            Self::Endorsement => "endorsement",
            Self::GiftNote => "gift",
            Self::Bio => "bio",
            Self::Church => "church",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "need" => Some(Self::Need),
            "offer" => Some(Self::Offer),
            "endorsement" => Some(Self::Endorsement),
            "gift" => Some(Self::GiftNote),
            "bio" => Some(Self::Bio),
            "church" => Some(Self::Church),
            _ => None,
        }
    }
}

/// First submit is a review of the rewrite. Second submit publishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoicePass {
    Review,
    Publish,
}

impl VoicePass {
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "publish" => Self::Publish,
            _ => Self::Review,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Publish => "publish",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leaf::DomainError;

    #[test]
    fn us_tone_01_tears_down_is_refused() {
        assert_eq!(require_uplifting(Posture::Lifts), Ok(()));
        assert_eq!(
            require_uplifting(Posture::TearsDown),
            Err(DomainError::TearsDown)
        );
    }

    #[test]
    fn us_refine_02_first_pass_is_review() {
        assert_eq!(VoicePass::parse(""), VoicePass::Review);
        assert_eq!(VoicePass::parse("review"), VoicePass::Review);
        assert_eq!(VoicePass::parse("publish"), VoicePass::Publish);
    }

    #[test]
    fn us_sec_09_demo_stays_sealed_unless_asked() {
        assert_eq!(DemoSeat::from_env_value(None), DemoSeat::Sealed);
        assert_eq!(DemoSeat::from_env_value(Some("0")), DemoSeat::Sealed);
        assert_eq!(DemoSeat::from_env_value(Some("1")), DemoSeat::Open);
        assert_eq!(DemoSeat::from_env_value(Some("true")), DemoSeat::Open);
    }
}
