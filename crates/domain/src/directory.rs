use super::model::Church;

pub type ChurchCard = (Church, i64, i64);
pub type PlaceGroup = Vec<ChurchCard>;

pub fn count_for(counts: &[(String, i64, i64)], church_id: &str) -> (i64, i64) {
    counts
        .iter()
        .find(|(id, _, _)| id == church_id)
        .map(|(_, members, needs)| (*members, *needs))
        .unwrap_or((0, 0))
}

pub fn churches_with_counts(
    churches: impl IntoIterator<Item = Church>,
    counts: &[(String, i64, i64)],
) -> Vec<ChurchCard> {
    churches
        .into_iter()
        .map(|church| {
            let (members, needs) = count_for(counts, &church.id);
            (church, members, needs)
        })
        .collect()
}

pub fn group_churches_by_place(cards: impl IntoIterator<Item = ChurchCard>) -> Vec<PlaceGroup> {
    let mut groups = Vec::new();
    for card in cards {
        push_card_into_place(&mut groups, card);
    }
    groups
}

fn push_card_into_place(groups: &mut Vec<PlaceGroup>, card: ChurchCard) {
    if let Some(list) = groups
        .iter_mut()
        .find(|list| same_place(&list[0].0, &card.0))
    {
        list.push(card);
        return;
    }
    groups.push(vec![card]);
}

fn same_place(a: &Church, b: &Church) -> bool {
    a.city == b.city && a.region == b.region
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
            [
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
        assert_eq!(groups[0][0].0.city, "Cedar Falls");
        assert_eq!(groups[0].len(), 2);
        assert_eq!(groups[1][0].0.city, "Waterloo");
    }
}
