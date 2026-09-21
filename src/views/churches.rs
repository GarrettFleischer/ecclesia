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
                h1 { "Church households" }
                a class="btn" href="/churches/new" { "Plant a group" }
            }
            p class="muted" { "A church here is a group with a shepherd. You request or receive an invite. They approve. Then your gifts are actually findable." }
            form class="row-form" method="post" action="/invites/redeem" {
                (csrf_input(csrf))
                label { "Invite code"
                    input name="code" placeholder="grace-k2m9" autocomplete="off";
                }
                button class="btn btn-quiet" type="submit" { "Redeem" }
            }
            (church_list(churches))
        },
    )
}

fn church_list(churches: &[ChurchCard]) -> Markup {
    if churches.is_empty() {
        return html! {
            div class="empty" { p { "No churches yet. The first pastor has to plant one." } }
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
        "Plant a church group",
        Some(&viewer.user),
        unread,
        Nav::Churches,
        flash,
        html! {
            h1 { "Plant a church group" }
            p class="muted" { "You become the owner — the pastor or steward who approves people in. Neighboring churches will see you if you share a city or region." }
            form class="stack" method="post" action="/churches" {
                (csrf_input(csrf))
                label { "Church name" input name="name" required placeholder="Grace Covenant Church" maxlength="120"; }
                div class="split" {
                    label { "City" input name="city" required value=(viewer.user.city); }
                    label { "Region" input name="region" required value=(viewer.user.region); }
                }
                label { "When you gather" input name="gathering" placeholder="Sundays 10:00 a.m."; }
                label { "Who you are"
                    textarea name="description" rows="4" required placeholder="A household in this city, not a brand." {}
                }
                button class="btn" type="submit" { "Create the group" }
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
            p class="meta" {
                @if !church.gathering.is_empty() { (church.gathering) " · " }
                "Shepherd: the owner of this group"
            }
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
        "active" => html! { p class="pill" { "You belong here as " (membership.role) } },
        "pending_request" => {
            html! { p class="pill pill-wait" { "Your request is waiting on a pastor" } }
        }
        "pending_invite" => html! {
            form method="post" action={ "/memberships/" (membership.id) "/accept-invite" } {
                (csrf_input(csrf))
                button class="btn" type="submit" { "Accept this church's invite" }
            }
        },
        "declined" => html! { p class="pill pill-warn" { "A previous request was declined" } },
        _ => html! {},
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
            h2 { "Keep the door" }
            p class="muted" { "Share this invite code. They still confirm; you already chose them. Or invite someone already in Ecclesia by email." }
            p class="code" { (church.invite_code) }
            form class="row-form" method="post" action={ "/churches/" (church.id) "/invite" } {
                (csrf_input(csrf))
                label { "Invite by email"
                    input type="email" name="email" required placeholder="james@stlukes.test";
                }
                button class="btn btn-quiet" type="submit" { "Invite" }
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
    html! { p class="muted" { "No approved members yet." } }
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
            p class="muted" { "No needs posted. Either they are between crises, or they have not learned to ask." }
        };
    }
    need_card_stack(visible, viewer)
}

pub fn the_body(viewer: &Viewer, groups: &[PlaceGroup], unread: i64) -> Markup {
    page(
        "The body",
        Some(&viewer.user),
        unread,
        Nav::Body,
        None,
        html! {
            h1 { "Churches are not islands" }
            p class="lede" {
                "A need marked for neighboring churches is visible to approved members in the same city or region. "
                "A need marked for the whole body is visible to anyone already received into a household. "
                "The point is not a marketplace. It is that the wound in one congregation can be bound by another."
            }
            (body_groups(groups))
        },
    )
}

fn body_groups(groups: &[PlaceGroup]) -> Markup {
    if groups.is_empty() {
        return html! {
            div class="empty" { p { "No churches have been planted yet." } }
        };
    }
    place_sections(groups)
}
