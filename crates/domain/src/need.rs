//! Need, reply, and prayer records.
//!
//! Stories that change these records live in [`crate::needs`]. Photo limits live
//! in [`crate::media`].

use serde::{Deserialize, Serialize};

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
            Self::Neighboring => "Nearby churches",
            Self::Body => "Everyone",
        }
    }
}

/// Days a met need stays in lists before its share code is removed.
pub const MET_NEED_DAYS: i64 = 30;

/// Whether a need still belongs in lists. `archived` is 0 or 1 in the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeedShelf {
    Listed,
    Archived,
}

impl NeedShelf {
    pub fn from_flag(flag: i64) -> Self {
        match flag {
            0 => Self::Listed,
            _ => Self::Archived,
        }
    }

    pub fn flag(self) -> i64 {
        match self {
            Self::Listed => 0,
            Self::Archived => 1,
        }
    }
}

/// When a newly issued need code stops working. Listed needs do not expire.
pub fn share_expires_on(shelf: NeedShelf, window_end: &str) -> Option<String> {
    match shelf {
        NeedShelf::Listed => None,
        NeedShelf::Archived => Some(window_end.to_string()),
    }
}

/// What a short code points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareKind {
    Need,
}

impl ShareKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Need => "need",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "need" => Some(Self::Need),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Share {
    pub code: String,
    pub kind: String,
    pub target_id: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
    pub closed_at: Option<String>,
    pub praise: Option<String>,
    pub shelf: NeedShelf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeedStatus {
    Open,
    Closed,
}

impl NeedStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "open" => Some(Self::Open),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationStatus {
    Pending,
    Accepted,
    Declined,
}

impl ApplicationStatus {
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

impl Need {
    pub fn scope(&self) -> Option<NeedScope> {
        NeedScope::parse(&self.scope)
    }

    pub fn status(&self) -> Option<NeedStatus> {
        NeedStatus::parse(&self.status)
    }

    pub fn is_open(&self) -> bool {
        self.status() == Some(NeedStatus::Open)
    }

    pub fn sight(&self) -> NeedSight<'_> {
        NeedSight {
            church_id: &self.church_id,
            author_id: &self.author_id,
            scope: self.scope(),
            status: &self.status,
        }
    }
}

/// Borrowed fields a visibility or apply rule actually reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NeedSight<'a> {
    pub church_id: &'a str,
    pub author_id: &'a str,
    pub scope: Option<NeedScope>,
    pub status: &'a str,
}

impl NeedSight<'_> {
    pub fn is_open(self) -> bool {
        NeedStatus::parse(self.status) == Some(NeedStatus::Open)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NeedCard {
    pub id: String,
    pub church_id: String,
    pub church_name: String,
    pub church_address: String,
    pub author_id: String,
    pub author_name: String,
    pub title: String,
    pub body: String,
    pub gift_id: Option<String>,
    pub gift_name: Option<String>,
    pub scope: String,
    pub status: String,
    pub created_at: String,
    pub praise: Option<String>,
    pub shelf: NeedShelf,
}

impl NeedCard {
    pub fn scope(&self) -> Option<NeedScope> {
        NeedScope::parse(&self.scope)
    }

    pub fn status(&self) -> Option<NeedStatus> {
        NeedStatus::parse(&self.status)
    }

    pub fn is_open(&self) -> bool {
        self.status() == Some(NeedStatus::Open)
    }

    pub fn sight(&self) -> NeedSight<'_> {
        NeedSight {
            church_id: &self.church_id,
            author_id: &self.author_id,
            scope: self.scope(),
            status: &self.status,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Application {
    pub id: String,
    pub need_id: String,
    pub user_id: String,
    pub message: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApplicationCard {
    pub id: String,
    pub need_id: String,
    pub user_id: String,
    pub user_name: String,
    pub message: String,
    pub status: String,
    pub created_at: String,
}

impl Application {
    pub fn status(&self) -> Option<ApplicationStatus> {
        ApplicationStatus::parse(&self.status)
    }
}

/// Whether a reply is an ordinary message or the author's completion.
///
/// # Notes
/// The store keeps [`ReplyKind::as_str`]: `message` or `completion`.
/// [`ReplyKind::parse`] is the only conversion. A row written before kinds
/// existed should be read as `message` when its column is missing.
///
/// # Examples
/// ```
/// use ecclesia_domain::ReplyKind;
///
/// assert_eq!(ReplyKind::parse("message"), Some(ReplyKind::Message));
/// assert_eq!(ReplyKind::Completion.as_str(), "completion");
/// assert_eq!(ReplyKind::parse("praise"), None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyKind {
    Message,
    Completion,
}

impl ReplyKind {
    /// Stored text for this kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Message => "message",
            Self::Completion => "completion",
        }
    }

    /// Reads stored text. Unknown text is not a kind.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "message" => Some(Self::Message),
            "completion" => Some(Self::Completion),
            _ => None,
        }
    }
}

/// A reply stored on a need.
///
/// `kind` is [`ReplyKind::Message`] for an ordinary reply and
/// [`ReplyKind::Completion`] for the author's closing reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NeedReply {
    pub id: String,
    pub need_id: String,
    pub author_id: String,
    pub kind: ReplyKind,
    pub body: String,
    pub created_at: String,
}

/// A reply plus the author's name, for a page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NeedReplyCard {
    pub id: String,
    pub need_id: String,
    pub author_id: String,
    pub author_name: String,
    /// The replier's active church, when they have one.
    pub author_church: Option<String>,
    pub kind: ReplyKind,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrayerStatus {
    Open,
    Answered,
}

impl PrayerStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Answered => "answered",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Prayer {
    pub id: String,
    pub church_id: String,
    pub author_id: Option<String>,
    pub body: String,
    pub status: String,
    pub praise: Option<String>,
    pub manage_hash: Option<String>,
    pub created_at: String,
    pub answered_at: Option<String>,
}

impl Prayer {
    pub fn is_open(&self) -> bool {
        self.status == PrayerStatus::Open.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrayerCard {
    pub id: String,
    pub church_id: String,
    pub church_name: String,
    pub author_id: Option<String>,
    pub author_name: Option<String>,
    pub author_avatar_id: Option<String>,
    pub body: String,
    pub status: String,
    pub praise: Option<String>,
    pub prayed_count: i64,
    pub created_at: String,
}

impl PrayerCard {
    pub fn is_open(&self) -> bool {
        self.status == PrayerStatus::Open.as_str()
    }
}
