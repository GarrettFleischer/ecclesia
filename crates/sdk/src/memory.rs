use ecclesia_domain::{
    Application, Attachment, Church, Effect, Endorsement, Membership, Need, NeedReply, NeedShelf,
    Notification, Prayer, Share, User, Write,
};

/// A photo link stored on a need or a reply.
///
/// # Notes
/// `owner_id` is the need id or the reply id, depending on which list holds the link.
/// Detach removes this link. It does not remove a [`MemoryWorld::media_assets`] id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryAttachment {
    pub owner_id: String,
    pub media_id: String,
    pub position: i64,
    pub description: Option<String>,
}

/// In-process world. The SDK applies Domain effects here so stories can be
/// confirmed without SQLite or HTTP.
#[derive(Debug, Default, Clone)]
pub struct MemoryWorld {
    pub users: Vec<User>,
    pub churches: Vec<Church>,
    pub closed_churches: Vec<(Church, String)>,
    pub member_gifts: Vec<(String, String, String)>,
    pub removed_gifts: Vec<(String, String, String)>,
    pub needs: Vec<Need>,
    pub shares: Vec<Share>,
    pub applications: Vec<Application>,
    pub need_replies: Vec<NeedReply>,
    /// `user_id`, `media_id`. No entry means the profile shows initials.
    pub avatars: Vec<(String, String)>,
    /// `need_id`, `reply_id` for the reply that closed the need.
    pub closing_replies: Vec<(String, String)>,
    pub need_media: Vec<MemoryAttachment>,
    pub reply_media: Vec<MemoryAttachment>,
    /// Asset ids already stored. Attachment writes do not add or remove these.
    pub media_assets: Vec<String>,
    pub prayers: Vec<Prayer>,
    pub prayer_marks: Vec<(String, String, String, String)>,
    pub endorsements: Vec<Endorsement>,
    pub notifications: Vec<Notification>,
}

impl MemoryWorld {
    pub fn apply(&mut self, effect: Effect, now: &str) {
        apply_writes(self, effect.writes);
        apply_notices(self, effect.notices, now);
    }

    pub fn user(&self, id: &str) -> Option<&User> {
        self.users.iter().find(|user| user.id == id)
    }

    pub fn gifts_for<'a>(
        &'a self,
        user_id: &'a str,
    ) -> impl Iterator<Item = &'a (String, String, String)> + 'a {
        self.member_gifts
            .iter()
            .filter(move |(id, _, _)| id == user_id)
    }
}

fn apply_writes(world: &mut MemoryWorld, writes: Vec<Write>) {
    for write in writes {
        apply_write(world, write);
    }
}

