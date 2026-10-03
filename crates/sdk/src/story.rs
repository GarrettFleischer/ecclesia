//! One function per user story. HTTP extracts a form and calls one of these.

use ecclesia_domain::{
    CatalogPresence, Church, ChurchCard, ChurchMember, DomainError, Effect, EndorsementQueue, Gift,
    GiftOnProfile, NeedApproach, NeedCard, Place, PrayerByline, PrayerCard, PrayerMarkKind,
    PrayerProof, PrayerReach, PrayerSight, PrayerSource, PriorOffer, PriorPrayerMark, SkillSource,
    User, Viewer, VoiceKind,
    accept_application as domain_accept_application,
    accept_endorsement as domain_accept_endorsement, accept_invite as domain_accept_invite,
    add_gift as domain_add_gift,     answer_prayer as domain_answer_prayer, apply_to_need as domain_apply_to_need,
    mark_prayer as domain_mark_prayer,
    approve_membership as domain_approve_membership, churches_with_counts,
    close_need as domain_close_need, coordinates,
    decline_application as domain_decline_application,
    decline_endorsement as domain_decline_endorsement,
    decline_membership as domain_decline_membership, endorse as domain_endorse,
    pick_daily_prayer, plant_church as domain_plant_church, post_need as domain_post_need,
    post_prayer as domain_post_prayer, remove_gift as domain_remove_gift, replace_with_code,
    replace_with_pending, reply_to_need as domain_reply_to_need,
    update_profile as domain_update_profile, visible_need_cards, visible_needs_near,
    visible_prayers_near,
};

use crate::cache::Cache;
use crate::clock::{new_id, nonce, now_iso, today_utc};
use crate::db::{Db, StoryExtras};
use crate::judge::JudgeHub;
use crate::limit::RateGate;
use crate::push::PushHub;
use crate::refine::RefineHub;

pub use crate::bearer::ApiSessionTokens;
pub use crate::identity::{
    ApiProfile, BearerIdentity, DeviceMeta, MailOrigin, api_logout_bearer, api_me_profile,
    change_password, complete_reset, consume_magic, logout, logout_all, refresh_api, register,
    request_magic, request_reset, reset_form_ok, resolve_bearer, resolve_session, revoke_session,
    sign_in, sign_in_api,
};

#[derive(Clone)]
pub struct Sdk {
    pub db: Db,
    pub cache: Cache,
    pub judge: JudgeHub,
    pub refine: RefineHub,
    pub push: PushHub,
    pub gate: RateGate,
}

impl Sdk {
    pub fn assemble(
        db: Db,
        judge: JudgeHub,
        refine: RefineHub,
        push: PushHub,
        cache: Cache,
    ) -> Self {
        Self {
            db,
            gate: RateGate::with_cache(cache.clone()),
            cache,
            judge,
            refine,
            push,
        }
    }

    pub async fn commit(&self, effect: &Effect) -> anyhow::Result<()> {
        self.commit_with(effect, &StoryExtras::default()).await
    }

    pub async fn commit_with(&self, effect: &Effect, extras: &StoryExtras) -> anyhow::Result<()> {
        let keys = self.db.apply_with(effect, extras).await?;
        self.cache.del_many(&keys).await;
        Ok(())
    }

    pub async fn weigh(&self, kind: VoiceKind, parts: &[&str]) -> ecclesia_domain::Posture {
        self.judge.weigh(kind, parts).await
    }

