use maud::{html, Markup};

use crate::leaf::{
    visible_need_cards, Church, ChurchCard, ChurchMember, Membership, NeedCard, PlaceGroup, Viewer,
};

use super::cards::{
    active_member_items, church_index_cards, has_active_member, need_card_stack,
    pending_member_cards, pending_people, place_sections,
};
use super::flash::Flash;
use super::layout::{csrf_input, page, Nav};

pub fn churches_index(
    viewer: &Viewer,
    flash: Option<Flash>,
    churches: &[ChurchCard],
    unread: i64,
    csrf: &str,
) -> Markup {
    page(
        "Churches",
        Some(&viewer.user),
        unread,
        Nav::Churches,
        flash,
        html! {
            div class="toolbar" {
                h1 { "Churches" }
                a class="btn" href="/churches/new" { "Add your church" }
            }
            form class="row-form" method="post" action="/invites/redeem" {
                (csrf_input(csrf))
                label { "Have an invite code?"
                    input name="code" placeholder="grace-k2m9" autocomplete="off";
                }
                button class="btn btn-quiet" type="submit" { "Use it" }
            }
            (church_list(churches))
        },
    )
}

fn church_list(churches: &[ChurchCard]) -> Markup {
    if churches.is_empty() {
        return html! {
            div class="empty" { p { "No churches yet. Yours could be the first." } }
        };
    }
    html! {
        div class="stack" {
            (church_index_cards(churches))
        }
    }
}

pub fn church_new(viewer: &Viewer, unread: i64, flash: Option<Flash>, csrf: &str) -> Markup {
    page(
        "Add your church",
        Some(&viewer.user),
        unread,
        Nav::Churches,
        flash,
        html! {
            h1 { "Add your church" }
            p class="muted" { "You'll be its pastor here, so you decide who joins." }
            form class="stack" method="post" action="/churches" {
                (csrf_input(csrf))
                label { "Church name" input name="name" required placeholder="Grace Covenant Church" maxlength="120"; }
                div class="split" {
                    label { "City" input name="city" required value=(viewer.user.city); }
                    label { "State or region" input name="region" required value=(viewer.user.region); }
                }
                label { "When you meet" input name="gathering" placeholder="Sundays, 10 a.m."; }
                label { "About the church"
                    textarea name="description" rows="4" required placeholder="A few sentences. Where you are, who comes, what you're about." {}
                }
                button class="btn" type="submit" { "Add church" }
            }
        },
    )
}

pub fn church_show(
    viewer: &Viewer,
    church: &Church,
    members: &[ChurchMember],
    needs: &[NeedCard],
    flash: Option<Flash>,
    unread: i64,
    csrf: &str,
) -> Markup {
    let mine = viewer.membership_in(&church.id);
    let door = door_keep(viewer, church);
    page(
        &church.name,
        Some(&viewer.user),
        unread,
        Nav::Churches,
        flash,
        html! {
            p class="eyebrow" { (church.city) ", " (church.region) }
            h1 { (church.name) }
            p class="lede" { (church.description) }
            @if !church.gathering.is_empty() { p class="meta" { (church.gathering) } }
            (membership_status(mine, church, csrf))
            (governor_door(church, door, csrf))
            (people_section(members, door, csrf))
            (needs_section(viewer, church, needs))
        },
    )
}

fn membership_status(mine: Option<&Membership>, church: &Church, csrf: &str) -> Markup {
    match mine {
        Some(membership) => membership_pill(membership, csrf),
        None => html! {
            form method="post" action={ "/churches/" (church.id) "/join" } {
                (csrf_input(csrf))
                button class="btn" type="submit" { "Ask to join" }
            }
        },
    }
}

