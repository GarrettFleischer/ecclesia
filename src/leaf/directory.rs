use super::model::Church;

pub fn count_for(counts: &[(String, i64, i64)], church_id: &str) -> (i64, i64) {
    counts
        .iter()
        .find(|(id, _, _)| id == church_id)
        .map(|(_, members, needs)| (*members, *needs))
        .unwrap_or((0, 0))
}

pub fn churches_with_counts(
    churches: Vec<Church>,
    counts: &[(String, i64, i64)],
) -> Vec<(Church, i64, i64)> {
    churches
        .into_iter()
        .map(|church| {
            let (members, needs) = count_for(counts, &church.id);
            (church, members, needs)
        })
        .collect()
}

pub fn group_churches_by_place(
    cards: Vec<(Church, i64, i64)>,
) -> Vec<(String, Vec<(Church, i64, i64)>)> {
    let mut groups = Vec::new();
    for card in cards {
        push_card_into_place(&mut groups, card);
    }
    groups
}

fn push_card_into_place(
    groups: &mut Vec<(String, Vec<(Church, i64, i64)>)>,
    card: (Church, i64, i64),
) {
    let place = format!("{}, {}", card.0.city, card.0.region);
    if let Some((_, list)) = groups.iter_mut().find(|(name, _)| *name == place) {
        list.push(card);
        return;
    }
    groups.push((place, vec![card]));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn church(id: &str, city: &str, region: &str) -> Church {
        Church {
            id: id.into(),
            name: id.into(),
            city: city.into(),
            region: region.into(),
            country: "US".into(),
            description: String::new(),
            gathering: String::new(),
            owner_id: "o".into(),
            invite_code: "c".into(),
            created_at: "t".into(),
        }
    }

    #[test]
    fn us_body_01_groups_the_valley_by_city() {
        let cards = churches_with_counts(
            vec![
                church("grace", "Cedar Falls", "Iowa"),
                church("luke", "Cedar Falls", "Iowa"),
                church("mercy", "Waterloo", "Iowa"),
            ],
            &[
                ("grace".into(), 3, 2),
                ("luke".into(), 2, 1),
                ("mercy".into(), 2, 1),
            ],
        );
        let groups = group_churches_by_place(cards);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, "Cedar Falls, Iowa");
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].0, "Waterloo, Iowa");
    }
}
