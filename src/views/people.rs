use maud::{html, Markup};

use crate::leaf::{
    Church, EndorsementCard, Gift, MemberGift, Membership, Notification, User, Viewer,
};

use super::cards::{
    catalog_name_options, household_items, member_gift_cards, my_gift_cards, notice_cards,
    pending_endorsement_cards, published_endorsement_cards, unused_gift_options,
};
use super::flash::Flash;
use super::layout::{csrf_input, first_name, page, Nav};

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
        csrf,
        html! {
            p class="eyebrow" { (person.city) ", " (person.region) }
            h1 { (person.name) }
            (bio_lede(person))
            section {
                h2 { "Churches" }
                (households(churches))
            }
            section {
                h2 { "Gifts" }
                (gift_section(gifts))
            }
            (endorsement_section(endorsements))
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
        return html! { p class="muted" { "None yet." } };
    }
    html! {
        ul class="people" {
            (household_items(churches))
        }
    }
}

fn gift_section(gifts: &[MemberGift]) -> Markup {
    if gifts.is_empty() {
        return html! { p class="muted" { "None yet." } };
    }
    html! {
        div class="stack" {
            (member_gift_cards(gifts))
        }
    }
}

fn endorsement_section(endorsements: &[EndorsementCard]) -> Markup {
    if endorsements.is_empty() {
        return html! {};
    }
    html! {
        section {
            h2 { "Endorsements" }
            div class="stack" {
                (published_endorsement_cards(endorsements))
            }
        }
    }
}

fn endorse_panel(viewer: &Viewer, person: &User, catalog: &[Gift], csrf: &str) -> Markup {
    if viewer.user.id == person.id {
        return html! {};
    }
    html! {
        section class="panel" {
            h2 { "Endorse " (first_name(&person.name)) }
            form class="stack" method="post" action={ "/members/" (person.id) "/endorse" } {
                (csrf_input(csrf))
                label { "Skill"
                    input name="skill" required maxlength="120" list="catalog-skills" placeholder="Hospitality";
                    datalist id="catalog-skills" {
                        (catalog_name_options(catalog))
                    }
                }
                label { "Why"
                    textarea name="note" rows="5" required maxlength="600" placeholder="She stayed until the last parent came." {}
                }
                button class="btn" type="submit" { "Send" }
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
        csrf,
        html! {
            h1 { "Inbox" }
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
            p { "Nothing here yet." }
        }
    }
}

fn pending_endorsements(pending: &[EndorsementCard], csrf: &str) -> Markup {
    if pending.is_empty() {
        return html! {};
    }
    html! {
        section {
            h2 { "Endorsements" }
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
            h2 { "Updates" }
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
        csrf,
        html! {
            h1 { (viewer.user.name) }
            form class="stack" method="post" action="/me" {
                (csrf_input(csrf))
                label { "Name" input name="name" required value=(viewer.user.name) maxlength="80"; }
                div class="split" {
                    label { "City" input name="city" required value=(viewer.user.city); }
                    label { "State or region" input name="region" required value=(viewer.user.region); }
                }
                label { "About you"
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
                    label { "Note (optional)"
                        input name="note" placeholder="Thursday nights. Hospital visits. Spreadsheets.";
                    }
                    button class="btn btn-quiet" type="submit" { "Add" }
                }
            }
            section {
                h2 { "Your churches" }
                (my_churches(memberships))
            }
            section class="panel" {
                h2 { "Alerts" }
                p class="muted" { "A banner on this phone when someone endorses you or a need needs you." }
                p class="muted" { "On iPhone, add the app to your home screen first. Then turn on alerts here." }
                button type="button" class="btn" data-alerts { "Turn on alerts" }
                p class="muted" data-alerts-status hidden {}
            }
            form method="post" action="/session/logout" {
                (csrf_input(csrf))
                button class="btn btn-quiet" type="submit" { "Sign out" }
            }
        },
    )
}

fn my_gifts_block(gifts: &[MemberGift], csrf: &str) -> Markup {
    if gifts.is_empty() {
        return html! {
            p class="muted" { "None yet." }
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
            a href="/churches" { "Find your church" }
        };
    }
    html! {
        ul class="people" {
            (household_items(memberships))
        }
    }
}