fn apply_write(world: &mut MemoryWorld, write: Write) {
    match write {
        Write::InsertUser(user) => world.users.push(user),
        Write::UpdateUser {
            id,
            first_name,
            last_name,
            bio,
        } => update_user(world, id, first_name, last_name, bio),
        Write::SetAvatar { user_id, media_id } => set_avatar(world, user_id, media_id),
        Write::ClearAvatar { user_id } => clear_avatar(world, user_id),
        Write::InsertChurch(church) => world.churches.push(church),
        Write::UpsertMembership {
            user_id,
            church_id,
            status,
            role,
        } => upsert_membership(world, user_id, church_id, status, role),
        Write::DeleteMembership { user_id, church_id } => {
            delete_membership(world, user_id, church_id)
        }
        Write::SetChurchOwner {
            church_id,
            owner_id,
        } => {
            if let Some(church) = world
                .churches
                .iter_mut()
                .find(|church| church.id == church_id)
            {
                church.owner_id = owner_id;
            }
        }
        Write::CloseChurch { id, deleted_at } => close_church(world, id, deleted_at),
        Write::InsertNeed(need) => world.needs.push(need),
        Write::InsertShare(share) => world.shares.push(share),
        Write::DeleteNeedShare { target_id } => delete_need_share(world, &target_id),
        Write::SetNeedStatus {
            id,
            status,
            closed_at,
            praise,
        } => set_need_status(world, id, status, closed_at, praise),
        Write::SetClosingReply { need_id, reply_id } => set_closing_reply(world, need_id, reply_id),
        Write::AttachNeedMedia {
            need_id,
            attachments,
        } => push_attachments(&mut world.need_media, need_id, attachments),
        Write::DetachNeedMedia { need_id, media_id } => {
            detach_attachment(&mut world.need_media, &need_id, &media_id)
        }
        Write::MoveNeed { id, church_id } => move_need(world, id, church_id),
        Write::InsertApplication(application) => world.applications.push(application),
        Write::SetApplicationStatus { id, status } => set_application_status(world, id, status),
        Write::InsertEndorsement(endorsement) => world.endorsements.push(endorsement),
        Write::SetEndorsementStatus { id, status } => set_endorsement_status(world, id, status),
        Write::UpsertMemberGift {
            user_id,
            gift_id,
            note,
        } => upsert_member_gift(world, user_id, gift_id, note),
        Write::RemoveMemberGift { user_id, gift_id } => remove_member_gift(world, user_id, gift_id),
        Write::InsertNeedReply(reply) => world.need_replies.push(reply),
        Write::AttachReplyMedia {
            reply_id,
            attachments,
        } => push_attachments(&mut world.reply_media, reply_id, attachments),
        Write::DetachReplyMedia { reply_id, media_id } => {
            detach_attachment(&mut world.reply_media, &reply_id, &media_id)
        }
        Write::InsertPrayer(prayer) => world.prayers.push(prayer),
        Write::SetPrayerAnswered {
            id,
            praise,
            answered_at,
        } => answer_prayer(world, id, praise, answered_at),
        Write::UpsertPrayerMark {
            user_id,
            prayer_id,
            day,
            kind,
        } => upsert_prayer_mark(world, user_id, prayer_id, day, kind),
    }
}

fn update_user(
    world: &mut MemoryWorld,
    id: String,
    first_name: String,
    last_name: String,
    bio: String,
) {
    if let Some(user) = world.users.iter_mut().find(|user| user.id == id) {
        user.first_name = first_name;
        user.last_name = last_name;
        user.bio = bio;
    }
}

fn close_church(world: &mut MemoryWorld, id: String, deleted_at: String) {
    let Some(index) = world.churches.iter().position(|church| church.id == id) else {
        return;
    };
    let church = world.churches.remove(index);
    world.closed_churches.push((church, deleted_at));
}

fn upsert_membership(
    world: &mut MemoryWorld,
    user_id: String,
    church_id: String,
    status: String,
    role: String,
) {
    let Some(user) = world.users.iter_mut().find(|user| user.id == user_id) else {
        return;
    };
    if let Some(link) = user
        .memberships
        .iter_mut()
        .find(|link| link.church_id == church_id)
    {
        link.status = status;
        link.role = role;
        return;
    }
    user.memberships.push(Membership {
        church_id,
        status,
        role,
    });
}

fn delete_membership(world: &mut MemoryWorld, user_id: String, church_id: String) {
    let Some(user) = world.users.iter_mut().find(|user| user.id == user_id) else {
        return;
    };
    user.memberships.retain(|link| link.church_id != church_id);
}

fn set_avatar(world: &mut MemoryWorld, user_id: String, media_id: String) {
    if let Some(avatar) = world.avatars.iter_mut().find(|(id, _)| id == &user_id) {
        avatar.1 = media_id;
        return;
    }
    world.avatars.push((user_id, media_id));
}

fn clear_avatar(world: &mut MemoryWorld, user_id: String) {
    world.avatars.retain(|(id, _)| id != &user_id);
}

fn set_closing_reply(world: &mut MemoryWorld, need_id: String, reply_id: Option<String>) {
    world.closing_replies.retain(|(id, _)| id != &need_id);
    if let Some(reply_id) = reply_id {
        world.closing_replies.push((need_id, reply_id));
    }
}

fn push_attachments(
    links: &mut Vec<MemoryAttachment>,
    owner_id: String,
    attachments: Vec<Attachment>,
) {
    for attachment in attachments {
        links.push(MemoryAttachment {
            owner_id: owner_id.clone(),
            media_id: attachment.media_id,
            position: attachment.position,
            description: attachment.description,
        });
    }
}

