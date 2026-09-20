use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub city: String,
    pub region: String,
    pub bio: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
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

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
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

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Gift {
    pub id: String,
    pub name: String,
    pub category: String,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
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

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
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
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
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

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Application {
    pub id: String,
    pub need_id: String,
    pub user_id: String,
    pub message: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
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

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Endorsement {
    pub id: String,
    pub from_user_id: String,
    pub to_user_id: String,
    pub gift_id: String,
    pub note: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
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

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
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

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ChurchMember {
    pub membership_id: String,
    pub user_id: String,
    pub name: String,
    pub city: String,
    pub role: String,
    pub status: String,
}

#[derive(Debug, Clone)]
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
}

pub fn churches_are_neighbors(a: &Church, b: &Church) -> bool {
    a.id != b.id
        && (a.city.eq_ignore_ascii_case(&b.city) || a.region.eq_ignore_ascii_case(&b.region))
}

pub fn can_view_need(viewer: &Viewer, need: &Need, church: &Church) -> bool {
    if viewer.user.id == need.author_id || viewer.can_govern(&need.church_id) {
        return true;
    }
    match need.scope() {
        Some(NeedScope::Body) => viewer.is_active_anywhere(),
        Some(NeedScope::Church) => viewer.is_active_in(&need.church_id),
        Some(NeedScope::Neighboring) => {
            viewer.is_active_in(&need.church_id)
                || viewer
                    .active_churches()
                    .any(|mine| churches_are_neighbors(mine, church))
        }
        None => false,
    }
}

pub fn can_apply(viewer: &Viewer, need: &Need, church: &Church) -> Result<(), DomainError> {
    if !need.is_open() {
        return Err(DomainError::NeedClosed);
    }
    if viewer.user.id == need.author_id {
        return Err(DomainError::OwnNeed);
    }
    if !can_view_need(viewer, need, church) {
        return match need.scope() {
            Some(NeedScope::Church) => Err(DomainError::OutsideChurch),
            Some(NeedScope::Neighboring) => Err(DomainError::OutsideNeighborhood),
            _ => Err(DomainError::NotInTheBody),
        };
    }
    if !viewer.is_active_anywhere() {
        return Err(DomainError::NotInTheBody);
    }
    Ok(())
}

pub fn can_endorse(from_id: &str, to_id: &str) -> Result<(), DomainError> {
    if from_id == to_id {
        Err(DomainError::SelfAction)
    } else {
        Ok(())
    }
}

pub fn can_decide_membership(actor: &Membership) -> Result<(), DomainError> {
    if actor.can_govern() {
        Ok(())
    } else {
        Err(DomainError::NotGovernor)
    }
}

pub fn next_membership_after_decision(
    current: MembershipStatus,
    approve: bool,
) -> Result<MembershipStatus, DomainError> {
    match (current, approve) {
        (MembershipStatus::PendingRequest | MembershipStatus::PendingInvite, true) => {
            Ok(MembershipStatus::Active)
        }
        (MembershipStatus::PendingRequest | MembershipStatus::PendingInvite, false) => {
            Ok(MembershipStatus::Declined)
        }
        _ => Err(DomainError::NothingPending),
    }
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn invite_code_for(name: &str) -> String {
    let slug: String = name
        .chars()
        .flat_map(|c| c.to_lowercase())
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect();
    let suffix = &uuid::Uuid::new_v4().simple().to_string()[..4];
    format!("{slug}-{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(id: &str, city: &str, region: &str) -> User {
        User {
            id: id.into(),
            name: id.into(),
            email: format!("{id}@ecclesia.test"),
            city: city.into(),
            region: region.into(),
            bio: String::new(),
            created_at: now_iso(),
        }
    }

    fn church(id: &str, city: &str, region: &str) -> Church {
        Church {
            id: id.into(),
            name: id.into(),
            city: city.into(),
            region: region.into(),
            country: "US".into(),
            description: String::new(),
            gathering: String::new(),
            owner_id: "owner".into(),
            invite_code: "code".into(),
            created_at: now_iso(),
        }
    }

    fn membership(church_id: &str, user_id: &str, status: &str) -> Membership {
        Membership {
            id: format!("{church_id}-{user_id}"),
            church_id: church_id.into(),
            user_id: user_id.into(),
            role: "member".into(),
            status: status.into(),
            created_at: now_iso(),
        }
    }

    fn viewer(user: User, churches: Vec<Church>, memberships: Vec<Membership>) -> Viewer {
        Viewer {
            user,
            memberships,
            churches,
            gift_ids: vec![],
        }
    }

    fn need(scope: NeedScope, church_id: &str, author_id: &str) -> Need {
        Need {
            id: "need".into(),
            church_id: church_id.into(),
            author_id: author_id.into(),
            title: "Help".into(),
            body: "Please".into(),
            gift_id: None,
            scope: scope.as_str().into(),
            status: "open".into(),
            created_at: now_iso(),
        }
    }

    #[test]
    fn neighbors_share_city_or_region() {
        let grace = church("grace", "Cedar Falls", "Iowa");
        let luke = church("luke", "Cedar Falls", "Iowa");
        let mercy = church("mercy", "Waterloo", "Iowa");
        let far = church("far", "Austin", "Texas");
        assert!(churches_are_neighbors(&grace, &luke));
        assert!(churches_are_neighbors(&grace, &mercy));
        assert!(!churches_are_neighbors(&grace, &far));
        assert!(!churches_are_neighbors(&grace, &grace));
    }

    #[test]
    fn church_scope_hides_need_from_neighbors() {
        let grace = church("grace", "Cedar Falls", "Iowa");
        let luke = church("luke", "Cedar Falls", "Iowa");
        let james = viewer(
            user("james", "Cedar Falls", "Iowa"),
            vec![luke],
            vec![membership("luke", "james", "active")],
        );
        let n = need(NeedScope::Church, "grace", "miriam");
        assert!(!can_view_need(&james, &n, &grace));
        assert_eq!(
            can_apply(&james, &n, &grace),
            Err(DomainError::OutsideChurch)
        );
    }

    #[test]
    fn neighboring_scope_opens_need_to_same_region() {
        let grace = church("grace", "Cedar Falls", "Iowa");
        let mercy = church("mercy", "Waterloo", "Iowa");
        let elena = viewer(
            user("elena", "Waterloo", "Iowa"),
            vec![mercy],
            vec![membership("mercy", "elena", "active")],
        );
        let n = need(NeedScope::Neighboring, "grace", "miriam");
        assert!(can_view_need(&elena, &n, &grace));
        assert_eq!(can_apply(&elena, &n, &grace), Ok(()));
    }

    #[test]
    fn pending_member_is_not_yet_in_the_body() {
        let grace = church("grace", "Cedar Falls", "Iowa");
        let peter = viewer(
            user("peter", "Cedar Falls", "Iowa"),
            vec![grace.clone()],
            vec![membership("grace", "peter", "pending_request")],
        );
        let n = need(NeedScope::Body, "grace", "miriam");
        assert!(!can_view_need(&peter, &n, &grace));
        assert_eq!(
            can_apply(&peter, &n, &grace),
            Err(DomainError::NotInTheBody)
        );
    }

    #[test]
    fn author_cannot_apply_to_own_need() {
        let grace = church("grace", "Cedar Falls", "Iowa");
        let miriam = viewer(
            user("miriam", "Cedar Falls", "Iowa"),
            vec![grace.clone()],
            vec![membership("grace", "miriam", "active")],
        );
        let n = need(NeedScope::Church, "grace", "miriam");
        assert_eq!(can_apply(&miriam, &n, &grace), Err(DomainError::OwnNeed));
    }

    #[test]
    fn endorsement_cannot_target_self() {
        assert_eq!(can_endorse("a", "a"), Err(DomainError::SelfAction));
        assert_eq!(can_endorse("a", "b"), Ok(()));
    }

    #[test]
    fn only_pending_memberships_can_be_decided() {
        assert_eq!(
            next_membership_after_decision(MembershipStatus::PendingRequest, true),
            Ok(MembershipStatus::Active)
        );
        assert_eq!(
            next_membership_after_decision(MembershipStatus::Active, true),
            Err(DomainError::NothingPending)
        );
    }
}
