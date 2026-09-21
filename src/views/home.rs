use maud::{Markup, html};

use crate::leaf::{Church, Membership, NeedCard, Viewer, visible_need_cards};

use super::cards::{NeedCardPlace, need_card_stack, pending_door_cards};
use super::flash::Flash;
use super::layout::{Nav, page};

pub fn home(
    viewer: &Viewer,
    flash: Option<Flash>,
    pending: &[(&Church, &Membership)],
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
        csrf,
        html! {
            p class="eyebrow" { (viewer.user.city) ", " (viewer.user.region) }
            h1 { "Open needs" }
            hr class="gold-rule";
            (no_church_yet(viewer, pending))
            (pending_section(pending, csrf))
            (active_toolbar(viewer))
            section {
                (needs_or_empty(needs, churches, viewer))
            }
        },
    )
}

fn no_church_yet(viewer: &Viewer, pending: &[(&Church, &Membership)]) -> Markup {
    if viewer.is_active_anywhere() || !pending.is_empty() {
        return html! {};
    }
    html! {
        div class="empty" {
            p { "You're not in a church on Ecclesia yet." }
            a class="btn" href="/churches" { "Find your church" }
        }
    }
}

fn pending_section(pending: &[(&Church, &Membership)], csrf: &str) -> Markup {
    if pending.is_empty() {
        return html! {};
    }
    html! {
        section class="panel" {
            h2 { "Pending" }
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
        }
    }
}

fn needs_or_empty(needs: &[NeedCard], churches: &[Church], viewer: &Viewer) -> Markup {
    let mut visible = visible_need_cards(viewer, needs, churches).peekable();
    if visible.peek().is_none() {
        return html! {
            div class="empty" { p { (no_needs_line(viewer)) } }
        };
    }
    need_card_stack(visible, viewer, NeedCardPlace::Feed)
}

fn no_needs_line(viewer: &Viewer) -> &'static str {
    if viewer.is_active_anywhere() {
        "No open needs."
    } else {
        "Once you're in a church, its needs show up here."
    }
}