fn detach_attachment(links: &mut Vec<MemoryAttachment>, owner_id: &str, media_id: &str) {
    let Some(index) = links
        .iter()
        .position(|link| link.owner_id == owner_id && link.media_id == media_id)
    else {
        return;
    };
    links.remove(index);
}

fn set_need_status(
    world: &mut MemoryWorld,
    id: String,
    status: &'static str,
    closed_at: Option<String>,
    praise: Option<String>,
) {
    if let Some(need) = world.needs.iter_mut().find(|need| need.id == id) {
        need.status = status.into();
        need.closed_at = closed_at;
        need.praise = praise;
    }
}

fn move_need(world: &mut MemoryWorld, id: String, church_id: String) {
    let Some(need) = world.needs.iter_mut().find(|need| need.id == id) else {
        return;
    };
    if need.status == "open" && need.shelf == NeedShelf::Listed {
        need.church_id = church_id;
    }
}

fn delete_need_share(world: &mut MemoryWorld, target_id: &str) {
    world
        .shares
        .retain(|share| share.kind != "need" || share.target_id != target_id);
}

fn set_application_status(world: &mut MemoryWorld, id: String, status: &'static str) {
    if let Some(application) = world.applications.iter_mut().find(|a| a.id == id) {
        application.status = status.into();
    }
}

fn set_endorsement_status(world: &mut MemoryWorld, id: String, status: &'static str) {
    if let Some(endorsement) = world.endorsements.iter_mut().find(|e| e.id == id) {
        endorsement.status = status.into();
    }
}

fn upsert_member_gift(world: &mut MemoryWorld, user_id: String, gift_id: String, note: String) {
    if let Some(existing) = world
        .member_gifts
        .iter_mut()
        .find(|(u, g, _)| *u == user_id && *g == gift_id)
    {
        existing.2 = note;
        return;
    }
    if let Some(index) = world
        .removed_gifts
        .iter()
        .position(|(user, gift, _)| user == &user_id && gift == &gift_id)
    {
        let mut gift = world.removed_gifts.remove(index);
        gift.2 = note;
        world.member_gifts.push(gift);
        return;
    }
    world.member_gifts.push((user_id, gift_id, note));
}

fn remove_member_gift(world: &mut MemoryWorld, user_id: String, gift_id: String) {
    let Some(index) = world
        .member_gifts
        .iter()
        .position(|(user, gift, _)| user == &user_id && gift == &gift_id)
    else {
        return;
    };
    let gift = world.member_gifts.remove(index);
    world.removed_gifts.push(gift);
}

fn answer_prayer(world: &mut MemoryWorld, id: String, praise: String, answered_at: String) {
    let Some(prayer) = world.prayers.iter_mut().find(|prayer| prayer.id == id) else {
        return;
    };
    prayer.status = "answered".into();
    prayer.praise = Some(praise);
    prayer.answered_at = Some(answered_at);
}

fn upsert_prayer_mark(
    world: &mut MemoryWorld,
    user_id: String,
    prayer_id: String,
    day: String,
    kind: &str,
) {
    if let Some(mark) = world
        .prayer_marks
        .iter_mut()
        .find(|mark| mark.0 == user_id && mark.1 == prayer_id && mark.2 == day)
    {
        mark.3 = kind.to_string();
        return;
    }
    world
        .prayer_marks
        .push((user_id, prayer_id, day, kind.to_string()));
}

fn apply_notices(world: &mut MemoryWorld, notices: Vec<ecclesia_domain::NoticeDraft>, now: &str) {
    for notice in notices {
        push_notice(world, notice, now);
    }
}

