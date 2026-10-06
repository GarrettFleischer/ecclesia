//! Repeated cards and option lists. Loops live here, not inside page bodies.

use maud::{Markup, html};

use ecclesia_sdk::db::ClosedNeedGroup;
use ecclesia_sdk::prelude::{
    Church, ChurchCard, ChurchLinkStatus, ChurchMember, EndorsementCard, Gift, MemberGift,
    MembershipRole, NeedCard, NeedScope, NeedStatus, Notification, PlaceGroup, Viewer,
    display_name,
};

use super::layout::{Icon, Monogram, csrf_input, icon, monogram};
use super::words::{category_label, census_line, household_line, role_word};

#[derive(Clone, Copy)]
pub enum NeedCardPlace {
    Feed,
    Church,
}

pub fn need_card(need: &NeedCard, viewer: &Viewer, place: NeedCardPlace) -> Markup {
    html! {
        a id={ "need-" (need.id) } class="card card-link need-card" href={ "/needs/" (need.id) } data-need=(need.id) {
            div class="card-top" {
                (scope_mark(&need.scope))
                p class="eyebrow" { (need_card_eyebrow(need, place)) }
            }
            h3 { (need.title) }
            p class="clamp" { (need.body) }
            div class="card-foot" {
                span class="byline" {
                    (monogram(&need.author_id, &need.author_name, Monogram::PersonSmall))
                    span { (need.author_name) }
                }
                @if let Some(gift) = &need.gift_name { span class="chip" { (gift) } }
                @if let Some(gift_id) = &need.gift_id {
                    @if viewer.has_gift(gift_id) { span class="chip chip-accent" { "Your gift" } }
                }
                @if need.status() == Some(NeedStatus::Closed) {
                    span class="chip chip-closed" { "Closed" }
                }
            }
        }
    }
}

fn need_card_eyebrow(need: &NeedCard, place: NeedCardPlace) -> String {
    match place {
        NeedCardPlace::Feed => {
            format!(
                "{} · {} · {}",
                scope_label(&need.scope),
                need.church_name,
                need.church_address
            )
        }
        NeedCardPlace::Church => scope_label(&need.scope).to_string(),
    }
}

pub fn scope_label(scope: &str) -> &'static str {
    NeedScope::parse(scope)
        .map(NeedScope::label)
        .unwrap_or("Need")
}

pub fn scope_mark(scope: &str) -> Markup {
    let (class, glyph) = match NeedScope::parse(scope) {
        Some(NeedScope::Neighboring) => ("scope-mark scope-neighboring", Icon::Pin),
        Some(NeedScope::Body) => ("scope-mark scope-body", Icon::Globe),
        Some(NeedScope::Church) | None => ("scope-mark scope-church", Icon::Church),
    };
    html! {
        span class=(class) aria-hidden="true" { (icon(glyph)) }
    }
}

pub fn need_card_stack<'a>(
    needs: impl IntoIterator<Item = &'a NeedCard>,
    viewer: &Viewer,
    place: NeedCardPlace,
) -> Markup {
    html! {
        div class="stack" {
            @for need in needs {
                (need_card(need, viewer, place))
            }
        }
    }
}

pub fn waiting_church_card(
    church: &Church,
    status: Option<ChurchLinkStatus>,
    csrf: &str,
) -> Markup {
    let asked = status == Some(ChurchLinkStatus::Pending);
    html! {
        article class="card" {
            div class="person-line" {
                (monogram(&church.id, &church.name, Monogram::ChurchSmall))
                div {
                    @if asked {
                        p {
                            "Your request to join "
                            a href={ "/churches/" (church.id) } { (church.name) }
                            " has been sent."
                        }
                    } @else {
                        p { (church.name) " invited you." }
                    }
                }
            }
            @if !asked {
                form method="post" action="/churches/join/accept" {
                    (csrf_input(csrf))
                    input type="hidden" name="church_id" value=(church.id);
                    button class="btn" type="submit" { "Accept invite" }
                }
            }
        }
    }
}

