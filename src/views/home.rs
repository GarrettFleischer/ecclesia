use maud::{html, Markup};

use crate::leaf::{visible_need_cards, Church, Membership, NeedCard, Viewer};

use super::cards::{need_card_stack, pending_door_cards};
use super::flash::Flash;
use super::layout::{first_name, page, Nav};

pub fn home(
    viewer: &Viewer,
    flash: Option<Flash>,
    pending: &[(&Membership, Church)],
    needs: &[NeedCard],
    churches: &[Church],
    unread: i64,
    csrf: &str,
) -> Markup {
    page(
        "Home",
        Some(&viewer.user),
        unread,
        Nav::Home,
        flash,
        html! {
            p class="eyebrow" { (viewer.user.city) ", " (viewer.user.region) }
            h1 { "Peace, " (first_name(&viewer.user.name)) "." }
            hr class="gold-rule";
            (empty_household(viewer, pending))
            (door_section(pending, csrf))
            (active_toolbar(viewer))
            section {
                h2 { "Needs the body can carry" }
                (needs_or_empty(needs, churches, viewer))
            }
        },
    )
}

fn empty_household(viewer: &Viewer, pending: &[(&Membership, Church)]) -> Markup {
    if viewer.is_active_anywhere() || !pending.is_empty() {
        return html! {};
    }
    html! {
        div class="empty" {
            p { "You are not yet in a church household. Ask to join one, redeem an invite, or plant a group if you are the pastor who will keep it." }
            a class="btn" href="/churches" { "Find a church" }
        }
    }
}

fn door_section(pending: &[(&Membership, Church)], csrf: &str) -> Markup {
    if pending.is_empty() {
        return html! {};
    }
    html! {
        section class="panel" {
            h2 { "At the door" }
            (pending_door_cards(pending, csrf))
        }
    }
}

fn active_toolbar(viewer: &Viewer) -> Markup {
    if !viewer.is_active_anywhere() {
        return html! {};
    }
    html! {
        div class="toolbar" {
            a class="btn" href="/needs/new" { "Post a need" }
            a class="btn btn-quiet" href="/the-body" { "See neighboring churches" }
        }
    }
}

fn needs_or_empty(needs: &[NeedCard], churches: &[Church], viewer: &Viewer) -> Markup {
    let mut visible = visible_need_cards(viewer, needs, churches).peekable();
    if visible.peek().is_none() {
        return html! {
            div class="empty" {
                p { "No open needs you can see. That can mean rest — or that a church is still trying to carry everything alone." }
            }
        };
    }
    need_card_stack(visible, viewer)
}