fn membership_pill(membership: &Membership, csrf: &str) -> Markup {
    match membership.status.as_str() {
        "active" => html! { p class="pill" { (role_line(&membership.role)) } },
        "pending_request" => {
            html! { p class="pill pill-wait" { "You asked to join. Waiting on the pastor." } }
        }
        "pending_invite" => html! {
            form method="post" action={ "/memberships/" (membership.id) "/accept-invite" } {
                (csrf_input(csrf))
                button class="btn" type="submit" { "Accept invite" }
            }
        },
        "declined" => html! { p class="pill pill-warn" { "Your request was declined." } },
        _ => html! {},
    }
}

fn role_line(role: &str) -> &'static str {
    match role {
        "owner" => "You're the pastor here.",
        "steward" => "You're a steward here.",
        _ => "You're a member here.",
    }
}

#[derive(Clone, Copy)]
enum DoorKeep {
    Keeps,
    WalksThrough,
}

fn door_keep(viewer: &Viewer, church: &Church) -> DoorKeep {
    if viewer.can_govern(&church.id) {
        DoorKeep::Keeps
    } else {
        DoorKeep::WalksThrough
    }
}

fn governor_door(church: &Church, door: DoorKeep, csrf: &str) -> Markup {
    if !matches!(door, DoorKeep::Keeps) {
        return html! {};
    }
    html! {
        section class="panel" {
            h2 { "Invite people" }
            p class="muted" { "Share this code, or send an invite by email." }
            p class="code" { (church.invite_code) }
            form class="row-form" method="post" action={ "/churches/" (church.id) "/invite" } {
                (csrf_input(csrf))
                label { "Email"
                    input type="email" name="email" required placeholder="james@stlukes.test";
                }
                button class="btn btn-quiet" type="submit" { "Send invite" }
            }
        }
    }
}

fn people_section(members: &[ChurchMember], door: DoorKeep, csrf: &str) -> Markup {
    html! {
        section {
            h2 { "People" }
            (pending_people_block(members, door, csrf))
            ul class="people" {
                (active_member_items(members))
            }
            (no_active_members(members))
        }
    }
}

fn pending_people_block(members: &[ChurchMember], door: DoorKeep, csrf: &str) -> Markup {
    if !matches!(door, DoorKeep::Keeps) {
        return html! {};
    }
    let mut pending = pending_people(members).peekable();
    if pending.peek().is_none() {
        return html! {};
    }
    html! {
        div class="stack" {
            (pending_member_cards(pending, csrf))
        }
    }
}

fn no_active_members(members: &[ChurchMember]) -> Markup {
    if has_active_member(members) {
        return html! {};
    }
    html! { p class="muted" { "No members yet." } }
}

fn needs_section(viewer: &Viewer, church: &Church, needs: &[NeedCard]) -> Markup {
    html! {
        section {
            div class="toolbar" {
                h2 { "Needs" }
                @if viewer.is_active_in(&church.id) {
                    a class="btn btn-quiet" href={ "/needs/new?church_id=" (church.id) } { "Post a need" }
                }
            }
            (church_needs(needs, viewer, church))
        }
    }
}

fn church_needs(needs: &[NeedCard], viewer: &Viewer, church: &Church) -> Markup {
    let mut visible = visible_need_cards(viewer, needs, std::slice::from_ref(church)).peekable();
    if visible.peek().is_none() {
        return html! {
            p class="muted" { "No open needs." }
        };
    }
    need_card_stack(visible, viewer)
}

pub fn the_body(viewer: &Viewer, groups: &[PlaceGroup], unread: i64) -> Markup {
    page(
        "Churches nearby",
        Some(&viewer.user),
        unread,
        Nav::Body,
        None,
        html! {
            h1 { "Churches nearby" }
            hr class="gold-rule";
            p class="lede" { "Churches in the same city or region can see each other's needs." }
            (body_groups(groups))
        },
    )
}

fn body_groups(groups: &[PlaceGroup]) -> Markup {
    if groups.is_empty() {
        return html! {
            div class="empty" { p { "No churches yet." } }
        };
    }
    place_sections(groups)
}