    pub async fn viewer(&self, user: User) -> anyhow::Result<Viewer> {
        let church = match user.church_id.as_deref() {
            Some(id) => self.db.church(id).await?,
            None => None,
        };
        let gift_ids = self.db.gift_ids_for(&user.id).await?;
        Ok(Viewer {
            user,
            church,
            gift_ids,
        })
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StoryOk {
    pub user_id: Option<String>,
    pub church_id: Option<String>,
    pub need_id: Option<String>,
    pub membership_id: Option<String>,
    pub session_id: Option<String>,
    pub csrf: Option<String>,
}

impl StoryOk {
    pub fn from_effect(effect: &Effect) -> Self {
        Self {
            user_id: effect.inserted_user_id().map(str::to_owned),
            church_id: effect
                .inserted_church_id()
                .or_else(|| effect.linked_church_id())
                .map(str::to_owned),
            need_id: effect.inserted_need_id().map(str::to_owned),
            membership_id: None,
            session_id: None,
            csrf: None,
        }
    }
}

pub async fn finish(
    sdk: &Sdk,
    effect: Result<Effect, DomainError>,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    match effect {
        Ok(effect) => {
            if !effect.writes.is_empty() {
                sdk.commit(&effect).await?;
            }
            Ok(Ok(StoryOk::from_effect(&effect)))
        }
        Err(error) => Ok(Err(error)),
    }
}

pub async fn finish_with(
    sdk: &Sdk,
    effect: Result<Effect, DomainError>,
    extras: StoryExtras,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    match effect {
        Ok(effect) => {
            sdk.commit_with(&effect, &extras).await?;
            Ok(Ok(StoryOk::from_effect(&effect)))
        }
        Err(error) => Ok(Err(error)),
    }
}

pub async fn plant_church(
    sdk: &Sdk,
    planter: &User,
    name: &str,
    address: &str,
    latitude: f64,
    longitude: f64,
    description: &str,
    gathering: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let posture = sdk
        .weigh(VoiceKind::Church, &[name, address, gathering, description])
        .await;
    finish(
        sdk,
        domain_plant_church(
            planter,
            name,
            address,
            latitude,
            longitude,
            description,
            gathering,
            posture,
            new_id(),
            &nonce(),
            now_iso(),
        ),
    )
    .await
}

pub async fn request_join(
    sdk: &Sdk,
    user: &User,
    church_id: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some(church) = sdk.db.church(church_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    let governors = sdk.db.governor_ids(church_id).await?;
    finish(sdk, replace_with_pending(user, &church, &governors)).await
}

pub async fn invite_member(
    sdk: &Sdk,
    viewer: &Viewer,
    church_id: &str,
    email: &str,
    origin: &crate::identity::MailOrigin,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    crate::identity::invite_member(sdk, viewer, church_id, email, origin).await
}

pub async fn redeem_invite(
    sdk: &Sdk,
    user: &User,
    code: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some(church) = sdk.db.church_by_invite(code).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    let church_id = church.id.clone();
    match finish(sdk, replace_with_code(user, &church)).await? {
        Ok(mut ok) => {
            if ok.church_id.is_none() {
                ok.church_id = Some(church_id);
            }
            Ok(Ok(ok))
        }
        Err(error) => Ok(Err(error)),
    }
}

pub async fn approve_membership(
    sdk: &Sdk,
    viewer: &Viewer,
    user_id: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some((target, church)) = load_linked_church(sdk, user_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    finish(sdk, domain_approve_membership(viewer, &target, &church)).await
}

pub async fn decline_membership(
    sdk: &Sdk,
    viewer: &Viewer,
    user_id: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some((target, church)) = load_linked_church(sdk, user_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    finish(sdk, domain_decline_membership(viewer, &target, &church)).await
}

pub async fn accept_invite(sdk: &Sdk, user: &User) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let church_id = user.church_id.clone();
    match finish(sdk, domain_accept_invite(user)).await? {
        Ok(mut ok) => {
            if ok.church_id.is_none() {
                ok.church_id = church_id;
            }
            Ok(Ok(ok))
        }
        Err(error) => Ok(Err(error)),
    }
}

pub async fn post_need(
    sdk: &Sdk,
    viewer: &Viewer,
    church_id: &str,
    title: &str,
    body: &str,
    gift_id: Option<&str>,
    scope: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let presence = match gift_id {
        Some(id) => CatalogPresence::of_lookup(sdk.db.gift(id).await?),
        None => CatalogPresence::Listed,
    };
    let posture = sdk.weigh(VoiceKind::Need, &[title, body]).await;
    finish(
        sdk,
        domain_post_need(
            viewer,
            church_id,
            title,
            body,
            gift_id,
            presence,
            scope,
            posture,
            new_id(),
            now_iso(),
        ),
    )
    .await
}

pub async fn apply_to_need(
    sdk: &Sdk,
    viewer: &Viewer,
    need_id: &str,
    message: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some(need) = sdk.db.need(need_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    let Some(church) = sdk.db.church(&need.church_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    let prior = PriorOffer::of_existing(sdk.db.application_pair(&need.id, &viewer.user.id).await?);
    let posture = sdk.weigh(VoiceKind::Offer, &[message]).await;
    finish(
        sdk,
        domain_apply_to_need(
            viewer,
            &need,
            &church,
            prior,
            message,
            posture,
            new_id(),
            now_iso(),
        ),
    )
    .await
}

pub async fn close_need(
    sdk: &Sdk,
    viewer: &Viewer,
    need_id: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some(need) = sdk.db.need(need_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    finish(sdk, domain_close_need(viewer, &need)).await
}

pub async fn accept_application(
    sdk: &Sdk,
    viewer: &Viewer,
    application_id: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some((need, application)) = load_application_need(sdk, application_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    finish(sdk, domain_accept_application(viewer, &need, &application)).await
}

pub async fn decline_application(
    sdk: &Sdk,
    viewer: &Viewer,
    application_id: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some((need, application)) = load_application_need(sdk, application_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    finish(sdk, domain_decline_application(viewer, &need, &application)).await
}

pub async fn endorse(
    sdk: &Sdk,
    from: &User,
    to_id: &str,
    skill: SkillSource<'_>,
    note: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some(person) = sdk.db.user(to_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    let queue = EndorsementQueue::of_existing(
        sdk.db
            .pending_endorsement(&from.id, to_id, skill.display())
            .await?,
    );
    let posture = sdk
        .weigh(VoiceKind::Endorsement, &[skill.display(), note])
        .await;
    finish(
        sdk,
        domain_endorse(
            from,
            &person,
            skill,
            queue,
            note,
            posture,
            new_id(),
            now_iso(),
        ),
    )
    .await
}

pub async fn accept_endorsement(
    sdk: &Sdk,
    user: &User,
    endorsement_id: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some(endorsement) = sdk.db.endorsement(endorsement_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    let gift_ids = sdk.db.gift_ids_for(&user.id).await?;
    let held = GiftOnProfile::of_ids(&gift_ids, &endorsement.gift_id);
    finish(sdk, domain_accept_endorsement(user, &endorsement, held)).await
}

pub async fn decline_endorsement(
    sdk: &Sdk,
    user: &User,
    endorsement_id: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some(endorsement) = sdk.db.endorsement(endorsement_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    finish(sdk, domain_decline_endorsement(user, &endorsement)).await
}

pub async fn update_profile(
    sdk: &Sdk,
    user_id: &str,
    first_name: &str,
    last_name: &str,
    bio: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let posture = sdk
        .weigh(VoiceKind::Bio, &[first_name, last_name, bio])
        .await;
    finish(
        sdk,
        domain_update_profile(user_id, first_name, last_name, bio, posture),
    )
    .await
}

pub async fn join_finder(
    sdk: &Sdk,
    except_church_id: Option<&str>,
    place: Option<Place>,
    query: &str,
) -> anyhow::Result<Result<Vec<ChurchSearchHit>, DomainError>> {
    let query = query.trim();
    let place = match place {
        Some(place) => match coordinates(place.latitude, place.longitude) {
            Ok((latitude, longitude)) => Some((latitude, longitude)),
            Err(error) if query.is_empty() => return Ok(Err(error)),
            Err(_) => None,
        },
        None => None,
    };
    if !query.is_empty() {
        return Ok(Ok(search_churches(sdk, query, 8).await?));
    }
    let Some((latitude, longitude)) = place else {
        return Ok(Ok(Vec::new()));
    };
    let churches = sdk
        .db
        .churches_near(latitude, longitude, except_church_id)
        .await?;
    Ok(Ok(churches.iter().map(search_hit).collect()))
}

pub async fn add_gift(
    sdk: &Sdk,
    user_id: &str,
    gift_id: &str,
    note: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let presence = CatalogPresence::of_lookup(sdk.db.gift(gift_id).await?);
    let posture = sdk.weigh(VoiceKind::GiftNote, &[note]).await;
    finish(
        sdk,
        domain_add_gift(user_id, gift_id, presence, note, posture),
    )
    .await
}

pub async fn remove_gift(
    sdk: &Sdk,
    user_id: &str,
    gift_id: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    finish(sdk, domain_remove_gift(user_id, gift_id)).await
}

pub async fn subscribe_push(
    sdk: &Sdk,
    user_id: &str,
    endpoint: &str,
    p256dh: &str,
    auth: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let endpoint = match ecclesia_domain::https_endpoint(endpoint) {
        Ok(endpoint) => endpoint,
        Err(error) => return Ok(Err(error)),
    };
    let p256dh = match ecclesia_domain::push_key(p256dh) {
        Ok(key) => key,
        Err(error) => return Ok(Err(error)),
    };
    let auth = match ecclesia_domain::push_key(auth) {
        Ok(key) => key,
        Err(error) => return Ok(Err(error)),
    };
    sdk.db
        .upsert_push_subscription(user_id, &endpoint, &p256dh, &auth)
        .await?;
    Ok(Ok(StoryOk::default()))
}

pub async fn unsubscribe_push(
    sdk: &Sdk,
    user_id: &str,
    endpoint: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let endpoint = match ecclesia_domain::https_endpoint(endpoint) {
        Ok(endpoint) => endpoint,
        Err(error) => return Ok(Err(error)),
    };
    sdk.db.remove_push_subscription(user_id, &endpoint).await?;
    Ok(Ok(StoryOk::default()))
}

pub async fn register_device(
    sdk: &Sdk,
    user_id: &str,
    token: &str,
    platform: &str,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let token = match ecclesia_domain::device_token(token) {
        Ok(token) => token,
        Err(error) => return Ok(Err(error)),
    };
    let platform = match ecclesia_domain::push_platform(platform) {
        Ok(platform) => platform,
        Err(error) => return Ok(Err(error)),
    };
    sdk.db.upsert_push_device(user_id, &token, platform).await?;
    Ok(Ok(StoryOk::default()))
}

pub struct NeedPage {
    pub cards: Vec<NeedCard>,
    pub churches: Vec<Church>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct DirectoryPage {
    pub cards: Vec<ChurchCard>,
    pub next_cursor: Option<String>,
}

pub struct ChurchPage {
    pub church: Church,
    pub members: Vec<ChurchMember>,
    pub needs: Vec<NeedCard>,
    pub answered: Vec<PrayerCard>,
    pub next_need_cursor: Option<String>,
    pub next_member_cursor: Option<String>,
}

pub async fn home_needs(
    sdk: &Sdk,
    viewer: &Viewer,
    after: Option<&str>,
) -> anyhow::Result<NeedPage> {
    let cursor = parse_need_cursor(after);
    let sql_page = sdk.db.home_need_cards(viewer, cursor).await?;
    let page_ids: Vec<&str> = unique_church_ids(&sql_page);
    let page_churches = sdk.db.churches_with_ids(&page_ids).await?;
    let churches = union_churches(viewer.church.as_ref(), page_churches);
    let visible = take_visible_cards(viewer, &sql_page, &churches);
    let next_cursor = next_need_cursor(&visible);
    Ok(NeedPage {
        cards: visible,
        churches,
        next_cursor,
    })
}

pub async fn church_directory(sdk: &Sdk, after: Option<&str>) -> anyhow::Result<DirectoryPage> {
    if after.is_none() {
        if let Some(page) = sdk.cache.get_json::<DirectoryPage>("directory").await {
            return Ok(page);
        }
    }
    let cursor = parse_church_cursor(after);
    let churches = sdk.db.churches_page(cursor).await?;
    let ids: Vec<&str> = churches.iter().map(|church| church.id.as_str()).collect();
    let counts = sdk.db.counts_for_church_ids(&ids).await?;
    let next_cursor = next_church_cursor(&churches);
    let page = DirectoryPage {
        cards: churches_with_counts(churches, &counts),
        next_cursor,
    };
    if after.is_none() {
        sdk.cache.set_json("directory", &page).await;
    }
    Ok(page)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ChurchSearchHit {
    pub id: String,
    pub name: String,
    pub address: String,
}

pub async fn search_churches(
    sdk: &Sdk,
    query: &str,
    limit: usize,
) -> anyhow::Result<Vec<ChurchSearchHit>> {
    use crate::church_search::rank_churches;
    let churches = sdk.db.churches_for_lookup().await?;
    Ok(rank_churches(query, &churches, limit)
        .into_iter()
        .map(search_hit)
        .collect())
}

fn search_hit(church: &Church) -> ChurchSearchHit {
    ChurchSearchHit {
        id: church.id.clone(),
        name: church.name.clone(),
        address: church.address.clone(),
    }
}

pub async fn church_show(
    sdk: &Sdk,
    church_id: &str,
    need_after: Option<&str>,
    member_after: Option<&str>,
) -> anyhow::Result<Option<ChurchPage>> {
    let Some(church) = cached_or_store_church(sdk, church_id).await? else {
        return Ok(None);
    };
    let needs = sdk
        .db
        .church_need_cards_page(church_id, parse_need_cursor(need_after))
        .await?;
    let members = sdk
        .db
        .church_members_page(church_id, parse_member_cursor(member_after))
        .await?;
    let answered = sdk.db.answered_prayers(church_id).await?;
    let next_need_cursor = next_need_cursor(&needs);
    let next_member_cursor = next_member_cursor(&members);
    Ok(Some(ChurchPage {
        church,
        members,
        needs,
        answered,
        next_need_cursor,
        next_member_cursor,
    }))
}

pub async fn gift_catalog(sdk: &Sdk) -> anyhow::Result<Vec<Gift>> {
    if let Some(gifts) = sdk.cache.get_json("catalog").await {
        return Ok(gifts);
    }
    let gifts = sdk.db.gifts().await?;
    sdk.cache.set_json("catalog", &gifts).await;
    Ok(gifts)
}

async fn cached_or_store_church(sdk: &Sdk, church_id: &str) -> anyhow::Result<Option<Church>> {
    let key = format!("church:{church_id}");
    if let Some(card) = sdk.cache.get_json::<ChurchCardJson>(&key).await {
        return Ok(Some(card.church));
    }
    let Some(church) = sdk.db.church(church_id).await? else {
        return Ok(None);
    };
    let (members, needs) = sdk.db.counts_for_one_church(church_id).await?;
    sdk.cache
        .set_json(
            &key,
            &ChurchCardJson {
                church: church.clone(),
                members,
                needs,
            },
        )
        .await;
    Ok(Some(church))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ChurchCardJson {
    church: Church,
    members: i64,
    needs: i64,
}

fn parse_need_cursor(after: Option<&str>) -> Option<(&str, &str)> {
    let after = after?;
    let (created_at, id) = after.split_once('|')?;
    Some((created_at, id))
}

fn parse_church_cursor(after: Option<&str>) -> Option<(&str, &str)> {
    let after = after?;
    let (name, id) = after.split_once('|')?;
    Some((name, id))
}

fn parse_member_cursor(after: Option<&str>) -> Option<(&str, &str, &str)> {
    let after = after?;
    let mut parts = after.splitn(3, '|');
    let first = parts.next()?;
    let last = parts.next()?;
    let id = parts.next()?;
    Some((first, last, id))
}

fn next_need_cursor(cards: &[NeedCard]) -> Option<String> {
    if cards.len() < 20 {
        return None;
    }
    let last = cards.last()?;
    Some(format!("{}|{}", last.created_at, last.id))
}

fn next_church_cursor(churches: &[Church]) -> Option<String> {
    if churches.len() < 20 {
        return None;
    }
    let last = churches.last()?;
    Some(format!("{}|{}", last.name, last.id))
}

fn next_member_cursor(members: &[ChurchMember]) -> Option<String> {
    if members.len() < 20 {
        return None;
    }
    let last = members.last()?;
    Some(format!(
        "{}|{}|{}",
        last.first_name, last.last_name, last.user_id
    ))
}

fn take_visible_cards(viewer: &Viewer, cards: &[NeedCard], churches: &[Church]) -> Vec<NeedCard> {
    let mut kept = Vec::new();
    for card in visible_need_cards(viewer, cards, churches) {
        kept.push(card.clone());
        if kept.len() == 20 {
            break;
        }
    }
    kept
}

fn unique_church_ids(cards: &[NeedCard]) -> Vec<&str> {
    let mut ids = Vec::new();
    for card in cards {
        if !ids.contains(&card.church_id.as_str()) {
            ids.push(card.church_id.as_str());
        }
    }
    ids
}

fn union_churches(mine: Option<&Church>, page: Vec<Church>) -> Vec<Church> {
    let mut out = Vec::new();
    if let Some(church) = mine {
        push_unique_church(&mut out, church);
    }
    for church in page {
        if !out.iter().any(|have| have.id == church.id) {
            out.push(church);
        }
    }
    out
}

fn push_unique_church(out: &mut Vec<Church>, church: &Church) {
    if out.iter().any(|have| have.id == church.id) {
        return;
    }
    out.push(church.clone());
}

async fn load_linked_church(sdk: &Sdk, user_id: &str) -> anyhow::Result<Option<(User, Church)>> {
    let Some(target) = sdk.db.user(user_id).await? else {
        return Ok(None);
    };
    let Some(church_id) = target.church_id.as_deref() else {
        return Ok(None);
    };
    let Some(church) = sdk.db.church(church_id).await? else {
        return Ok(None);
    };
    Ok(Some((target, church)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecclesia_domain::{Effect, Need, User, Write};

    async fn sdk_on(path: &std::path::Path) -> Sdk {
        let db = Db::connect(&format!("sqlite://{}", path.display()))
            .await
            .expect("test database");
        Sdk::assemble(
            db,
            JudgeHub::silent(),
            RefineHub::silent(),
            PushHub::silent(),
            Cache::memory(),
        )
    }

    async fn fresh_sdk() -> Sdk {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "ecclesia-story-{}-{}-{}.db",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        sdk_on(&path).await
    }

    fn test_device() -> DeviceMeta {
        DeviceMeta {
            user_agent: String::new(),
            ip: "local".into(),
        }
    }

    async fn register_named(sdk: &Sdk, name: &str, email: &str) -> User {
        register(
            sdk,
            name,
            "Lane",
            email,
            "Thursday dinners at six oclock",
            &test_device(),
        )
        .await
        .unwrap()
        .unwrap();
        sdk.db.user_by_email(email).await.unwrap().expect(email)
    }

    #[tokio::test]
    async fn us_auth_02_second_handle_loads_the_same_session() {
        let path = std::env::temp_dir().join(format!(
            "ecclesia-session-share-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let first = sdk_on(&path).await;
        let ok = register(
            &first,
            "Ada",
            "Cole",
            "ada@share.test",
            "Thursday dinners at six oclock",
            &test_device(),
        )
        .await
        .unwrap()
        .unwrap();
        let cookie = crate::session::Session::signed_in(
            ok.session_id.clone().expect("session"),
            ok.user_id.clone().expect("user"),
            ok.csrf.clone().expect("csrf"),
        );
        let raw = cookie.encode("verify-secret");
        let parsed = crate::session::Session::decode("verify-secret", &raw).expect("cookie");
        assert!(parsed.user_id.is_none());
        let second = sdk_on(&path).await;
        let live = resolve_session(&second, parsed).await.unwrap();
        assert_eq!(live.user_id.as_deref(), ok.user_id.as_deref());
        logout(&first, live.session_id.as_deref().expect("id"))
            .await
            .unwrap();
        let spent = crate::session::Session::decode("verify-secret", &raw).expect("cookie");
        let guest = resolve_session(&second, spent).await.unwrap();
        assert!(guest.user_id.is_none());
        assert!(guest.session_id.is_none());
    }

    fn mail_origin() -> MailOrigin {
        MailOrigin {
            origin: "http://127.0.0.1:43781".into(),
        }
    }

    async fn latest_mail_text(sdk: &Sdk) -> String {
        let rows = sdk.db.pending_outbox().await.unwrap();
        let row = rows
            .into_iter()
            .rev()
            .find(|row| row.kind == "mail")
            .expect("mail row");
        let payload: serde_json::Value = serde_json::from_str(&row.payload).unwrap();
        payload["text"].as_str().expect("text").to_string()
    }

    fn href_parts(text: &str) -> (String, String) {
        let href = text.lines().last().expect("href");
        let tail = href.rsplit('/').next().expect("id query");
        let (id, secret) = tail.split_once("?t=").expect("token query");
        (id.to_string(), secret.to_string())
    }

    async fn magic_count(sdk: &Sdk) -> i64 {
        #[derive(sqlx::FromRow)]
        struct CountRow {
            n: i64,
        }
        sdk.db
            .fetch_all::<CountRow>("SELECT count(*) AS n FROM magic_links", &[])
            .await
            .unwrap()
            .into_iter()
            .next()
            .map(|row| row.n)
            .unwrap_or(0)
    }

    #[tokio::test]
    async fn us_auth_01_weak_register_inserts_no_user() {
        let sdk = fresh_sdk().await;
        let error = register(
            &sdk,
            "Cara",
            "Nguyen",
            "cara@verify.test",
            "password",
            &test_device(),
        )
        .await
        .unwrap()
        .expect_err("weak");
        assert_eq!(error, DomainError::WeakPassword);
        assert!(
            sdk.db
                .user_by_email("cara@verify.test")
                .await
                .unwrap()
                .is_none()
        );
        assert!(sdk.db.pending_outbox().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn us_mail_04_unknown_magic_writes_no_row() {
        let sdk = fresh_sdk().await;
        request_magic(&sdk, "ghost@nowhere.test", &mail_origin())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(magic_count(&sdk).await, 0);
        assert!(sdk.db.pending_outbox().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn us_mail_05_invite_known_writes_mail_unknown_does_not() {
        let sdk = fresh_sdk().await;
        let pastor = register_named(&sdk, "Ada Cole", "ada@verify.test").await;
        let planted = plant_church(
            &sdk,
            &pastor,
            "Grace Covenant",
            "100 Main Street",
            42.53,
            -92.45,
            "A church on Main Street.",
            "Sunday at 10.",
        )
        .await
        .unwrap()
        .unwrap();
        let church_id = planted.church_id.expect("church");
        let pastor = sdk.db.user(&pastor.id).await.unwrap().expect("pastor");
        let _guest = register_named(&sdk, "James Whitaker", "james@stlukes.test").await;
        let viewer = sdk.viewer(pastor).await.unwrap();
        invite_member(&sdk, &viewer, &church_id, "nobody@x.test", &mail_origin())
            .await
            .unwrap()
            .unwrap();
        let after_unknown = sdk.db.pending_outbox().await.unwrap();
        assert!(
            after_unknown
                .iter()
                .filter(|row| row.kind == "mail")
                .all(|row| !row.payload.contains("nobody@x.test"))
        );
        invite_member(
            &sdk,
            &viewer,
            &church_id,
            "james@stlukes.test",
            &mail_origin(),
        )
        .await
        .unwrap()
        .unwrap();
        let church = sdk.db.church(&church_id).await.unwrap().expect("church");
        let text = latest_mail_text(&sdk).await;
        assert!(text.contains(&church.invite_code), "mail was {text}");
        assert!(
            text.contains("http://127.0.0.1:43781/churches"),
            "mail was {text}"
        );
    }

    #[tokio::test]
    async fn us_auth_03_change_password_kills_other_sessions() {
        let sdk = fresh_sdk().await;
        let user = register_named(&sdk, "Ada Cole", "ada@verify.test").await;
        sign_in(
            &sdk,
            "ada@verify.test",
            "Thursday dinners at six oclock",
            &test_device(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(sdk.db.sessions_for_user(&user.id).await.unwrap().len(), 2);
        let ok = change_password(
            &sdk,
            &user,
            "Thursday dinners at six oclock",
            "Saturday lunch at noon sharp",
            &test_device(),
        )
        .await
        .unwrap()
        .unwrap();
        let live = sdk.db.sessions_for_user(&user.id).await.unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].id, ok.session_id.expect("new session"));
    }

    #[tokio::test]
    async fn us_api_03_parallel_refresh_has_one_winner() {
        let path = std::env::temp_dir().join(format!(
            "ecclesia-refresh-race-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let first = sdk_on(&path).await;
        register_named(&first, "Race Ada", "race@verify.test").await;
        let tokens = crate::identity::sign_in_api(
            &first,
            "verify-secret",
            "race@verify.test",
            "Thursday dinners at six oclock",
            &test_device(),
        )
        .await
        .unwrap()
        .unwrap();
        let refresh = tokens.refresh_token.clone();
        let second = sdk_on(&path).await;
        let (a, b) = tokio::join!(
            crate::identity::refresh_api(&first, "verify-secret", &refresh),
            crate::identity::refresh_api(&second, "verify-secret", &refresh),
        );
        let wins = [a.unwrap().is_some(), b.unwrap().is_some()];
        assert_eq!(wins.iter().filter(|won| **won).count(), 1);
    }

    #[tokio::test]
    async fn us_api_02_api_row_does_not_bind_cookie() {
        let sdk = fresh_sdk().await;
        register_named(&sdk, "Api Ada", "api@verify.test").await;
        let tokens = crate::identity::sign_in_api(
            &sdk,
            "verify-secret",
            "api@verify.test",
            "Thursday dinners at six oclock",
            &test_device(),
        )
        .await
        .unwrap()
        .unwrap();
        let who = crate::identity::resolve_bearer(&sdk, "verify-secret", &tokens.access_token)
            .await
            .unwrap()
            .expect("bearer");
        let row = sdk.db.session(&who.session_id).await.unwrap().expect("row");
        let cookie = crate::session::Session::signed_in(
            row.id.clone(),
            who.user_id.clone(),
            row.csrf.clone(),
        );
        let live = resolve_session(&sdk, cookie).await.unwrap();
        assert!(live.user_id.is_none());
    }

    #[tokio::test]
    async fn us_api_02_sign_in_api_mints_bearer_session() {
        let sdk = fresh_sdk().await;
        register_named(&sdk, "Api Ada", "api@verify.test").await;
        let tokens = crate::identity::sign_in_api(
            &sdk,
            "verify-secret",
            "api@verify.test",
            "Thursday dinners at six oclock",
            &test_device(),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(tokens.access_token.starts_with("v3.at."));
        assert!(tokens.refresh_token.starts_with("rt."));
        let who = crate::identity::resolve_bearer(&sdk, "verify-secret", &tokens.access_token)
            .await
            .unwrap()
            .expect("bearer");
        let row = sdk.db.session(&who.session_id).await.unwrap().expect("row");
        assert_eq!(row.transport, "api");
        assert!(row.refresh_token_hash.is_some());
    }

    #[tokio::test]
    async fn us_auth_03_revoke_other_person_is_a_miss() {
        let sdk = fresh_sdk().await;
        let ada = register_named(&sdk, "Ada Cole", "ada@verify.test").await;
        let james = register_named(&sdk, "James Whitaker", "james@stlukes.test").await;
        let ada_session = sdk.db.sessions_for_user(&ada.id).await.unwrap();
        let target = ada_session[0].id.clone();
        let error = revoke_session(&sdk, &james.id, &target)
            .await
            .unwrap()
            .expect_err("foreign revoke");
        assert_eq!(error, DomainError::NotFound);
        assert!(sdk.db.session(&target).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn us_mail_08_magic_is_one_use() {
        let sdk = fresh_sdk().await;
        let user = register_named(&sdk, "Ada Cole", "ada@verify.test").await;
        request_magic(&sdk, &user.email, &mail_origin())
            .await
            .unwrap()
            .unwrap();
        let text = latest_mail_text(&sdk).await;
        assert!(text.contains("http://127.0.0.1:43781/session/link/"));
        let (id, secret) = href_parts(&text);
        consume_magic(&sdk, &id, &secret, &test_device())
            .await
            .unwrap()
            .unwrap();
        let again = consume_magic(&sdk, &id, &secret, &test_device())
            .await
            .unwrap()
            .expect_err("spent");
        assert_eq!(again, DomainError::NotFound);
    }

    #[tokio::test]
    async fn us_mail_08_reset_mints_one_session() {
        let sdk = fresh_sdk().await;
        let user = register_named(&sdk, "Ada Cole", "ada@verify.test").await;
        sign_in(
            &sdk,
            "ada@verify.test",
            "Thursday dinners at six oclock",
            &test_device(),
        )
        .await
        .unwrap()
        .unwrap();
        request_reset(&sdk, &user.email, &mail_origin())
            .await
            .unwrap()
            .unwrap();
        let text = latest_mail_text(&sdk).await;
        assert!(text.contains("http://127.0.0.1:43781/session/reset/"));
        let (id, secret) = href_parts(&text);
        assert!(reset_form_ok(&sdk, &id, &secret).await.unwrap());
        let ok = complete_reset(
            &sdk,
            &id,
            &secret,
            "Sunday supper after church",
            &test_device(),
        )
        .await
        .unwrap()
        .unwrap();
        let live = sdk.db.sessions_for_user(&user.id).await.unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].id, ok.session_id.expect("reset session"));
        assert!(!reset_form_ok(&sdk, &id, &secret).await.unwrap());
    }

    #[tokio::test]
    async fn us_read_04_home_keeps_twenty_and_cursors_the_last_visible() {
        let sdk = fresh_sdk().await;
        let user = register_named(&sdk, "Miriam Cole", "miriam@grace.test").await;
        let planted = plant_church(
            &sdk,
            &user,
            "Grace Covenant",
            "100 Main Street",
            42.53,
            -92.45,
            "A church on Main Street.",
            "Sunday at 10.",
        )
        .await
        .unwrap()
        .unwrap();
        let church_id = planted.church_id.expect("church");
        let mut effect = Effect::write(Write::InsertNeed(Need {
            id: "need_extra_00".into(),
            church_id: church_id.clone(),
            author_id: user.id.clone(),
            title: "Extra dinner 00".into(),
            body: "Side door after 5.".into(),
            gift_id: None,
            scope: "church".into(),
            status: "open".into(),
            created_at: "2026-08-01T00:00:00Z".into(),
        }));
        for index in 1..21 {
            effect.push(Write::InsertNeed(Need {
                id: format!("need_extra_{index:02}"),
                church_id: church_id.clone(),
                author_id: user.id.clone(),
                title: format!("Extra dinner {index:02}"),
                body: "Side door after 5.".into(),
                gift_id: None,
                scope: "church".into(),
                status: "open".into(),
                created_at: format!("2026-08-{:02}T00:00:00Z", index + 1),
            }));
        }
        sdk.db.apply(&effect).await.unwrap();
        let user = sdk.db.user(&user.id).await.unwrap().expect("planter");
        let viewer = sdk.viewer(user).await.unwrap();
        let page = home_needs(&sdk, &viewer, None).await.unwrap();
        assert_eq!(page.cards.len(), 20);
        let cursor = page.next_cursor.expect("full page has a cursor");
        assert!(cursor.contains('|'));
        let last = page.cards.last().expect("last card");
        assert_eq!(cursor, format!("{}|{}", last.created_at, last.id));
        let next = home_needs(&sdk, &viewer, Some(&cursor)).await.unwrap();
        assert!(!next.cards.is_empty());
        assert!(next.cards.len() < 20);
        assert!(next.next_cursor.is_none());
    }

    #[tokio::test]
    async fn us_read_02_directory_pages_twenty_churches_then_the_rest() {
        let sdk = fresh_sdk().await;
        let mut effect = Effect::write(Write::InsertChurch(Church {
            id: "church_page_00".into(),
            name: "Paging Church 00".into(),
            address: "100 Alpha Street".into(),
            latitude: 42.53,
            longitude: -92.45,
            country: "US".into(),
            description: "Paging.".into(),
            gathering: String::new(),
            owner_id: "user_page".into(),
            invite_code: "page-00".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
        }));
        for index in 1..22 {
            effect.push(Write::InsertChurch(Church {
                id: format!("church_page_{index:02}"),
                name: format!("Paging Church {index:02}"),
                address: "100 Alpha Street".into(),
                latitude: 42.53,
                longitude: -92.45,
                country: "US".into(),
                description: "Paging.".into(),
                gathering: String::new(),
                owner_id: "user_page".into(),
                invite_code: format!("page-{index:02}"),
                created_at: format!("2026-01-{:02}T00:00:00Z", index + 1),
            }));
        }
        sdk.db.apply(&effect).await.unwrap();
        let first = church_directory(&sdk, None).await.unwrap();
        assert_eq!(first.cards.len(), 20);
        let cursor = first.next_cursor.expect("full directory page");
        assert!(cursor.contains('|'));
        let next = church_directory(&sdk, Some(&cursor)).await.unwrap();
        assert!(!next.cards.is_empty());
        assert!(next.cards.len() < 20);
    }

    #[tokio::test]
    async fn us_read_03_church_show_pages_members_and_needs() {
        let sdk = fresh_sdk().await;
        let user = register_named(&sdk, "Miriam Cole", "miriam@grace.test").await;
        let planted = plant_church(
            &sdk,
            &user,
            "Grace Covenant",
            "100 Main Street",
            42.53,
            -92.45,
            "A church on Main Street.",
            "Sunday at 10.",
        )
        .await
        .unwrap()
        .unwrap();
        let church_id = planted.church_id.expect("church");
        let mut effect = Effect {
            writes: Vec::new(),
            notices: Vec::new(),
        };
        for index in 0..20 {
            let user_id = format!("user_page_{index:02}");
            effect.push(Write::InsertUser(User {
                id: user_id,
                first_name: "Page".into(),
                last_name: format!("Member {index:02}"),
                email: format!("page{index:02}@grace.test"),
                bio: String::new(),
                created_at: format!("2026-02-{:02}T00:00:00Z", index + 1),
                church_id: Some(church_id.clone()),
                church_status: Some("active".into()),
                church_role: Some("member".into()),
            }));
        }
        for index in 0..21 {
            effect.push(Write::InsertNeed(Need {
                id: format!("need_page_{index:02}"),
                church_id: church_id.clone(),
                author_id: user.id.clone(),
                title: format!("Paging dinner {index:02}"),
                body: "Side door after 5.".into(),
                gift_id: None,
                scope: "church".into(),
                status: "open".into(),
                created_at: format!("2026-08-{:02}T00:00:00Z", index + 1),
            }));
        }
        sdk.db.apply(&effect).await.unwrap();
        let first = church_show(&sdk, &church_id, None, None)
            .await
            .unwrap()
            .expect("church");
        assert_eq!(first.members.len(), 20);
        assert_eq!(first.needs.len(), 20);
        let members_after = first.next_member_cursor.expect("more people");
        let needs_after = first.next_need_cursor.expect("more needs");
        let next = church_show(&sdk, &church_id, Some(&needs_after), Some(&members_after))
            .await
            .unwrap()
            .expect("church page 2");
        assert!(!next.members.is_empty());
        assert!(next.members.len() < 20);
        assert!(!next.needs.is_empty());
        assert!(next.needs.len() < 20);
    }
}

pub enum PrayerName {
    Signed,
    Unnamed,
}

pub struct PrayerPost {
    pub prayer_id: String,
    pub manage_token: Option<String>,
}

pub enum PrayerDeck {
    Card {
        card: PrayerCard,
        source: PrayerSource,
    },
    SharePlace,
    Finished,
}

pub struct NearbyFeed {
    pub needs: Vec<NeedCard>,
    pub prayers: Vec<PrayerCard>,
}

pub async fn post_prayer(
    sdk: &Sdk,
    viewer: &Viewer,
    church_id: &str,
    body: &str,
    name: PrayerName,
) -> anyhow::Result<Result<PrayerPost, DomainError>> {
    let (byline, manage_token) = match name {
        PrayerName::Signed => (PrayerByline::Signed, None),
        PrayerName::Unnamed => {
            let token = crate::password::mint_token();
            let manage_hash = crate::password::hash_token(&token);
            (
                PrayerByline::Unnamed { manage_hash },
                Some(token),
            )
        }
    };
    let posture = sdk.weigh(VoiceKind::Prayer, &[body]).await;
    let effect = domain_post_prayer(
        viewer,
        church_id,
        body,
        byline,
        posture,
        new_id(),
        now_iso(),
    );
    match effect {
        Ok(effect) => {
            let prayer_id = effect
                .writes
                .iter()
                .find_map(|write| match write {
                    ecclesia_domain::Write::InsertPrayer(prayer) => Some(prayer.id.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            if !effect.writes.is_empty() {
                sdk.commit(&effect).await?;
            }
            Ok(Ok(PrayerPost {
                prayer_id,
                manage_token,
            }))
        }
        Err(error) => Ok(Err(error)),
    }
}

pub async fn prayer_deck(
    sdk: &Sdk,
    viewer: &Viewer,
    place: Option<Place>,
) -> anyhow::Result<PrayerDeck> {
    let day = today_utc();
    let church_id = viewer.active_church().map(|church| church.id.as_str());
    let located = sdk
        .db
        .open_prayers_for_deck(church_id, place.map(|point| (point.latitude, point.longitude)))
        .await?;
    let seen = sdk.db.seen_prayer_ids(&viewer.user.id, &day).await?;
    let seen_ids: Vec<&str> = seen.iter().map(String::as_str).collect();
    let sights: Vec<PrayerSight<'_>> = located
        .iter()
        .map(|(prayer, latitude, longitude)| PrayerSight {
            prayer,
            latitude: *latitude,
            longitude: *longitude,
        })
        .collect();
    let Some((prayer, source)) = pick_daily_prayer(viewer, &day, place, &sights, &seen_ids) else {
        return Ok(if place.is_none() {
            PrayerDeck::SharePlace
        } else {
            PrayerDeck::Finished
        });
    };
    let Some(card) = sdk.db.prayer_card(&prayer.id).await? else {
        return Ok(PrayerDeck::Finished);
    };
    Ok(PrayerDeck::Card { card, source })
}

pub async fn mark_prayer(
    sdk: &Sdk,
    viewer: &Viewer,
    prayer_id: &str,
    place: Option<Place>,
    kind: PrayerMarkKind,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some(prayer) = sdk.db.prayer(prayer_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    let Some(church) = sdk.db.church(&prayer.church_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    let reach = if viewer.is_active_in(&prayer.church_id) {
        PrayerReach::HomeChurch
    } else {
        let Some(place) = place else {
            return Ok(Err(DomainError::OutsideNeighborhood));
        };
        PrayerReach::Near(place)
    };
    let day = today_utc();
    let prior = PriorPrayerMark::of_kind(
        sdk.db
            .prayer_mark_kind(&viewer.user.id, prayer_id, &day)
            .await?
            .as_deref(),
    );
    finish(
        sdk,
        domain_mark_prayer(viewer, &prayer, &church, reach, &day, kind, prior),
    )
    .await
}

pub async fn answer_prayer(
    sdk: &Sdk,
    viewer: &Viewer,
    prayer_id: &str,
    praise: &str,
    proof: PrayerProof,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some(prayer) = sdk.db.prayer(prayer_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    let posture = sdk.weigh(VoiceKind::Prayer, &[praise]).await;
    finish(
        sdk,
        domain_answer_prayer(viewer, &prayer, praise, proof, posture, now_iso()),
    )
    .await
}

pub async fn reply_to_need(
    sdk: &Sdk,
    viewer: &Viewer,
    need_id: &str,
    body: &str,
    place: Option<Place>,
) -> anyhow::Result<Result<StoryOk, DomainError>> {
    let Some(need) = sdk.db.need(need_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    let Some(church) = sdk.db.church(&need.church_id).await? else {
        return Ok(Err(DomainError::NotFound));
    };
    let approach = if ecclesia_domain::can_view_need(viewer, need.sight(), &church) {
        NeedApproach::Membership
    } else if let Some(place) = place {
        NeedApproach::Near(place)
    } else {
        NeedApproach::Membership
    };
    let posture = sdk.weigh(VoiceKind::Reply, &[body]).await;
    finish(
        sdk,
        domain_reply_to_need(
            viewer,
            &need,
            &church,
            approach,
            body,
            posture,
            new_id(),
            now_iso(),
        ),
    )
    .await
}

pub async fn nearby_feed(
    sdk: &Sdk,
    viewer: &Viewer,
    place: Place,
) -> anyhow::Result<NearbyFeed> {
    let needs = sdk.db.needs_near(place.latitude, place.longitude).await?;
    let prayers = sdk
        .db
        .prayers_near(place.latitude, place.longitude)
        .await?;
    let mut ids: Vec<&str> = unique_church_ids(&needs);
    for card in &prayers {
        if !ids.contains(&card.church_id.as_str()) {
            ids.push(card.church_id.as_str());
        }
    }
    let churches = sdk.db.churches_with_ids(&ids).await?;
    let day = today_utc();
    let seen = sdk.db.seen_prayer_ids(&viewer.user.id, &day).await?;
    let seen_ids: Vec<&str> = seen.iter().map(String::as_str).collect();
    let mut kept_needs = Vec::new();
    for card in visible_needs_near(viewer, &needs, &churches, place) {
        kept_needs.push(card.clone());
    }
    let mut kept_prayers = Vec::new();
    for card in visible_prayers_near(viewer, &prayers, &churches, place, &seen_ids) {
        kept_prayers.push(card.clone());
    }
    Ok(NearbyFeed {
        needs: kept_needs,
        prayers: kept_prayers,
    })
}

pub async fn answered_prayers(sdk: &Sdk, church_id: &str) -> anyhow::Result<Vec<PrayerCard>> {
    sdk.db.answered_prayers(church_id).await
}

async fn load_application_need(
    sdk: &Sdk,
    application_id: &str,
) -> anyhow::Result<Option<(ecclesia_domain::Need, ecclesia_domain::Application)>> {
    let Some(application) = sdk.db.application(application_id).await? else {
        return Ok(None);
    };
    let Some(need) = sdk.db.need(&application.need_id).await? else {
        return Ok(None);
    };
    Ok(Some((need, application)))
}