pub fn church_index_cards(churches: &[ChurchCard]) -> Markup {
    html! {
        @for card in churches {
            (church_index_card(card))
        }
    }
}

fn church_index_card(card: &ChurchCard) -> Markup {
    let (church, members, needs) = card;
    html! {
        a class="card card-link church-card" href={ "/churches/" (church.id) } {
            (monogram(&church.id, &church.name, Monogram::Church))
            div {
                h3 { (church.name) }
                p class="muted" { (church.address) }
                p class="clamp-2" { (church.description) }
                p class="meta" { (census_line(*members, *needs)) }
            }
        }
    }
}

pub fn pending_member_cards<'a>(
    church_id: &str,
    members: impl IntoIterator<Item = &'a ChurchMember>,
    csrf: &str,
) -> Markup {
    html! {
        @for member in members {
            (pending_member_card(church_id, member, csrf))
        }
    }
}

fn pending_member_card(church_id: &str, member: &ChurchMember, csrf: &str) -> Markup {
    let name = member_name(member);
    html! {
        article class="card" {
            div class="person-line" {
                (monogram(&member.user_id, &name, Monogram::Person))
                div {
                    a href={ "/members/" (member.user_id) } { strong { (name) } }
                    p class="muted" { (pending_member_line(member)) }
                }
            }
            @if member.status() == Some(ChurchLinkStatus::Pending) {
                div class="row" {
                    form method="post" action={ "/churches/" (church_id) "/members/" (member.user_id) "/approve" } {
                        (csrf_input(csrf))
                        button class="btn" type="submit" { "Approve" }
                    }
                    form method="post" action={ "/churches/" (church_id) "/members/" (member.user_id) "/decline" } {
                        (csrf_input(csrf))
                        button class="btn btn-quiet" type="submit" { "Decline" }
                    }
                }
            }
        }
    }
}

fn member_name(member: &ChurchMember) -> String {
    display_name(&member.first_name, &member.last_name)
}

fn pending_member_line(member: &ChurchMember) -> &'static str {
    if member.status() == Some(ChurchLinkStatus::Pending) {
        "Asked to join"
    } else {
        "Invited"
    }
}

pub fn pending_people(members: &[ChurchMember]) -> impl Iterator<Item = &ChurchMember> {
    members.iter().filter(|member| member.is_pending())
}

pub fn active_member_items(members: &[ChurchMember]) -> Markup {
    html! {
        @for member in members.iter().filter(|member| member.is_active()) {
            (active_member_item(member))
        }
    }
}

fn active_member_item(member: &ChurchMember) -> Markup {
    let name = member_name(member);
    html! {
        li {
            a href={ "/members/" (member.user_id) } {
                (monogram(&member.user_id, &name, Monogram::Person))
                span { (name) }
            }
            span class="muted" { (role_word(member.role())) }
        }
    }
}

pub fn has_active_member(members: &[ChurchMember]) -> bool {
    members.iter().any(|member| member.is_active())
}

pub fn church_options<'a>(
    churches: impl IntoIterator<Item = &'a Church>,
    selected: Option<&str>,
) -> Markup {
    html! {
        @for church in churches {
            option value=(church.id) selected[selected == Some(church.id.as_str())] { (church.name) }
        }
    }
}

pub fn movable_needs_section(groups: &[ClosedNeedGroup], viewer: &Viewer, csrf: &str) -> Markup {
    if groups.is_empty() {
        return html! {};
    }
    let churches: Vec<&Church> = viewer.active_churches().collect();
    html! {
        @for group in groups {
            (movable_need_group(group, &churches, csrf))
        }
    }
}

fn movable_need_group(group: &ClosedNeedGroup, churches: &[&Church], csrf: &str) -> Markup {
    html! {
        section {
            h2 { "Open needs from " (group.church_name) }
            ul {
                @for title in &group.titles {
                    li { (title) }
                }
            }
            (move_needs_form(group, churches, csrf))
        }
    }
}

