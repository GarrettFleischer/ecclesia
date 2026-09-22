use ecclesia_domain::{
    Application, Church, Effect, Endorsement, Membership, Need, Notification, User, Write,
};

/// In-process world. The SDK applies Domain effects here so stories can be
/// confirmed without SQLite or HTTP.
#[derive(Debug, Default, Clone)]
pub struct MemoryWorld {
    pub users: Vec<User>,
    pub churches: Vec<Church>,
    pub memberships: Vec<Membership>,
    pub member_gifts: Vec<(String, String, String)>,
    pub needs: Vec<Need>,
    pub applications: Vec<Application>,
    pub endorsements: Vec<Endorsement>,
    pub notifications: Vec<Notification>,
}

impl MemoryWorld {
    pub fn apply(&mut self, effect: Effect, now: &str) {
        apply_writes(self, effect.writes);
        apply_notices(self, effect.notices, now);
    }

    pub fn membership(&self, church_id: &str, user_id: &str) -> Option<&Membership> {
        self.memberships
            .iter()
            .find(|m| m.church_id == church_id && m.user_id == user_id)
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
            name,
            city,
            region,
            bio,
        } => update_user(world, id, name, city, region, bio),
        Write::InsertChurch(church) => world.churches.push(church),
        Write::InsertMembership(membership) => world.memberships.push(membership),
        Write::SetMembershipStatus { id, status } => set_membership_status(world, id, status),
        Write::InsertNeed(need) => world.needs.push(need),
        Write::SetNeedStatus { id, status } => set_need_status(world, id, status),
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
    }
}

fn update_user(
    world: &mut MemoryWorld,
    id: String,
    name: String,
    city: String,
    region: String,
    bio: String,
) {
    if let Some(user) = world.users.iter_mut().find(|user| user.id == id) {
        user.name = name;
        user.city = city;
        user.region = region;
        user.bio = bio;
    }
}

fn set_membership_status(world: &mut MemoryWorld, id: String, status: &'static str) {
    if let Some(membership) = world.memberships.iter_mut().find(|m| m.id == id) {
        membership.status = status.into();
    }
}

fn set_need_status(world: &mut MemoryWorld, id: String, status: &'static str) {
    if let Some(need) = world.needs.iter_mut().find(|need| need.id == id) {
        need.status = status.into();
    }
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
    world.member_gifts.push((user_id, gift_id, note));
}

fn remove_member_gift(world: &mut MemoryWorld, user_id: String, gift_id: String) {
    world
        .member_gifts
        .retain(|(u, g, _)| !(u == &user_id && g == &gift_id));
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
    use ecclesia_domain::sample::user_named;
    use ecclesia_domain::{
        Viewer, accept_endorsement, accept_invite, approve_membership, request_join,
    };

    fn user(id: &str, name: &str) -> User {
        user_named(id, name)
    }

    #[test]
    fn us_mem_01_sdk_applies_a_join_request() {
        let mut world = MemoryWorld::default();
        let peter = user("peter", "Peter");
        let church = Church {
            id: "grace".into(),
            name: "Grace".into(),
            city: "Cedar Falls".into(),
            region: "Iowa".into(),
            country: "US".into(),
            description: String::new(),
            gathering: String::new(),
            owner_id: "miriam".into(),
            invite_code: "grace-k2m9".into(),
            created_at: "t0".into(),
        };
        let effect = request_join(
            &peter,
            &church,
            None,
            &["miriam".into()],
            "mem1".into(),
            "t1".into(),
        )
        .unwrap();
        world.apply(effect, "t1");
        let membership = world.membership("grace", "peter").unwrap();
        assert_eq!(membership.status, "pending_request");
        assert_eq!(world.notifications[0].user_id, "miriam");
    }

    #[test]
    fn us_mem_04_sdk_approve_then_accept_invite_round_trip() {
        let mut world = MemoryWorld::default();
        world.memberships.push(Membership {
            id: "own".into(),
            church_id: "grace".into(),
            user_id: "miriam".into(),
            role: "owner".into(),
            status: "active".into(),
            created_at: "t0".into(),
        });
        world.memberships.push(Membership {
            id: "inv".into(),
            church_id: "grace".into(),
            user_id: "peter".into(),
            role: "member".into(),
            status: "pending_invite".into(),
            created_at: "t0".into(),
        });
        let peter = user("peter", "Peter");
        let effect = accept_invite(&peter, world.membership("grace", "peter").unwrap()).unwrap();
        world.apply(effect, "t1");
        assert_eq!(world.membership("grace", "peter").unwrap().status, "active");
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
        let membership = Membership {
            id: "m1".into(),
            church_id: "grace".into(),
            user_id: "ruth".into(),
            role: "member".into(),
            status: "active".into(),
            created_at: "t0".into(),
        };
        let viewer = Viewer {
            user: user("ruth", "Ruth"),
            memberships: vec![membership.clone()],
            churches: vec![],
            gift_ids: vec![],
        };
        let target = Membership {
            id: "m2".into(),
            church_id: "grace".into(),
            user_id: "peter".into(),
            role: "member".into(),
            status: "pending_request".into(),
            created_at: "t0".into(),
        };
        let church = Church {
            id: "grace".into(),
            name: "Grace".into(),
            city: "Cedar Falls".into(),
            region: "Iowa".into(),
            country: "US".into(),
            description: String::new(),
            gathering: String::new(),
            owner_id: "miriam".into(),
            invite_code: "x".into(),
            created_at: "t0".into(),
        };
        assert!(approve_membership(&viewer, &target, &church).is_err());
    }
}
