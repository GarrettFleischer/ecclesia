use crate::leaf::{
    Application, Church, Effect, Endorsement, Membership, Need, Notification, User, Write,
};

/// In-process world. The SDK applies leaf effects here so stories can be
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
    pub fn apply(&mut self, effect: &Effect, now: &str) {
        for write in &effect.writes {
            match write {
                Write::InsertUser(user) => self.users.push(user.clone()),
                Write::UpdateUser {
                    id,
                    name,
                    city,
                    region,
                    bio,
                } => {
                    if let Some(user) = self.users.iter_mut().find(|user| user.id == *id) {
                        user.name = name.clone();
                        user.city = city.clone();
                        user.region = region.clone();
                        user.bio = bio.clone();
                    }
                }
                Write::InsertChurch(church) => self.churches.push(church.clone()),
                Write::InsertMembership(membership) => self.memberships.push(membership.clone()),
                Write::SetMembershipStatus { id, status } => {
                    if let Some(membership) = self.memberships.iter_mut().find(|m| m.id == *id) {
                        membership.status = status.clone();
                    }
                }
                Write::InsertNeed(need) => self.needs.push(need.clone()),
                Write::SetNeedStatus { id, status } => {
                    if let Some(need) = self.needs.iter_mut().find(|need| need.id == *id) {
                        need.status = status.clone();
                    }
                }
                Write::InsertApplication(application) => {
                    self.applications.push(application.clone())
                }
                Write::SetApplicationStatus { id, status } => {
                    if let Some(application) = self.applications.iter_mut().find(|a| a.id == *id) {
                        application.status = status.clone();
                    }
                }
                Write::InsertEndorsement(endorsement) => {
                    self.endorsements.push(endorsement.clone())
                }
                Write::SetEndorsementStatus { id, status } => {
                    if let Some(endorsement) = self.endorsements.iter_mut().find(|e| e.id == *id) {
                        endorsement.status = status.clone();
                    }
                }
                Write::UpsertMemberGift {
                    user_id,
                    gift_id,
                    note,
                } => {
                    if let Some(existing) = self
                        .member_gifts
                        .iter_mut()
                        .find(|(u, g, _)| u == user_id && g == gift_id)
                    {
                        existing.2 = note.clone();
                    } else {
                        self.member_gifts
                            .push((user_id.clone(), gift_id.clone(), note.clone()));
                    }
                }
                Write::RemoveMemberGift { user_id, gift_id } => {
                    self.member_gifts
                        .retain(|(u, g, _)| !(u == user_id && g == gift_id));
                }
            }
        }
        for notice in &effect.notices {
            self.notifications.push(Notification {
                id: format!("n-{}", self.notifications.len()),
                user_id: notice.user_id.clone(),
                kind: notice.kind.clone(),
                title: notice.title.clone(),
                body: notice.body.clone(),
                href: notice.href.clone(),
                read: 0,
                created_at: now.into(),
            });
        }
    }

    pub fn membership(&self, church_id: &str, user_id: &str) -> Option<&Membership> {
        self.memberships
            .iter()
            .find(|m| m.church_id == church_id && m.user_id == user_id)
    }

    pub fn user(&self, id: &str) -> Option<&User> {
        self.users.iter().find(|user| user.id == id)
    }

    pub fn gifts_for(&self, user_id: &str) -> Vec<&(String, String, String)> {
        self.member_gifts
            .iter()
            .filter(|(id, _, _)| id == user_id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leaf::{accept_invite, decide_endorsement, decide_membership, request_join, Viewer};

    fn user(id: &str, name: &str) -> User {
        User {
            id: id.into(),
            name: name.into(),
            email: format!("{id}@ecclesia.test"),
            city: "Cedar Falls".into(),
            region: "Iowa".into(),
            bio: String::new(),
            created_at: "t0".into(),
        }
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
        world.apply(&effect, "t1");
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
        world.apply(&effect, "t1");
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
            note: "She stayed.".into(),
            status: "pending".into(),
            created_at: "t0".into(),
        });
        let ruth = user("ruth", "Ruth");
        let effect =
            decide_endorsement(&ruth, &world.endorsements[0], true, "Hospitality").unwrap();
        world.apply(&effect, "t1");
        assert_eq!(world.endorsements[0].status, "accepted");
        assert_eq!(world.gifts_for("ruth").len(), 1);
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
        assert!(decide_membership(&viewer, &target, &church, true).is_err());
    }
}