fn move_needs_form(group: &ClosedNeedGroup, churches: &[&Church], csrf: &str) -> Markup {
    let Some(first) = churches.first() else {
        return html! {};
    };
    html! {
        form method="post" action="/needs/import" {
            (csrf_input(csrf))
            input type="hidden" name="source_church_id" value=(group.church_id);
            @if churches.len() == 1 {
                input type="hidden" name="church_id" value=(first.id);
                button class="btn" type="submit" { "Move open needs to " (first.name) }
            } @else {
                label { "Church"
                    select name="church_id" required {
                        (church_options(churches.iter().copied(), None))
                    }
                }
                button class="btn" type="submit" { "Move open needs" }
            }
        }
    }
}

pub fn gift_options(gifts: &[Gift], selected: &str) -> Markup {
    gift_option_groups(gifts.iter(), selected)
}

pub fn catalog_name_options(catalog: &[Gift]) -> Markup {
    html! {
        @for gift in catalog {
            option value=(gift.name) {}
        }
    }
}

pub fn unused_gift_options(catalog: &[Gift], held: &[MemberGift], selected: &str) -> Markup {
    gift_option_groups(
        catalog
            .iter()
            .filter(|gift| !held.iter().any(|owned| owned.gift_id == gift.id)),
        selected,
    )
}

pub fn church_link_item(
    church: &Church,
    status: Option<ChurchLinkStatus>,
    role: Option<MembershipRole>,
) -> Markup {
    html! {
        li {
            a href={ "/churches/" (church.id) } {
                (monogram(&church.id, &church.name, Monogram::ChurchSmall))
                span { (church.name) }
            }
            span class="muted" { (household_line(status, role)) }
        }
    }
}

pub fn member_gift_cards(gifts: &[MemberGift]) -> Markup {
    html! {
        @for gift in gifts {
            (member_gift_card(gift))
        }
    }
}

fn member_gift_card(gift: &MemberGift) -> Markup {
    html! {
        article class="card" {
            p class="cat" { (category_label(&gift.category)) }
            h3 { (gift.gift_name) }
            @if !gift.note.is_empty() { p class="muted" { (gift.note) } }
        }
    }
}

#[derive(Clone, Copy)]
pub enum DeclineAction {
    CanAccept,
    Read,
}

pub fn accepted_endorsement_cards(endorsements: &[EndorsementCard]) -> Markup {
    html! {
        @for endorsement in endorsements {
            (accepted_endorsement_card(endorsement))
        }
    }
}

fn accepted_endorsement_card(endorsement: &EndorsementCard) -> Markup {
    html! {
        article class="card" {
            p class="cat" { (endorsement.gift_name) }
            p class="quote" { (endorsement.note) }
            (endorsement_from(endorsement))
        }
    }
}

fn endorsement_from(endorsement: &EndorsementCard) -> Markup {
    html! {
        p class="meta byline" {
            (monogram(&endorsement.from_user_id, &endorsement.from_user_name, Monogram::PersonSmall))
            span {
                "From " a href={ "/members/" (endorsement.from_user_id) } { (endorsement.from_user_name) }
            }
        }
    }
}

pub fn pending_endorsement_cards(pending: &[EndorsementCard], csrf: &str) -> Markup {
    html! {
        @for endorsement in pending {
            (pending_endorsement_card(endorsement, csrf))
        }
    }
}

fn pending_endorsement_card(endorsement: &EndorsementCard, csrf: &str) -> Markup {
    html! {
        article class="card" {
            div class="person-line" {
                (monogram(&endorsement.from_user_id, &endorsement.from_user_name, Monogram::Person))
                p {
                    a href={ "/members/" (endorsement.from_user_id) } { (endorsement.from_user_name) }
                    " endorsed you for " strong { (endorsement.gift_name) } "."
                }
            }
            p class="quote" { (endorsement.note) }
            div class="row" {
                form method="post" action={ "/endorsements/" (endorsement.id) "/accept" } {
                    (csrf_input(csrf))
                    button class="btn" type="submit" { "Accept" }
                }
                form method="post" action={ "/endorsements/" (endorsement.id) "/decline" } {
                    (csrf_input(csrf))
                    button class="btn btn-quiet" type="submit" { "Decline" }
                }
            }
        }
    }
}

