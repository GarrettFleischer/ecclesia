//! sqlx row shapes. Domain types cannot derive FromRow.

use ecclesia_domain::{
    Application, Church, ChurchMember, Endorsement, EndorsementCard, Gift, MemberGift, Need,
    NeedCard, NeedReply, NeedReplyCard, NeedShelf, Notification, Prayer, PrayerCard, ReplyKind,
    Share, User, display_name,
};

/// Stored reply kind text that is not `message` or `completion`.
///
/// # Notes
/// A missing column or SQL `NULL` is not this error. Those are a legacy row
/// and read as [`ReplyKind::Message`]. A nonempty unknown value is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownReplyKind {
    pub value: String,
}

impl std::fmt::Display for UnknownReplyKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "unknown reply kind")
    }
}

impl std::error::Error for UnknownReplyKind {}

/// Reads `need_replies.kind`.
///
/// # Parameters
/// - `value`: the column text. `None` means the column was missing or `NULL`.
///
/// # Returns
/// [`ReplyKind::Message`] for a missing value, the parsed kind, or [`UnknownReplyKind`].
///
/// # Examples
/// ```
/// use ecclesia_domain::ReplyKind;
/// use ecclesia_sdk::db::reply_kind_from_column;
///
/// assert_eq!(reply_kind_from_column(None).unwrap(), ReplyKind::Message);
/// assert_eq!(
///     reply_kind_from_column(Some("completion")).unwrap(),
///     ReplyKind::Completion
/// );
/// assert!(reply_kind_from_column(Some("note")).is_err());
/// assert!(reply_kind_from_column(Some("")).is_err());
/// ```
pub fn reply_kind_from_column(value: Option<&str>) -> Result<ReplyKind, UnknownReplyKind> {
    let Some(text) = value else {
        return Ok(ReplyKind::Message);
    };
    ReplyKind::parse(text).ok_or_else(|| UnknownReplyKind {
        value: text.to_string(),
    })
}

#[derive(sqlx::FromRow)]
pub struct UserRow {
    pub id: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub bio: String,
    pub created_at: String,
    #[sqlx(default)]
    pub password_hash: Option<String>,
}

#[derive(sqlx::FromRow)]
pub struct MembershipRow {
    pub church_id: String,
    pub role: String,
    pub status: String,
}

#[derive(sqlx::FromRow)]
pub struct GiftRow {
    pub id: String,
    pub name: String,
    pub category: String,
}

#[derive(sqlx::FromRow)]
pub struct MemberGiftRow {
    pub user_id: String,
    pub gift_id: String,
    pub note: String,
    pub gift_name: String,
    pub category: String,
}

#[derive(sqlx::FromRow)]
pub struct EndorsementRow {
    pub id: String,
    pub from_user_id: String,
    pub to_user_id: String,
    pub gift_id: String,
    pub skill: String,
    pub note: String,
    pub status: String,
    pub created_at: String,
}

#[derive(sqlx::FromRow)]
pub struct EndorsementCardRow {
    pub id: String,
    pub from_user_id: String,
    pub from_first: String,
    pub from_last: String,
    pub to_user_id: String,
    pub to_first: String,
    pub to_last: String,
    pub gift_id: String,
    pub gift_name: String,
    pub note: String,
    pub status: String,
    pub created_at: String,
}

#[derive(sqlx::FromRow)]
pub struct NotificationRow {
    pub id: String,
    pub user_id: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub href: String,
    pub read: i64,
    pub created_at: String,
}

#[derive(sqlx::FromRow)]
pub struct ChurchMemberRow {
    pub user_id: String,
    pub first_name: String,
    pub last_name: String,
    pub role: String,
    pub status: String,
}

#[derive(sqlx::FromRow)]
pub struct CountRow {
    pub id: String,
    pub members: i64,
    pub needs: i64,
}

