//! Repeated cards and option lists. Loops live here, not inside page bodies.

use maud::{html, Markup};

use crate::leaf::{
    ApplicationCard, Church, ChurchCard, ChurchMember, EndorsementCard, Gift, MemberGift,
    Membership, NeedCard, NeedScope, Notification, PlaceGroup, User, Viewer,
};

use super::layout::{csrf_input, initials};

pub fn need_card(need: &NeedCard, viewer: &Viewer) -> Markup {
    html! {
        a class="card card-link" href={ "/needs/" (need.id) } {
            p class="eyebrow" {
                (scope_label(&need.scope)) " · " (need.church_name) " · " (need.church_city)
            }
            h3 { (need.title) }
            p { (need.body) }
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

pub fn scope_label(scope: &str) -> &'static str {
    NeedScope::parse(scope)
        .map(NeedScope::label)
        .unwrap_or("Need")
}

pub fn need_card_stack<'a>(
    needs: impl IntoIterator<Item = &'a NeedCard>,
    viewer: &Viewer,
) -> Markup {
    html! {
        div class="stack" {
            @for need in needs {
                (need_card(need, viewer))
            }
        }
    }
}

pub fn persona_grid(users: &[User], csrf: &str) -> Markup {
    html! {
        div class="persona-grid" {
            (persona_forms(users, csrf))
        }
    }
}

fn persona_forms(users: &[User], csrf: &str) -> Markup {
    html! {
        @for user in users {
            (persona_form(user, csrf))
        }
    }
}

fn persona_form(user: &User, csrf: &str) -> Markup {
    html! {
        form method="post" action="/session" {
            (csrf_input(csrf))
            input type="hidden" name="user_id" value=(user.id);
            button class="persona" type="submit" {
                span class="avatar avatar-lg" { (initials(&user.name)) }
                strong { (user.name) }
                span { (persona_line(user)) }
            }
        }
    }
}

fn persona_line(user: &User) -> &'static str {
    match user.id.as_str() {
        "user_miriam" => "Pastor, Grace Covenant",
        "user_daniel" => "Grace Covenant · needs a ramp built",
        "user_ruth" => "Grace Covenant · has an endorsement waiting",
        "user_samuel" => "Rector, St. Luke's",
        "user_james" => "St. Luke's · carpenter",
        "user_keisha" => "Pastor, New Mercy (Waterloo)",
        "user_elena" => "New Mercy · speaks Spanish",
        "user_peter" => "Asked to join Grace Covenant",
        _ => "Member",
    }
}

pub fn pending_door_cards(pending: &[(&Membership, Church)], csrf: &str) -> Markup {
    html! {
        @for pair in pending {
            (pending_door_card(pair, csrf))
        }
    }
}

fn pending_door_card(pair: &(&Membership, Church), csrf: &str) -> Markup {
    let (membership, church) = pair;
    html! {
        article class="card" {
            @if membership.status == "pending_request" {
                p { "You asked to join " a href={ "/churches/" (church.id) } { (church.name) } ". The pastor will approve or decline." }
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
            p { (church.description) }
            p class="meta" { (members) " members · " (needs) " open needs" }
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
            @if member.status == "pending_request" {
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
    if member.status == "pending_request" {
        "Asked to join"
    } else {
        "Invited"
    }
}

pub fn pending_people(members: &[ChurchMember]) -> impl Iterator<Item = &ChurchMember> {
    members.iter().filter(|member| is_pending_member(member))
}

fn is_pending_member(member: &ChurchMember) -> bool {
    matches!(member.status.as_str(), "pending_request" | "pending_invite")
}

pub fn active_member_items(members: &[ChurchMember]) -> Markup {
    html! {
        @for member in members.iter().filter(|member| member.status == "active") {
            (active_member_item(member))
        }
    }
}

fn active_member_item(member: &ChurchMember) -> Markup {
    html! {
        li {
            a href={ "/members/" (member.user_id) } { (member.name) }
            span class="muted" { " · " (member.role) }
        }
    }
}

pub fn has_active_member(members: &[ChurchMember]) -> bool {
    members.iter().any(|member| member.status == "active")
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

pub fn gift_options(gifts: &[Gift]) -> Markup {
    html! {
        @for gift in gifts {
            option value=(gift.id) { (gift.name) " · " (gift.category) }
        }
    }
}

pub fn catalog_name_options(catalog: &[Gift]) -> Markup {
    html! {
        @for gift in catalog {
            option value=(gift.name) {}
        }
    }
}

pub fn unused_gift_options(catalog: &[Gift], held: &[MemberGift]) -> Markup {
    html! {
        @for gift in catalog {
            @if !held.iter().any(|owned| owned.gift_id == gift.id) {
                option value=(gift.id) { (gift.name) " · " (gift.category) }
            }
        }
    }
}

pub fn application_cards(
    applications: &[ApplicationCard],
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
            p class="meta" { (application.status) }
            @if matches!(steward, StewardView::Steward) && application.status == "pending" {
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

pub fn household_items(churches: &[(Church, &Membership)]) -> Markup {
    html! {
        @for pair in churches {
            (household_item(pair))
        }
    }
}

fn household_item(pair: &(Church, &Membership)) -> Markup {
    let (church, membership) = pair;
    html! {
        li {
            a href={ "/churches/" (church.id) } { (church.name) }
            span class="muted" { " · " (membership.role) " · " (membership.status) }
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
            p class="eyebrow" { (gift.category) }
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

pub fn notice_cards(notes: &[Notification]) -> Markup {
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
            p class="meta" { (note.kind) }
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
            p { (church.description) }
            p class="meta" { (members) " members · " (needs) " open needs" }
        }
    }
}
