//! Rank churches by fuzzy name match for registration lookup.

use ecclesia_domain::Church;

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

fn score_church(needle: &str, church: &Church) -> Option<i32> {
    let name = church.name.to_lowercase();
    let city = church.city.to_lowercase();
    let region = church.region.to_lowercase();
    let hay = format!("{name} {city} {region}");
    let mut best = field_score(needle, &name);
    best = best.max(field_score(needle, &city) / 2);
    best = best.max(field_score(needle, &hay) / 2);
    if best > 0 {
        Some(best)
    } else {
        None
    }
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

    fn church(name: &str, city: &str) -> Church {
        Church {
            id: name.into(),
            name: name.into(),
            city: city.into(),
            region: "Iowa".into(),
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
