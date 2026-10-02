//! sqlx row shapes. Domain types cannot derive FromRow.

use ecclesia_domain::{
    Application, ApplicationCard, Church, ChurchMember, Endorsement, EndorsementCard, Gift,
    MemberGift, Need, NeedCard, Notification, User, display_name,
};

#[derive(sqlx::FromRow)]
pub struct UserRow {
    pub id: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub bio: String,
    pub created_at: String,
    pub church_id: Option<String>,
    pub church_status: Option<String>,
    pub church_role: Option<String>,
    #[sqlx(default)]
    pub password_hash: Option<String>,
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

#[derive(sqlx::FromRow)]
pub struct ApplicationCardRow {
    pub id: String,
    pub need_id: String,
    pub user_id: String,
    pub user_first: String,
    pub user_last: String,
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

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            first_name: row.first_name,
            last_name: row.last_name,
            email: row.email,
            bio: row.bio,
            created_at: row.created_at,
            church_id: row.church_id,
            church_status: row.church_status,
            church_role: row.church_role,
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

impl From<ApplicationCardRow> for ApplicationCard {
    fn from(row: ApplicationCardRow) -> Self {
        Self {
            id: row.id,
            need_id: row.need_id,
            user_id: row.user_id,
            user_name: display_name(&row.user_first, &row.user_last),
            message: row.message,
            status: row.status,
            created_at: row.created_at,
        }
    }
}
