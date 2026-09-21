use maud::{html, Markup};

use crate::leaf::{
    Church, EndorsementCard, Gift, MemberGift, Membership, Notification, User, Viewer,
};

use super::cards::{
    catalog_name_options, household_items, member_gift_cards, my_gift_cards, notice_cards,
    pending_endorsement_cards, unused_gift_options,
};
use super::flash::Flash;
use super::layout::{csrf_input, page, Nav};

pub fn member_show(
    viewer: &Viewer,
    person: &User,
    churches: &[(Church, &Membership)],
    gifts: &[MemberGift],
    endorsements: &[EndorsementCard],
    catalog: &[Gift],
    unread: i64,
    flash: Option<Flash>,
    csrf: &str,
) -> Markup {
    let nav = nav_for_person(viewer, person);
    page(
        &person.name,
        Some(&viewer.user),
        unread,
        nav,
        flash,
        html! {
            p class="eyebrow" { (person.city) ", " (person.region) }
            h1 { (person.name) }
            (bio_lede(person))
            section {
                h2 { "Households" }
                (households(churches))
            }
            section {
                h2 { "Gifts" }
                (gift_section(gifts, endorsements))
            }
            (endorse_panel(viewer, person, catalog, csrf))
        },
    )
}

fn nav_for_person(viewer: &Viewer, person: &User) -> Nav {
    if viewer.user.id == person.id {
        Nav::You
    } else {
        Nav::Churches
    }
}

fn bio_lede(person: &User) -> Markup {
    if person.bio.is_empty() {
        return html! {};
    }
    html! { p class="lede" { (person.bio) } }
}

fn households(churches: &[(Church, &Membership)]) -> Markup {
    if churches.is_empty() {
        return html! { p class="muted" { "Not yet approved in a church." } };
    }
    html! {
        ul class="people" {
            (household_items(churches))
        }
    }
}

fn gift_section(gifts: &[MemberGift], endorsements: &[EndorsementCard]) -> Markup {
    if gifts.is_empty() {
        return html! { p class="muted" { "No gifts named yet." } };
    }
    html! {
        div class="stack" {
            (member_gift_cards(gifts, endorsements))
        }
    }
}

fn endorse_panel(viewer: &Viewer, person: &User, catalog: &[Gift], csrf: &str) -> Markup {
    if viewer.user.id == person.id {
        return html! {};
    }
    html! {
        section class="panel" {
            h2 { "Endorse a gift" }
            p class="muted" { "They will be notified and can accept it onto their profile — or decline. You do not get to write their name for them." }
            form class="stack" method="post" action={ "/members/" (person.id) "/endorse" } {
                (csrf_input(csrf))
                label { "Gift"
                    select name="gift_id" required {
                        (catalog_name_options(catalog))
                    }
                }
                label { "What you have actually seen"
                    textarea name="note" rows="3" required placeholder="Be specific. A gift is a life, not a compliment." {}
                }
                button class="btn" type="submit" { "Send endorsement" }
            }
        }
    }
}

pub fn inbox(
    viewer: &Viewer,
    pending: &[EndorsementCard],
    notes: &[Notification],
    unread: i64,
    flash: Option<Flash>,
    csrf: &str,
) -> Markup {
    page(
        "Inbox",
        Some(&viewer.user),
        unread,
        Nav::Inbox,
        flash,
        html! {
            h1 { "What needs your yes" }
            hr class="gold-rule";
            (inbox_empty(pending, notes))
            (pending_endorsements(pending, csrf))
            (notice_section(notes))
        },
    )
}

fn inbox_empty(pending: &[EndorsementCard], notes: &[Notification]) -> Markup {
    if !pending.is_empty() || !notes.is_empty() {
        return html! {};
    }
    html! {
        div class="empty" {
            p { "Nothing waiting. When someone endorses you, asks to join your church, or offers to help, it will land here." }
        }
    }
}

fn pending_endorsements(pending: &[EndorsementCard], csrf: &str) -> Markup {
    if pending.is_empty() {
        return html! {};
    }
    html! {
        section {
            h2 { "Endorsements to receive" }
            div class="stack" {
                (pending_endorsement_cards(pending, csrf))
            }
        }
    }
}

fn notice_section(notes: &[Notification]) -> Markup {
    if notes.is_empty() {
        return html! {};
    }
    html! {
        section {
            h2 { "Notices" }
            div class="stack" {
                (notice_cards(notes))
            }
        }
    }
}

pub fn me(
    viewer: &Viewer,
    gifts: &[MemberGift],
    catalog: &[Gift],
    memberships: &[(Church, &Membership)],
    unread: i64,
    flash: Option<Flash>,
    csrf: &str,
) -> Markup {
    page(
        "You",
        Some(&viewer.user),
        unread,
        Nav::You,
        flash,
        html! {
            h1 { (viewer.user.name) }
            form class="stack" method="post" action="/me" {
                (csrf_input(csrf))
                label { "Name" input name="name" required value=(viewer.user.name) maxlength="80"; }
                div class="split" {
                    label { "City" input name="city" required value=(viewer.user.city); }
                    label { "Region" input name="region" required value=(viewer.user.region); }
                }
                label { "How you serve"
                    textarea name="bio" rows="3" { (viewer.user.bio) }
                }
                button class="btn" type="submit" { "Save" }
            }
            section class="panel" {
                h2 { "Your gifts" }
                (my_gifts_block(gifts, csrf))
                form class="stack" method="post" action="/me/gifts" {
                    (csrf_input(csrf))
                    label { "Add a gift"
                        select name="gift_id" required {
                            (unused_gift_options(catalog, gifts))
                        }
                    }
                    label { "How you practice it"
                        input name="note" placeholder="Thursday nights. Hospital rooms. Spreadsheets.";
                    }
                    button class="btn btn-quiet" type="submit" { "Add gift" }
                }
            }
            section {
                h2 { "Your churches" }
                (my_churches(memberships))
            }
            form method="post" action="/session/logout" {
                (csrf_input(csrf))
                button class="btn btn-quiet" type="submit" { "Leave this seat" }
            }
        },
    )
}

fn my_gifts_block(gifts: &[MemberGift], csrf: &str) -> Markup {
    if gifts.is_empty() {
        return html! {
            p class="muted" { "Name what you can actually do. This is how a need finds a person." }
        };
    }
    html! {
        div class="stack" {
            (my_gift_cards(gifts, csrf))
        }
    }
}

fn my_churches(memberships: &[(Church, &Membership)]) -> Markup {
    if memberships.is_empty() {
        return html! {
            p class="muted" { "None yet." }
            a href="/churches" { "Join or plant one" }
        };
    }
    html! {
        ul class="people" {
            (household_items(memberships))
        }
    }
}