#[derive(sqlx::FromRow)]
pub struct ChurchRow {
    pub id: String,
    pub name: String,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub country: String,
    pub description: String,
    pub gathering: String,
    pub ein: String,
    pub registry_state: String,
    pub registry_number: String,
    pub owner_id: String,
    pub invite_code: String,
    pub created_at: String,
}

#[derive(sqlx::FromRow)]
pub struct NeedRow {
    pub id: String,
    pub church_id: String,
    pub author_id: String,
    pub title: String,
    pub body: String,
    pub gift_id: Option<String>,
    pub scope: String,
    pub status: String,
    pub created_at: String,
    #[sqlx(default)]
    pub closed_at: Option<String>,
    #[sqlx(default)]
    pub praise: Option<String>,
    #[sqlx(default)]
    pub archived: i64,
}

#[derive(sqlx::FromRow)]
pub struct NeedCardRow {
    pub id: String,
    pub church_id: String,
    pub church_name: String,
    pub church_address: String,
    pub author_id: String,
    pub author_first: String,
    pub author_last: String,
    pub title: String,
    pub body: String,
    pub gift_id: Option<String>,
    pub gift_name: Option<String>,
    pub scope: String,
    pub status: String,
    pub created_at: String,
    #[sqlx(default)]
    pub praise: Option<String>,
    #[sqlx(default)]
    pub archived: i64,
}

#[derive(sqlx::FromRow)]
pub struct ApplicationRow {
    pub id: String,
    pub need_id: String,
    pub user_id: String,
    pub message: String,
    pub status: String,
    pub created_at: String,
}

pub fn map_all<R, T>(rows: Vec<R>) -> Vec<T>
where
    T: From<R>,
{
    rows.into_iter().map(T::from).collect()
}

/// Maps reply rows, refusing an unknown nonempty kind.
///
/// # Returns
/// Cards in the same order, or the first [`UnknownReplyKind`].
pub fn map_reply_cards(
    rows: Vec<NeedReplyCardRow>,
) -> Result<Vec<NeedReplyCard>, UnknownReplyKind> {
    let mut cards = Vec::with_capacity(rows.len());
    for row in rows {
        cards.push(reply_card_from_row(row)?);
    }
    Ok(cards)
}

fn reply_card_from_row(row: NeedReplyCardRow) -> Result<NeedReplyCard, UnknownReplyKind> {
    Ok(NeedReplyCard {
        id: row.id,
        need_id: row.need_id,
        author_id: row.author_id,
        author_name: display_name(&row.author_first, &row.author_last),
        kind: reply_kind_from_column(row.kind.as_deref())?,
        body: row.body,
        created_at: row.created_at,
    })
}

pub(crate) fn need_reply_from_row(row: NeedReplyRow) -> Result<NeedReply, UnknownReplyKind> {
    Ok(NeedReply {
        id: row.id,
        need_id: row.need_id,
        author_id: row.author_id,
        kind: reply_kind_from_column(row.kind.as_deref())?,
        body: row.body,
        created_at: row.created_at,
    })
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            first_name: row.first_name,
            last_name: row.last_name,
            email: row.email,
            bio: row.bio,
            created_at: row.created_at,
            memberships: Vec::new(),
        }
    }
}

impl From<GiftRow> for Gift {
    fn from(row: GiftRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            category: row.category,
        }
    }
}

impl From<MemberGiftRow> for MemberGift {
    fn from(row: MemberGiftRow) -> Self {
        Self {
            user_id: row.user_id,
            gift_id: row.gift_id,
            note: row.note,
            gift_name: row.gift_name,
            category: row.category,
        }
    }
}

impl From<EndorsementRow> for Endorsement {
    fn from(row: EndorsementRow) -> Self {
        Self {
            id: row.id,
            from_user_id: row.from_user_id,
            to_user_id: row.to_user_id,
            gift_id: row.gift_id,
            skill: row.skill,
            note: row.note,
            status: row.status,
            created_at: row.created_at,
        }
    }
}