fn push_notice(world: &mut MemoryWorld, notice: ecclesia_domain::NoticeDraft, now: &str) {
    world.notifications.push(Notification {
        id: format!("n-{}", world.notifications.len()),
        user_id: notice.user_id,
        kind: notice.kind.into(),
        title: notice.title.to_string(),
        body: notice.body.into(),
        href: notice.href,
        read: 0,
        created_at: now.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecclesia_domain::sample::{church, user_in_church, user_named, viewer_of};
    use ecclesia_domain::{
        AttachmentRef, CatalogPresence, Posture, PriorOffer, ReplyKind, Viewer, accept_endorsement,
        accept_invite, apply_to_need, approve_membership, complete_need,
        post_need_with_attachments, reopen_need, replace_with_pending,
    };

    fn user(id: &str, first: &str) -> User {
        user_named(id, first, "Lane")
    }

    #[test]
    fn us_mem_01_sdk_applies_a_join_request() {
        let mut world = MemoryWorld::default();
        let peter = user("peter", "Peter");
        world.users.push(peter.clone());
        let grace = church("grace");
        let effect = replace_with_pending(&peter, &grace, &["miriam".into()]).unwrap();
        world.apply(effect, "t1");
        let saved = world.user("peter").unwrap();
        assert_eq!(
            saved
                .membership_in("grace")
                .map(|link| link.status.as_str()),
            Some("pending")
        );
        assert_eq!(world.notifications[0].user_id, "miriam");
    }

    #[test]
    fn us_mem_04_sdk_accept_invite_round_trip() {
        let mut world = MemoryWorld::default();
        let peter = user_in_church("peter", "grace", "member", "invited");
        world.users.push(peter.clone());
        let effect = accept_invite(&peter, "grace").unwrap();
        world.apply(effect, "t1");
        assert_eq!(
            world
                .user("peter")
                .unwrap()
                .membership_in("grace")
                .map(|link| link.status.as_str()),
            Some("active")
        );
    }

    #[test]
    fn us_end_02_sdk_accept_writes_gift_and_notice() {
        let mut world = MemoryWorld::default();
        world.endorsements.push(Endorsement {
            id: "e1".into(),
            from_user_id: "james".into(),
            to_user_id: "ruth".into(),
            gift_id: "gift_hospitality".into(),
            skill: "Hospitality".into(),
            note: "She stayed.".into(),
            status: "pending".into(),
            created_at: "t0".into(),
        });
        let ruth = user("ruth", "Ruth");
        let effect = accept_endorsement(
            &ruth,
            &world.endorsements[0],
            ecclesia_domain::GiftOnProfile::Absent,
        )
        .unwrap();
        world.apply(effect, "t1");
        assert_eq!(world.endorsements[0].status, "accepted");
        assert_eq!(world.gifts_for("ruth").count(), 1);
        assert_eq!(world.notifications[0].user_id, "james");
    }

    #[test]
    fn us_mem_04_sdk_member_cannot_approve() {
        let viewer = Viewer {
            user: user_in_church("ruth", "grace", "member", "active"),
            churches: vec![church("grace")],
            gift_ids: vec![],
        };
        let target = user_in_church("peter", "grace", "member", "pending");
        assert!(approve_membership(&viewer, &target, &church("grace")).is_err());
    }

    fn author() -> Viewer {
        viewer_of(
            user_in_church("ada", "grace", "member", "active"),
            Some(church("grace")),
        )
    }

    #[test]
    fn completion_applies_reply_media_status_and_closing_reply_in_order() {
        let viewer = author();
        let mut world = MemoryWorld::default();
        world.media_assets.push("m1".into());
        let posted = post_need_with_attachments(
            &viewer,
            "grace",
            "Roof",
            "It leaked.",
            None,
            CatalogPresence::Listed,
            "church",
            Posture::Lifts,
            &[AttachmentRef {
                media_id: "m1",
                description: Some("west slope"),
            }],
            "need-1".into(),
            "t0".into(),
        )
        .unwrap();
        world.apply(posted, "t0");
        let need = world.needs[0].clone();
        let completed = complete_need(
            &viewer,
            &need,
            "The roof is dry.",
            Posture::Lifts,
            &[AttachmentRef {
                media_id: "m1",
                description: None,
            }],
            "reply-1".into(),
            "t1".into(),
        )
        .unwrap();
        assert!(matches!(completed.writes[0], Write::InsertNeedReply(_)));
        assert!(matches!(
            completed.writes[1],
            Write::AttachReplyMedia { .. }
        ));
        assert!(matches!(completed.writes[2], Write::SetNeedStatus { .. }));
        assert!(matches!(completed.writes[3], Write::SetClosingReply { .. }));
        world.apply(completed, "t1");
        assert_eq!(world.need_replies[0].kind, ReplyKind::Completion);
        assert_eq!(world.reply_media.len(), 1);
        assert_eq!(world.needs[0].status, "closed");
        assert_eq!(world.needs[0].praise, None);
        assert_eq!(
            world.closing_replies,
            vec![("need-1".into(), "reply-1".into())]
        );
        assert_eq!(world.media_assets, vec!["m1".to_string()]);
        let reopened = reopen_need(&viewer, &world.needs[0]).unwrap();
        world.apply(reopened, "t2");
        assert_eq!(world.needs[0].status, "open");
        assert!(world.closing_replies.is_empty());
        assert_eq!(world.need_replies.len(), 1);
        assert_eq!(world.need_replies[0].kind, ReplyKind::Completion);
    }

    #[test]
    fn empty_attachment_writes_insert_no_links() {
        let viewer = author();
        let mut world = MemoryWorld::default();
        let posted = post_need_with_attachments(
            &viewer,
            "grace",
            "Roof",
            "It leaked.",
            None,
            CatalogPresence::Listed,
            "church",
            Posture::Lifts,
            &[],
            "need-1".into(),
            "t0".into(),
        )
        .unwrap();
        world.apply(posted, "t0");
        let completed = complete_need(
            &viewer,
            &world.needs[0],
            "The roof is dry.",
            Posture::Lifts,
            &[],
            "reply-1".into(),
            "t1".into(),
        )
        .unwrap();
        let Write::AttachReplyMedia { attachments, .. } = &completed.writes[1] else {
            panic!("completion attaches photos even when there are none");
        };
        assert!(attachments.is_empty());
        world.apply(completed, "t1");
        assert!(world.need_media.is_empty());
        assert!(world.reply_media.is_empty());
        assert_eq!(world.need_replies[0].kind, ReplyKind::Completion);
    }

    #[test]
    fn detach_removes_the_link_and_keeps_the_asset() {
        let mut world = MemoryWorld::default();
        world.media_assets.push("m1".into());
        world.apply(
            Effect {
                writes: vec![
                    Write::AttachNeedMedia {
                        need_id: "need-1".into(),
                        attachments: vec![Attachment {
                            media_id: "m1".into(),
                            position: 0,
                            description: Some("porch".into()),
                        }],
                    },
                    Write::DetachNeedMedia {
                        need_id: "need-1".into(),
                        media_id: "m1".into(),
                    },
                ],
                notices: vec![],
            },
            "t",
        );
        assert!(world.need_media.is_empty());
        assert_eq!(world.media_assets, vec!["m1".to_string()]);
    }

    #[test]
    fn avatar_set_replaces_and_clear_removes() {
        let mut world = MemoryWorld::default();
        world.apply(
            Effect::write(Write::SetAvatar {
                user_id: "ada".into(),
                media_id: "m1".into(),
            }),
            "t",
        );
        world.apply(
            Effect::write(Write::SetAvatar {
                user_id: "ada".into(),
                media_id: "m2".into(),
            }),
            "t",
        );
        assert_eq!(world.avatars, vec![("ada".into(), "m2".into())]);
        world.apply(
            Effect::write(Write::ClearAvatar {
                user_id: "ada".into(),
            }),
            "t",
        );
        assert!(world.avatars.is_empty());
    }

    #[test]
    fn an_offer_does_not_become_a_public_reply() {
        let viewer = author();
        let helper = viewer_of(
            user_in_church("bea", "grace", "member", "active"),
            Some(church("grace")),
        );
        let grace = church("grace");
        let mut world = MemoryWorld::default();
        let posted = post_need_with_attachments(
            &viewer,
            "grace",
            "Roof",
            "It leaked.",
            None,
            CatalogPresence::Listed,
            "church",
            Posture::Lifts,
            &[],
            "need-1".into(),
            "t0".into(),
        )
        .unwrap();
        world.apply(posted, "t0");
        let offer = apply_to_need(
            &helper,
            &world.needs[0],
            &grace,
            PriorOffer::Fresh,
            "I can bring soup on Thursday evening.",
            Posture::Lifts,
            "app-1".into(),
            "t1".into(),
        )
        .unwrap();
        world.apply(offer, "t1");
        assert!(world.need_replies.is_empty());
        assert_eq!(
            world.applications[0].message,
            "I can bring soup on Thursday evening."
        );
    }
}
