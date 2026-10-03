//! Rank churches by name or city for the find-church step.

use ecclesia_domain::{Church, Place, place_is_near};

const MAX_QUERY_LEN: usize = 80;

pub fn rank_churches<'a>(query: &str, churches: &'a [Church], limit: usize) -> Vec<&'a Church> {
    let needle = normalize_query(query);
    if needle.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut scored: Vec<(i32, &Church)> = churches
        .iter()
        .filter_map(|church| score_church(&needle, church).map(|score| (score, church)))
        .collect();
    scored.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.name.cmp(&right.1.name)));
    scored.into_iter().take(limit).map(|(_, church)| church).collect()
}

fn normalize_query(query: &str) -> String {
    query.trim().to_lowercase().chars().take(MAX_QUERY_LEN).collect()
}

/// Churches inside the nearby radius move ahead of the rest. Rank order stays inside each group.
pub fn prefer_nearby<'a>(ranked: &mut Vec<&'a Church>, here: Place) {
    ranked.sort_by(|left, right| {
        let left_near = place_is_near(left.latitude, left.longitude, here);
        let right_near = place_is_near(right.latitude, right.longitude, here);
        right_near.cmp(&left_near)
    });
}

fn score_church(needle: &str, church: &Church) -> Option<i32> {
    let name = church.name.to_lowercase();
    let address = church.address.to_lowercase();
    let hay = format!("{name} {address}");
    let mut best = field_score(needle, &name);
    for city in city_names(&church.address) {
        best = best.max(field_score(needle, &city));
    }
    best = best.max(field_score(needle, &address) / 2);
    best = best.max(field_score(needle, &hay) / 2);
    if best > 0 {
        Some(best)
    } else {
        None
    }
}

fn city_names(address: &str) -> Vec<String> {
    let mut cities = Vec::new();
    for line in address.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let city = line.split(',').next().unwrap_or(line).trim();
        if city.is_empty() || starts_with_digit(city) {
            continue;
        }
        cities.push(city.to_lowercase());
    }
    cities
}

fn starts_with_digit(value: &str) -> bool {
    value.chars().next().is_some_and(|ch| ch.is_ascii_digit())
}

fn field_score(needle: &str, haystack: &str) -> i32 {
    if haystack.is_empty() || needle.is_empty() {
        return 0;
    }
    if haystack == needle {
        return 10_000;
    }
    if haystack.starts_with(needle) {
        return 5_000 + prefix_bonus(needle.len(), haystack.len());
    }
    if haystack.contains(needle) {
        return 2_000 + prefix_bonus(needle.len(), haystack.len());
    }
    let words: Vec<&str> = haystack.split_whitespace().collect();
    for word in &words {
        if word.starts_with(needle) {
            return 1_500 + prefix_bonus(needle.len(), word.len());
        }
    }
    word_prefix_score(needle, &words)
}

/// Every typed word must be a prefix of some church word, in order.
/// "gra fel" matches "Grace Fellowship".
fn word_prefix_score(needle: &str, words: &[&str]) -> i32 {
    let parts: Vec<&str> = needle.split_whitespace().filter(|part| !part.is_empty()).collect();
    if parts.len() < 2 || words.is_empty() {
        return 0;
    }
    let mut at = 0;
    for part in &parts {
        let found = words[at..]
            .iter()
            .position(|word| word.starts_with(part));
        let Some(found) = found else {
            return 0;
        };
        at += found + 1;
    }
    1_200 + parts.len() as i32 * 40
}

fn prefix_bonus(needle_len: usize, hay_len: usize) -> i32 {
    if hay_len == 0 {
        return 0;
    }
    ((needle_len * 100) / hay_len) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecclesia_domain::Church;

    fn church(name: &str, address: &str) -> Church {
        Church {
            id: name.into(),
            name: name.into(),
            address: address.into(),
            latitude: 42.53,
            longitude: -92.45,
            country: "US".into(),
            description: String::new(),
            gathering: String::new(),
            owner_id: "o".into(),
            invite_code: "code".into(),
            created_at: String::new(),
        }
    }

    #[test]
    fn us_search_01_prefix_wins_over_substring() {
        let churches = [
            church("Grace Covenant", "Cedar Falls"),
            church("New Grace Chapel", "Waterloo"),
        ];
        let hits = rank_churches("grace cov", &churches, 5);
        assert_eq!(hits.first().map(|c| c.name.as_str()), Some("Grace Covenant"));
    }

    #[test]
    fn us_search_01_returns_at_most_five() {
        let churches: Vec<Church> = (0..8)
            .map(|i| church(&format!("Grace Church {i}"), "Cedar Falls"))
            .collect();
        assert_eq!(rank_churches("grace", &churches, 5).len(), 5);
    }

    #[test]
    fn us_search_01_city_matches_the_address() {
        let churches = [
            church("Grace Fellowship", "100 Main Street\nCedar Falls, IA 50613"),
            church("Mercy Chapel", "200 Oak Street\nAustin, TX 78701"),
        ];
        let cedar = rank_churches("cedar falls", &churches, 5);
        assert_eq!(cedar.len(), 1);
        assert_eq!(cedar[0].name, "Grace Fellowship");
        let austin = rank_churches("austin", &churches, 5);
        assert_eq!(austin.first().map(|hit| hit.name.as_str()), Some("Mercy Chapel"));
    }

    #[test]
    fn us_search_01_nearby_matches_come_first() {
        let mut far = church("North Chapel", "200 Oak Street\nAustin, TX 78701");
        far.latitude = 30.2672;
        far.longitude = -97.7431;
        let mut near = church("South Chapel", "100 Main Street\nCedar Falls, IA 50613");
        near.latitude = 42.5349;
        near.longitude = -92.4453;
        let churches = [far, near];
        let mut hits = rank_churches("chapel", &churches, 5);
        assert_eq!(hits[0].name, "North Chapel");
        prefer_nearby(&mut hits, Place { latitude: 42.5349, longitude: -92.4453 });
        assert_eq!(hits[0].name, "South Chapel");
    }

    #[test]
    fn us_search_01_short_prefix_keeps_the_church() {
        let churches = [church("Grace Fellowship", "Cedar Falls")];
        for query in ["g", "gr", "gra", "grace", "grace fel", "grace fellowship"] {
            assert_eq!(
                rank_churches(query, &churches, 5).first().map(|c| c.name.as_str()),
                Some("Grace Fellowship"),
                "{query}"
            );
        }
    }
}