impl From<EndorsementCardRow> for EndorsementCard {
    fn from(row: EndorsementCardRow) -> Self {
        Self {
            id: row.id,
            from_user_id: row.from_user_id,
            from_user_name: display_name(&row.from_first, &row.from_last),
            to_user_id: row.to_user_id,
            to_user_name: display_name(&row.to_first, &row.to_last),
            gift_id: row.gift_id,
            gift_name: row.gift_name,
            note: row.note,
            status: row.status,
            created_at: row.created_at,
        }
    }
}

impl From<NotificationRow> for Notification {
    fn from(row: NotificationRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            kind: row.kind,
            title: row.title,
            body: row.body,
            href: row.href,
            read: row.read,
            created_at: row.created_at,
        }
    }
}

impl From<ChurchMemberRow> for ChurchMember {
    fn from(row: ChurchMemberRow) -> Self {
        Self {
            user_id: row.user_id,
            first_name: row.first_name,
            last_name: row.last_name,
            role: row.role,
            status: row.status,
        }
    }
}

impl From<ChurchRow> for Church {
    fn from(row: ChurchRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            address: row.address,
            latitude: row.latitude,
            longitude: row.longitude,
            country: row.country,
            description: row.description,
            gathering: row.gathering,
            ein: row.ein,
            registry_state: row.registry_state,
            registry_number: row.registry_number,
            owner_id: row.owner_id,
            invite_code: row.invite_code,
            created_at: row.created_at,
        }
    }
}

impl From<NeedRow> for Need {
    fn from(row: NeedRow) -> Self {
        Self {
            id: row.id,
            church_id: row.church_id,
            author_id: row.author_id,
            title: row.title,
            body: row.body,
            gift_id: row.gift_id,
            scope: row.scope,
            status: row.status,
            created_at: row.created_at,
            closed_at: row.closed_at,
            praise: row.praise,
            shelf: NeedShelf::from_flag(row.archived),
        }
    }
}

impl From<NeedCardRow> for NeedCard {
    fn from(row: NeedCardRow) -> Self {
        Self {
            id: row.id,
            church_id: row.church_id,
            church_name: row.church_name,
            church_address: row.church_address,
            author_id: row.author_id,
            author_name: display_name(&row.author_first, &row.author_last),
            title: row.title,
            body: row.body,
            gift_id: row.gift_id,
            gift_name: row.gift_name,
            scope: row.scope,
            status: row.status,
            created_at: row.created_at,
            praise: row.praise,
            shelf: NeedShelf::from_flag(row.archived),
        }
    }
}