pub fn declined_endorsement_cards<'a>(
    cards: impl IntoIterator<Item = &'a EndorsementCard>,
    csrf: &str,
    action: DeclineAction,
) -> Markup {
    html! {
        @for endorsement in cards {
            (declined_endorsement_card(endorsement, csrf, action))
        }
    }
}

fn declined_endorsement_card(
    endorsement: &EndorsementCard,
    csrf: &str,
    action: DeclineAction,
) -> Markup {
    html! {
        article class="card card-dim" {
            p class="cat" { (endorsement.gift_name) }
            p class="quote" { (endorsement.note) }
            (endorsement_from(endorsement))
            (declined_accept(endorsement, csrf, action))
        }
    }
}

fn declined_accept(endorsement: &EndorsementCard, csrf: &str, action: DeclineAction) -> Markup {
    match action {
        DeclineAction::Read => html! {},
        DeclineAction::CanAccept => html! {
            form method="post" action={ "/endorsements/" (endorsement.id) "/accept" } {
                (csrf_input(csrf))
                button class="btn" type="submit" { "Accept" }
            }
        },
    }
}

pub fn notice_cards<'a>(notes: impl IntoIterator<Item = &'a Notification>) -> Markup {
    html! {
        @for note in notes {
            (notice_card(note))
        }
    }
}

fn notice_card(note: &Notification) -> Markup {
    html! {
        a class="card card-link notice-card" href=(note.href) {
            h3 { (note.title) }
            p class="muted" { (note.body) }
        }
    }
}

pub fn my_gift_cards(gifts: &[MemberGift], csrf: &str) -> Markup {
    html! {
        @for gift in gifts {
            (my_gift_card(gift, csrf))
        }
    }
}

fn my_gift_card(gift: &MemberGift, csrf: &str) -> Markup {
    html! {
        article class="card row-between" {
            div {
                strong { (gift.gift_name) }
                @if !gift.note.is_empty() { p class="muted" { (gift.note) } }
            }
            form method="post" action={ "/me/gifts/" (gift.gift_id) "/remove" } {
                (csrf_input(csrf))
                button class="btn btn-quiet" type="submit" { "Remove" }
            }
        }
    }
}

pub fn place_sections(groups: &[PlaceGroup]) -> Markup {
    html! {
        @for group in groups {
            (place_section(group))
        }
    }
}

fn place_section(group: &PlaceGroup) -> Markup {
    let Some((church, _, _)) = group.first() else {
        return html! {};
    };
    html! {
        section class="place" {
            h2 { (church.address) }
            div class="stack" {
                (place_church_cards(group))
            }
        }
    }
}

fn place_church_cards(churches: &[ChurchCard]) -> Markup {
    html! {
        @for card in churches {
            (place_church_card(card))
        }
    }
}

fn place_church_card(card: &ChurchCard) -> Markup {
    let (church, members, needs) = card;
    html! {
        a class="card card-link church-card" href={ "/churches/" (church.id) } {
            (monogram(&church.id, &church.name, Monogram::Church))
            div {
                h3 { (church.name) }
                p class="clamp-2" { (church.description) }
                p class="meta" { (census_line(*members, *needs)) }
            }
        }
    }
}

fn gift_option_groups<'a>(gifts: impl Iterator<Item = &'a Gift>, selected: &str) -> Markup {
    let gifts: Vec<&Gift> = gifts.collect();
    html! {
        @for (category, items) in gift_runs(&gifts) {
            optgroup label=(category_label(category)) {
                @for gift in items {
                    option value=(gift.id) selected[selected == gift.id] { (gift.name) }
                }
            }
        }
    }
}

fn gift_runs<'a>(gifts: &[&'a Gift]) -> Vec<(&'a str, Vec<&'a Gift>)> {
    let mut runs: Vec<(&'a str, Vec<&'a Gift>)> = Vec::new();
    for gift in gifts {
        match runs.last_mut() {
            Some((category, items)) if *category == gift.category.as_str() => {
                items.push(*gift);
            }
            _ => runs.push((gift.category.as_str(), vec![*gift])),
        }
    }
    runs
}
