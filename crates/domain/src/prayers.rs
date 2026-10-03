//! Prayer requests. A signed prayer keeps an author. An unnamed prayer does not.

use super::effect::{DomainError, Effect, Write};
use super::flags::{Posture, require_uplifting};
use super::geo::{Place, place_is_near};
use super::household::Church;
use super::need::{Prayer, PrayerCard, PrayerStatus};
use super::person::Viewer;
use super::validate::{note_field, prayer_body};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrayerByline {
    Signed,
    Unnamed { manage_hash: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrayerProof {
    Author,
    Token,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrayerMarkKind {
    Seen,
    Prayed,
}

impl PrayerMarkKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Seen => "seen",
            Self::Prayed => "prayed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "seen" => Some(Self::Seen),
            "prayed" => Some(Self::Prayed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorPrayerMark {
    None,
    Seen,
    Prayed,
}

impl PriorPrayerMark {
    pub fn of_kind(kind: Option<&str>) -> Self {
        match kind.and_then(PrayerMarkKind::parse) {
            Some(PrayerMarkKind::Prayed) => Self::Prayed,
            Some(PrayerMarkKind::Seen) => Self::Seen,
            None => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrayerReach {
    HomeChurch,
    Near(Place),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrayerSource {
    Church,
    Nearby,
}

pub struct PrayerSight<'a> {
    pub prayer: &'a Prayer,
    pub latitude: f64,
    pub longitude: f64,
}

pub fn post_prayer(
    viewer: &Viewer,
    church_id: &str,
    body: &str,
    byline: PrayerByline,
    posture: Posture,
    id: String,
    now: String,
) -> Result<Effect, DomainError> {
    require_uplifting(posture)?;
    if !viewer.is_active_in(church_id) {
        return Err(DomainError::NotInTheBody);
    }
    let body = prayer_body(body)?;
    let (author_id, manage_hash) = byline_fields(viewer, byline)?;
    Ok(Effect::write(Write::InsertPrayer(Prayer {
        id,
        church_id: church_id.into(),
        author_id,
        body,
        status: PrayerStatus::Open.as_str().into(),
        praise: None,
        manage_hash,
        created_at: now,
        answered_at: None,
    })))
}

fn byline_fields(
    viewer: &Viewer,
    byline: PrayerByline,
) -> Result<(Option<String>, Option<String>), DomainError> {
    match byline {
        PrayerByline::Signed => Ok((Some(viewer.user.id.clone()), None)),
        PrayerByline::Unnamed { manage_hash } => {
            let manage_hash = note_field(&manage_hash)?;
            Ok((None, Some(manage_hash)))
        }
    }
}

pub fn mark_prayer(
    viewer: &Viewer,
    prayer: &Prayer,
    church: &Church,
    reach: PrayerReach,
    day: &str,
    kind: PrayerMarkKind,
    prior: PriorPrayerMark,
) -> Result<Effect, DomainError> {
    if !prayer.is_open() {
        return Err(DomainError::PrayerAnswered);
    }
    if prayer.author_id.as_deref() == Some(viewer.user.id.as_str()) {
        return Err(DomainError::SelfAction);
    }
    require_prayer_reach(viewer, prayer, church, reach)?;
    let day = note_field(day)?;
    if !writes_mark(prior, kind) {
        return Ok(Effect::default());
    }
    Ok(Effect::write(Write::UpsertPrayerMark {
        user_id: viewer.user.id.clone(),
        prayer_id: prayer.id.clone(),
        day,
        kind: kind.as_str(),
    }))
}

fn writes_mark(prior: PriorPrayerMark, kind: PrayerMarkKind) -> bool {
    match (prior, kind) {
        (PriorPrayerMark::None, _) => true,
        (PriorPrayerMark::Seen, PrayerMarkKind::Prayed) => true,
        (PriorPrayerMark::Seen, PrayerMarkKind::Seen) => false,
        (PriorPrayerMark::Prayed, _) => false,
    }
}

fn require_prayer_reach(
    viewer: &Viewer,
    prayer: &Prayer,
    church: &Church,
    reach: PrayerReach,
) -> Result<(), DomainError> {
    if church.id != prayer.church_id {
        return Err(DomainError::NotFound);
    }
    match reach {
        PrayerReach::HomeChurch => {
            if viewer.is_active_in(&prayer.church_id) {
                Ok(())
            } else {
                Err(DomainError::NotInTheBody)
            }
        }
        PrayerReach::Near(place) => {
            if viewer.is_active_anywhere() && place_is_near(church.latitude, church.longitude, place)
            {
                Ok(())
            } else {
                Err(DomainError::OutsideNeighborhood)
            }
        }
    }
}

pub fn answer_prayer(
    viewer: &Viewer,
    prayer: &Prayer,
    praise: &str,
    proof: PrayerProof,
    posture: Posture,
    answered_at: String,
) -> Result<Effect, DomainError> {
    require_uplifting(posture)?;
    if !prayer.is_open() {
        return Err(DomainError::PrayerAnswered);
    }
    require_answer_proof(viewer, prayer, proof)?;
    let praise = note_field(praise)?;
    Ok(Effect::write(Write::SetPrayerAnswered {
        id: prayer.id.clone(),
        praise,
        answered_at,
    }))
}

fn require_answer_proof(
    viewer: &Viewer,
    prayer: &Prayer,
    proof: PrayerProof,
) -> Result<(), DomainError> {
    match (prayer.author_id.as_deref(), proof) {
        (Some(author_id), PrayerProof::Author) if author_id == viewer.user.id => Ok(()),
        (None, PrayerProof::Token) => Ok(()),
        _ => Err(DomainError::NotAuthor),
    }
}

pub fn visible_prayers_near<'a>(
    viewer: &'a Viewer,
    cards: &'a [PrayerCard],
    churches: &'a [Church],
    place: Place,
    seen: &'a [&str],
) -> impl Iterator<Item = &'a PrayerCard> + 'a {
    cards.iter().filter(move |card| {
        card.is_open()
            && viewer.is_active_anywhere()
            && !seen.iter().any(|id| *id == card.id.as_str())
            && churches.iter().any(|church| {
                church.id == card.church_id
                    && place_is_near(church.latitude, church.longitude, place)
            })
    })
}

pub fn pick_daily_prayer<'a>(
    viewer: &Viewer,
    day: &str,
    place: Option<Place>,
    prayers: &'a [PrayerSight<'a>],
    seen: &[&str],
) -> Option<(&'a Prayer, PrayerSource)> {
    let church_id = viewer.active_church().map(|church| church.id.as_str());
    if let Some(prayer) = first_ranked(viewer, day, seen, prayers, |sight| {
        church_id == Some(sight.prayer.church_id.as_str())
    }) {
        return Some((prayer, PrayerSource::Church));
    }
    let place = place?;
    first_ranked(viewer, day, seen, prayers, |sight| {
        church_id != Some(sight.prayer.church_id.as_str())
            && place_is_near(sight.latitude, sight.longitude, place)
    })
    .map(|prayer| (prayer, PrayerSource::Nearby))
}

fn first_ranked<'a>(
    viewer: &Viewer,
    day: &str,
    seen: &[&str],
    prayers: &'a [PrayerSight<'a>],
    keep: impl Fn(&PrayerSight<'a>) -> bool,
) -> Option<&'a Prayer> {
    let mut best: Option<&PrayerSight<'a>> = None;
    for sight in prayers {
        if !keep(sight) || !eligible(viewer, sight.prayer, seen) {
            continue;
        }
        best = Some(match best {
            Some(current) if rank(viewer, day, current.prayer) <= rank(viewer, day, sight.prayer) => {
                current
            }
            _ => sight,
        });
    }
    best.map(|sight| sight.prayer)
}

fn eligible(viewer: &Viewer, prayer: &Prayer, seen: &[&str]) -> bool {
    prayer.is_open()
        && prayer.author_id.as_deref() != Some(viewer.user.id.as_str())
        && !seen.iter().any(|id| *id == prayer.id)
}

fn rank(viewer: &Viewer, day: &str, prayer: &Prayer) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    feed(&mut hash, &viewer.user.id);
    feed(&mut hash, day);
    feed(&mut hash, &prayer.id);
    hash
}

fn feed(hash: &mut u64, value: &str) {
    for byte in value.bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::{church, church_at, user_in_church, viewer_of};

    fn card(id: &str, church_id: &str) -> PrayerCard {
        PrayerCard {
            id: id.into(),
            church_id: church_id.into(),
            church_name: "Grace".into(),
            author_id: Some("miriam".into()),
            author_name: Some("Miriam Cole".into()),
            body: "Pray for the surgery on Thursday.".into(),
            status: "open".into(),
            praise: None,
            prayed_count: 0,
            created_at: "t".into(),
        }
    }

    fn prayer(id: &str, church_id: &str, author_id: Option<&str>) -> Prayer {
        Prayer {
            id: id.into(),
            church_id: church_id.into(),
            author_id: author_id.map(str::to_string),
            body: "Pray for the surgery on Thursday.".into(),
            status: "open".into(),
            praise: None,
            manage_hash: None,
            created_at: "t".into(),
            answered_at: None,
        }
    }

    fn sight<'a>(prayer: &'a Prayer, church: &Church) -> PrayerSight<'a> {
        PrayerSight {
            prayer,
            latitude: church.latitude,
            longitude: church.longitude,
        }
    }

    #[test]
    fn signed_prayer_keeps_the_author() {
        let viewer = viewer_of(
            user_in_church("miriam", "grace", "owner", "active"),
            Some(church("grace")),
        );
        let effect = post_prayer(
            &viewer,
            "grace",
            "Pray for the surgery on Thursday.",
            PrayerByline::Signed,
            Posture::Lifts,
            "p1".into(),
            "t".into(),
        )
        .unwrap();
        let Write::InsertPrayer(saved) = &effect.writes[0] else {
            panic!("insert");
        };
        assert_eq!(saved.author_id.as_deref(), Some("miriam"));
        assert!(saved.manage_hash.is_none());
    }

    #[test]
    fn unnamed_prayer_stores_no_author() {
        let viewer = viewer_of(
            user_in_church("miriam", "grace", "owner", "active"),
            Some(church("grace")),
        );
        let effect = post_prayer(
            &viewer,
            "grace",
            "Pray for the surgery on Thursday.",
            PrayerByline::Unnamed {
                manage_hash: "abc123".into(),
            },
            Posture::Lifts,
            "p1".into(),
            "t".into(),
        )
        .unwrap();
        let Write::InsertPrayer(saved) = &effect.writes[0] else {
            panic!("insert");
        };
        assert!(saved.author_id.is_none());
        assert_eq!(saved.manage_hash.as_deref(), Some("abc123"));
    }

    #[test]
    fn church_prayers_come_before_the_shared_point() {
        let home = church("grace");
        let viewer = viewer_of(
            user_in_church("elena", "grace", "member", "active"),
            Some(home.clone()),
        );
        let own = prayer("own", "grace", Some("elena"));
        let church_prayer = prayer("church", "grace", Some("miriam"));
        let near = prayer("near", "mercy", Some("james"));
        let far_church = church_at("far", 30.2672, -97.7431);
        let far = prayer("far", "far", Some("ada"));
        let mercy = church_at("mercy", 42.4928, -92.3426);
        let sights = [
            sight(&own, &home),
            sight(&church_prayer, &home),
            sight(&near, &mercy),
            sight(&far, &far_church),
        ];
        let place = Place {
            latitude: home.latitude,
            longitude: home.longitude,
        };
        let (picked, source) =
            pick_daily_prayer(&viewer, "2026-10-02", Some(place), &sights, &[]).unwrap();
        assert_eq!(picked.id, "church");
        assert_eq!(source, PrayerSource::Church);
        let (picked, source) =
            pick_daily_prayer(&viewer, "2026-10-02", Some(place), &sights, &["church"]).unwrap();
        assert_eq!(picked.id, "near");
        assert_eq!(source, PrayerSource::Nearby);
        assert!(
            pick_daily_prayer(&viewer, "2026-10-02", None, &sights, &["church"]).is_none()
        );
    }

    #[test]
    fn a_marked_prayer_stays_off_nearby_for_the_day() {
        let grace = church("grace");
        let elena = viewer_of(
            user_in_church("elena", "grace", "member", "active"),
            Some(grace.clone()),
        );
        let cards = [card("church", "grace"), card("near", "mercy")];
        let churches = [grace.clone(), church_at("mercy", 42.4928, -92.3426)];
        let place = Place {
            latitude: grace.latitude,
            longitude: grace.longitude,
        };
        let mut visible = Vec::new();
        for prayer in visible_prayers_near(&elena, &cards, &churches, place, &["church"]) {
            visible.push(prayer.id.as_str());
        }
        assert_eq!(visible, ["near"]);
    }

    #[test]
    fn a_seen_mark_does_not_downgrade_a_prayer() {
        let viewer = viewer_of(
            user_in_church("elena", "grace", "member", "active"),
            Some(church("grace")),
        );
        let prayer = prayer("church", "grace", Some("miriam"));
        let effect = mark_prayer(
            &viewer,
            &prayer,
            &church("grace"),
            PrayerReach::HomeChurch,
            "2026-10-02",
            PrayerMarkKind::Seen,
            PriorPrayerMark::Prayed,
        )
        .unwrap();
        assert!(effect.writes.is_empty());
    }

    #[test]
    fn token_answers_an_unnamed_prayer() {
        let viewer = viewer_of(
            user_in_church("miriam", "grace", "owner", "active"),
            Some(church("grace")),
        );
        let mut prayer = prayer("p", "grace", None);
        prayer.manage_hash = Some("hash".into());
        let effect = answer_prayer(
            &viewer,
            &prayer,
            "The surgery went well.",
            PrayerProof::Token,
            Posture::Lifts,
            "t2".into(),
        )
        .unwrap();
        assert!(matches!(effect.writes[0], Write::SetPrayerAnswered { .. }));
        assert_eq!(
            answer_prayer(
                &viewer,
                &prayer,
                "The surgery went well.",
                PrayerProof::Author,
                Posture::Lifts,
                "t2".into(),
            ),
            Err(DomainError::NotAuthor)
        );
    }
}
