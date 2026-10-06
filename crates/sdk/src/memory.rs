use ecclesia_domain::{
    Application, Church, Effect, Endorsement, Membership, Need, NeedReply, NeedShelf, Notification,
    Prayer, Share, User, Write,
};

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
    use ecclesia_domain::sample::{church, user_in_church, user_named};
    use ecclesia_domain::{
        Viewer, accept_endorsement, accept_invite, approve_membership, replace_with_pending,
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
}
