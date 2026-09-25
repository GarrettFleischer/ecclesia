//! Repeated cards and option lists. Loops live here, not inside page bodies.

use maud::{Markup, html};

use ecclesia_sdk::prelude::{
    ApplicationCard, ApplicationStatus, Church, ChurchCard, ChurchMember, EndorsementCard, Gift,
    MemberGift, Membership, MembershipStatus, NeedCard, NeedScope, Notification, PlaceGroup,
    Viewer,
};

use super::layout::{csrf_input, initials};
use super::words::{category_label, census_line, household_line, offer_status_word, role_word};

#[derive(Clone, Copy)]
pub enum NeedCardPlace {
    Feed,
    Church,
}

pub fn need_card(need: &NeedCard, viewer: &Viewer, place: NeedCardPlace) -> Markup {
    html! {
        a class="card card-link" href={ "/needs/" (need.id) } {
            p class="eyebrow" { (need_card_eyebrow(need, place)) }
            h3 { (need.title) }
            p class="clamp" { (need.body) }
            p class="meta" {
                @if let Some(gift) = &need.gift_name { (gift) " · " }
                (need.author_name)
                @if let Some(gift_id) = &need.gift_id {
                    @if viewer.has_gift(gift_id) { span class="chip" { "Your gift" } }
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
                need.church_city
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

pub fn pending_door_cards(pending: &[(&Church, &Membership)], csrf: &str) -> Markup {
    html! {
        @for pair in pending {
            (pending_door_card(pair, csrf))
        }
    }
}

fn pending_door_card(pair: &(&Church, &Membership), csrf: &str) -> Markup {
    let (church, membership) = pair;
    html! {
        article class="card" {
            @if membership.status() == Some(MembershipStatus::PendingRequest) {
                p { "You asked to join " a href={ "/churches/" (church.id) } { (church.name) } }
                p class="muted" { "Waiting on the pastor." }
            } @else {
                p { (church.name) " invited you." }
                form method="post" action={ "/memberships/" (membership.id) "/accept-invite" } {
                    (csrf_input(csrf))
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
        a class="card card-link" href={ "/churches/" (church.id) } {
            h3 { (church.name) }
            p class="muted" { (church.city) ", " (church.region) }
            p class="clamp-2" { (church.description) }
            p class="meta" { (census_line(*members, *needs)) }
        }
    }
}

pub fn pending_member_cards<'a>(
    members: impl IntoIterator<Item = &'a ChurchMember>,
    csrf: &str,
) -> Markup {
    html! {
        @for member in members {
            (pending_member_card(member, csrf))
        }
    }
}

fn pending_member_card(member: &ChurchMember, csrf: &str) -> Markup {
    html! {
        article class="card" {
            a href={ "/members/" (member.user_id) } { strong { (member.name) } }
            p class="muted" { (pending_member_line(member)) }
            @if member.status() == Some(MembershipStatus::PendingRequest) {
                div class="row" {
                    form method="post" action={ "/memberships/" (member.membership_id) "/approve" } {
                        (csrf_input(csrf))
                        button class="btn" type="submit" { "Approve" }
                    }
                    form method="post" action={ "/memberships/" (member.membership_id) "/decline" } {
                        (csrf_input(csrf))
                        button class="btn btn-quiet" type="submit" { "Decline" }
                    }
                }
            }
        }
    }
}

fn pending_member_line(member: &ChurchMember) -> &'static str {
    if member.status() == Some(MembershipStatus::PendingRequest) {
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
    html! {
        li {
            a href={ "/members/" (member.user_id) } {
                span class="avatar" { (initials(&member.name)) }
                span { (member.name) }
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

pub fn application_cards<'a>(
    applications: impl IntoIterator<Item = &'a ApplicationCard>,
    steward: StewardView,
    csrf: &str,
) -> Markup {
    html! {
        @for application in applications {
            (application_card(application, steward, csrf))
        }
    }
}

#[derive(Clone, Copy)]
pub enum StewardView {
    Steward,
    Guest,
}

fn application_card(application: &ApplicationCard, steward: StewardView, csrf: &str) -> Markup {
    html! {
        article class="card" {
            a href={ "/members/" (application.user_id) } { strong { (application.user_name) } }
            p { (application.message) }
            p class="meta" { (offer_status_word(application.status())) }
            @if matches!(steward, StewardView::Steward)
                && application.status() == Some(ApplicationStatus::Pending)
            {
                (application_verdict_row(application, csrf))
            }
        }
    }
}

fn application_verdict_row(application: &ApplicationCard, csrf: &str) -> Markup {
    html! {
        div class="row" {
            form method="post" action={ "/applications/" (application.id) "/accept" } {
                (csrf_input(csrf))
                button class="btn" type="submit" { "Accept" }
            }
            form method="post" action={ "/applications/" (application.id) "/decline" } {
                (csrf_input(csrf))
                button class="btn btn-quiet" type="submit" { "Decline" }
            }
        }
    }
}

pub fn household_items(churches: &[(&Church, &Membership)]) -> Markup {
    html! {
        @for pair in churches {
            (household_item(pair))
        }
    }
}

fn household_item(pair: &(&Church, &Membership)) -> Markup {
    let (church, membership) = pair;
    html! {
        li {
            a href={ "/churches/" (church.id) } { (church.name) }
            span class="muted" { (household_line(membership)) }
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
            @if !gift.note.is_empty() { p { (gift.note) } }
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
            p class="eyebrow" { (endorsement.gift_name) }
            p class="quote" { (endorsement.note) }
            p class="meta" {
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
            p {
                a href={ "/members/" (endorsement.from_user_id) } { (endorsement.from_user_name) }
                " endorsed you for " strong { (endorsement.gift_name) } "."
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
            p class="eyebrow" { (endorsement.gift_name) }
            p class="quote" { (endorsement.note) }
            p class="meta" {
                "From " a href={ "/members/" (endorsement.from_user_id) } { (endorsement.from_user_name) }
            }
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
        a class="card card-link" href=(note.href) {
            h3 { (note.title) }
            p { (note.body) }
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
        section class="panel" {
            h2 { (church.city) ", " (church.region) }
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
        a class="card card-link" href={ "/churches/" (church.id) } {
            h3 { (church.name) }
            p class="clamp-2" { (church.description) }
            p class="meta" { (census_line(*members, *needs)) }
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
