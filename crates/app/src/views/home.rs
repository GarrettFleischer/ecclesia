use maud::{Markup, html};

use ecclesia_sdk::prelude::{Church, NeedCard, Viewer, visible_need_cards};

use super::cards::{NeedCardPlace, need_card_stack, waiting_church_card};
use super::flash::Flash;
use super::layout::{Icon, Nav, icon, page, page_lead};

pub fn home(
    viewer: &Viewer,
    flash: Option<Flash>,
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
            div class="page-head" {
                div {
                    (page_lead("Open needs"))
                }
                (post_need_action(viewer))
            }
            (no_church_yet(viewer))
            (pending_section(viewer, csrf))
            section {
                (needs_or_empty(needs, churches, viewer, next_cursor))
            }
        },
    )
}

fn no_church_yet(viewer: &Viewer) -> Markup {
    if viewer.is_active_anywhere() || viewer.user.is_waiting() {
        return html! {};
    }
    html! {
        div class="empty" {
            p { "You're not in a church on Ecclesia yet." }
            a class="btn" href="/churches/join" { "Find your church" }
        }
    }
}

fn pending_section(viewer: &Viewer, csrf: &str) -> Markup {
    let Some(church) = viewer.waiting_church() else {
        return html! {};
    };
    html! {
        section {
            h2 { "Waiting" }
            div class="stack" {
                (waiting_church_card(church, viewer.user.link_status(), csrf))
            }
        }
    }
}

fn post_need_action(viewer: &Viewer) -> Markup {
    if !viewer.is_active_anywhere() {
        return html! {};
    }
    html! {
        a class="btn" href="/needs/new" {
            span class="btn-icon" aria-hidden="true" { (icon(Icon::Plus)) }
            "Post a need"
        }
    }
}

fn needs_or_empty(
    needs: &[NeedCard],
    churches: &[Church],
    viewer: &Viewer,
    next_cursor: Option<&str>,
) -> Markup {
    let mut visible = visible_need_cards(viewer, needs, churches).peekable();
    if visible.peek().is_none() {
        return empty_needs(viewer);
    }
    html! {
        (need_card_stack(visible, viewer, NeedCardPlace::Feed))
        (super::more_needs("/home", next_cursor))
    }
}

fn empty_needs(viewer: &Viewer) -> Markup {
    if viewer.is_active_anywhere() {
        return html! {
            div class="empty" { p { "No open needs." } }
        };
    }
    if viewer.user.is_waiting() {
        return html! {};
    }
    html! {
        div class="empty" { p { "Once you're in a church, its needs show up here." } }
    }
}
