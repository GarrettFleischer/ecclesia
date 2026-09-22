use maud::{Markup, html};

use ecclesia_sdk::prelude::{Church, Membership, NeedCard, Viewer, visible_need_cards};

use super::cards::{NeedCardPlace, need_card_stack, pending_door_cards};
use super::flash::Flash;
use super::layout::{Nav, page, page_lead};

pub fn home(
    viewer: &Viewer,
    flash: Option<Flash>,
    pending: &[(&Church, &Membership)],
    needs: &[NeedCard],
    churches: &[Church],
    next_cursor: Option<&str>,
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
            (page_lead("Open needs"))
            (no_church_yet(viewer, pending))
            (pending_section(pending, csrf))
            (active_toolbar(viewer))
            section {
                (needs_or_empty(needs, churches, viewer, pending, next_cursor))
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
        section {
            h2 { "Waiting" }
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

fn needs_or_empty(
    needs: &[NeedCard],
    churches: &[Church],
    viewer: &Viewer,
    pending: &[(&Church, &Membership)],
    next_cursor: Option<&str>,
) -> Markup {
    let mut visible = visible_need_cards(viewer, needs, churches).peekable();
    if visible.peek().is_none() {
        return empty_needs(viewer, pending);
    }
    html! {
        (need_card_stack(visible, viewer, NeedCardPlace::Feed))
        (super::more_needs("/home", next_cursor))
    }
}

fn empty_needs(viewer: &Viewer, pending: &[(&Church, &Membership)]) -> Markup {
    if viewer.is_active_anywhere() {
        return html! {
            div class="empty" { p { "No open needs." } }
        };
    }
    if !pending.is_empty() {
        return html! {};
    }
    html! {
        div class="empty" { p { "Once you're in a church, its needs show up here." } }
    }
}