impl From<ApplicationRow> for Application {
    fn from(row: ApplicationRow) -> Self {
        Self {
            id: row.id,
            need_id: row.need_id,
            user_id: row.user_id,
            message: row.message,
            status: row.status,
            created_at: row.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
pub struct NeedReplyCardRow {
    pub id: String,
    pub need_id: String,
    pub author_id: String,
    pub author_first: String,
    pub author_last: String,
    /// Missing on a row written before `kind` existed. `NULL` uses the same path.
    #[sqlx(default)]
    pub kind: Option<String>,
    pub body: String,
    pub created_at: String,
}

#[derive(sqlx::FromRow)]
pub struct NeedReplyRow {
    pub id: String,
    pub need_id: String,
    pub author_id: String,
    pub body: String,
    pub created_at: String,
    /// Missing on a row written before `kind` existed. `NULL` uses the same path.
    #[sqlx(default)]
    pub kind: Option<String>,
}

/// One `need_media` or `reply_media` row, in display order.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct AttachmentRow {
    pub media_id: String,
    pub position: i64,
    pub description: Option<String>,
}

/// A single nullable text column selected as `value`.
#[derive(sqlx::FromRow)]
pub(crate) struct OptionalTextRow {
    pub value: Option<String>,
}

#[derive(sqlx::FromRow)]
pub struct PrayerRow {
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

#[derive(sqlx::FromRow)]
pub struct PrayerCardRow {
    pub id: String,
    pub church_id: String,
    pub church_name: String,
    pub author_id: Option<String>,
    pub author_first: Option<String>,
    pub author_last: Option<String>,
    pub author_avatar: Option<String>,
    pub body: String,
    pub status: String,
    pub praise: Option<String>,
    pub prayed_count: i64,
    pub created_at: String,
}

#[derive(sqlx::FromRow)]
pub struct PrayerLocatedRow {
    pub id: String,
    pub church_id: String,
    pub author_id: Option<String>,
    pub body: String,
    pub status: String,
    pub praise: Option<String>,
    pub manage_hash: Option<String>,
    pub created_at: String,
    pub answered_at: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(sqlx::FromRow)]
pub struct MarkRow {
    pub prayer_id: String,
    pub kind: String,
}

impl From<PrayerRow> for Prayer {
    fn from(row: PrayerRow) -> Self {
        Self {
            id: row.id,
            church_id: row.church_id,
            author_id: row.author_id,
            body: row.body,
            status: row.status,
            praise: row.praise,
            manage_hash: row.manage_hash,
            created_at: row.created_at,
            answered_at: row.answered_at,
        }
    }
}

impl From<PrayerCardRow> for PrayerCard {
    fn from(row: PrayerCardRow) -> Self {
        let author_name = match (row.author_first, row.author_last) {
            (Some(first), Some(last)) => Some(display_name(&first, &last)),
            _ => None,
        };
        Self {
            id: row.id,
            church_id: row.church_id,
            church_name: row.church_name,
            author_id: row.author_id,
            author_name,
            author_avatar_id: row.author_avatar,
            body: row.body,
            status: row.status,
            praise: row.praise,
            prayed_count: row.prayed_count,
            created_at: row.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
pub struct ShareRow {
    pub code: String,
    pub kind: String,
    pub target_id: String,
    pub expires_at: Option<String>,
}

impl From<ShareRow> for Share {
    fn from(row: ShareRow) -> Self {
        Self {
            code: row.code,
            expires_at: row.expires_at,
            kind: row.kind,
            target_id: row.target_id,
        }
    }
}

impl From<PrayerLocatedRow> for (Prayer, f64, f64) {
    fn from(row: PrayerLocatedRow) -> Self {
        let latitude = row.latitude;
        let longitude = row.longitude;
        (
            Prayer {
                id: row.id,
                church_id: row.church_id,
                author_id: row.author_id,
                body: row.body,
                status: row.status,
                praise: row.praise,
                manage_hash: row.manage_hash,
                created_at: row.created_at,
                answered_at: row.answered_at,
            },
            latitude,
            longitude,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(kind: Option<&str>) -> NeedReplyCardRow {
        NeedReplyCardRow {
            id: "r".into(),
            need_id: "n".into(),
            author_id: "ada".into(),
            author_first: "Ada".into(),
            author_last: "Lane".into(),
            kind: kind.map(str::to_string),
            body: "Saturday.".into(),
            created_at: "t".into(),
        }
    }

    #[test]
    fn missing_kind_is_a_message_and_unknown_kind_is_refused() {
        assert_eq!(reply_kind_from_column(None).unwrap(), ReplyKind::Message);
        assert_eq!(
            reply_kind_from_column(Some("message")).unwrap(),
            ReplyKind::Message
        );
        assert_eq!(
            reply_kind_from_column(Some("completion")).unwrap(),
            ReplyKind::Completion
        );
        let unknown = reply_kind_from_column(Some("note")).unwrap_err();
        assert_eq!(unknown.value, "note");
        assert!(reply_kind_from_column(Some("")).is_err());
        let cards = map_reply_cards(vec![card(None), card(Some("completion"))]).unwrap();
        assert_eq!(cards[0].kind, ReplyKind::Message);
        assert_eq!(cards[1].kind, ReplyKind::Completion);
        assert!(map_reply_cards(vec![card(Some("praise"))]).is_err());
    }
}
