use maud::{Markup, html};

use crate::leaf::{
    Church, EndorsementCard, Gift, MemberGift, Membership, Notification, User, Viewer, VoiceKind,
    declined_visible_to,
};

use super::cards::{
    DeclineAction, accepted_endorsement_cards, catalog_name_options, declined_endorsement_cards,
    household_items, member_gift_cards, my_gift_cards, notice_cards, pending_endorsement_cards,
    unused_gift_options,
};
use super::draft::{EndorseDraft, GiftDraft, ProfileDraft, review_banner, voice_pass_input};
use super::flash::Flash;
use super::layout::{Nav, csrf_input, first_name, page, page_lead, rewrite_row};

pub fn member_show(
    viewer: &Viewer,
    person: &User,
    churches: &[(&Church, &Membership)],
    gifts: &[MemberGift],
    endorsements: &[EndorsementCard],
    declined: &[EndorsementCard],
    catalog: &[Gift],
    unread: i64,
    flash: Option<Flash>,
    csrf: &str,
    draft: &EndorseDraft<'_>,
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
            (page_lead(&person.name))
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
            (declined_endorsement_section(viewer, person, declined, csrf))
            (endorse_panel(viewer, person, catalog, csrf, draft))
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

fn households(churches: &[(&Church, &Membership)]) -> Markup {
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
                (accepted_endorsement_cards(endorsements))
            }
        }
    }
}

fn declined_endorsement_section(
    viewer: &Viewer,
    person: &User,
    declined: &[EndorsementCard],
    csrf: &str,
) -> Markup {
    let mut visible = declined_visible_to(&viewer.user.id, declined).peekable();
    if visible.peek().is_none() {
        return html! {};
    }
    html! {
        section {
            h2 { "Declined" }
            div class="stack" {
                (declined_endorsement_cards(
                    visible,
                    csrf,
                    decline_action_for(&viewer.user.id, &person.id),
                ))
            }
        }
    }
}

fn decline_action_for(viewer_id: &str, person_id: &str) -> DeclineAction {
    if viewer_id == person_id {
        DeclineAction::CanAccept
    } else {
        DeclineAction::Read
    }
}

fn endorse_panel(
    viewer: &Viewer,
    person: &User,
    catalog: &[Gift],
    csrf: &str,
    draft: &EndorseDraft<'_>,
) -> Markup {
    if viewer.user.id == person.id {
        return html! {};
    }
    html! {
        section class="panel" {
            h2 { "Endorse " (first_name(&person.name)) }
            form class="stack" method="post" action={ "/members/" (person.id) "/endorse" } {
                (csrf_input(csrf))
                (voice_pass_input(draft.kind))
                (review_banner(draft.kind))
                label { "Skill"
                    input name="skill" required maxlength="120" list="catalog-skills" placeholder="Hospitality" value=(draft.skill);
                    datalist id="catalog-skills" {
                        (catalog_name_options(catalog))
                    }
                }
                label { "Why"
                    textarea name="note" rows="5" required maxlength="600" placeholder="She stayed until the last parent came." { (draft.note) }
                    (rewrite_row(VoiceKind::Endorsement))
                }
                button class="btn" type="submit" { (draft.kind.submit_label("Send")) }
            }
        }
    }
}

pub fn inbox(
    viewer: &Viewer,
    pending: &[EndorsementCard],
    declined: &[EndorsementCard],
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
            (page_lead("Inbox"))
            (inbox_empty(pending, declined, notes))
            (pending_endorsements(pending, csrf))
            (inbox_declined(declined, csrf))
            (notice_section(notes))
        },
    )
}

fn inbox_empty(
    pending: &[EndorsementCard],
    declined: &[EndorsementCard],
    notes: &[Notification],
) -> Markup {
    if !pending.is_empty() || !declined.is_empty() || update_notes(notes).next().is_some() {
        return html! {};
    }
    html! {
        div class="empty" {
            p { "Nothing here yet." }
        }
    }
}

fn inbox_declined(declined: &[EndorsementCard], csrf: &str) -> Markup {
    if declined.is_empty() {
        return html! {};
    }
    html! {
        section {
            h2 { "Declined" }
            div class="stack" {
                (declined_endorsement_cards(declined, csrf, DeclineAction::CanAccept))
            }
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
    let mut visible = update_notes(notes).peekable();
    if visible.peek().is_none() {
        return html! {};
    }
    html! {
        section {
            h2 { "Updates" }
            div class="stack" {
                (notice_cards(visible))
            }
        }
    }
}

fn update_notes(notes: &[Notification]) -> impl Iterator<Item = &Notification> {
    notes.iter().filter(|note| !endorsement_prompt(note))
}

fn endorsement_prompt(note: &Notification) -> bool {
    note.kind == "endorsement" && note.href == "/inbox"
}

pub fn me(
    viewer: &Viewer,
    gifts: &[MemberGift],
    catalog: &[Gift],
    memberships: &[(&Church, &Membership)],
    unread: i64,
    flash: Option<Flash>,
    csrf: &str,
    profile: &ProfileDraft<'_>,
    gift: &GiftDraft<'_>,
) -> Markup {
    page(
        "You",
        Some(&viewer.user),
        unread,
        Nav::You,
        flash,
        csrf,
        html! {
            (page_lead(&viewer.user.name))
            section {
                h2 { "Profile" }
                form class="stack" method="post" action="/me" {
                    (csrf_input(csrf))
                    (voice_pass_input(profile.kind))
                    (review_banner(profile.kind))
                    label { "Name" input name="name" required autocomplete="name" autocapitalize="words" value=(profile.name) maxlength="80"; }
                    div class="split" {
                        label { "City" input name="city" required autocomplete="address-level2" maxlength="80" value=(profile.city); }
                        label { "State or region" input name="region" required autocomplete="address-level1" maxlength="80" value=(profile.region); }
                    }
                    label { "About you"
                        textarea name="bio" rows="3" maxlength="800" { (profile.bio) }
                        (rewrite_row(VoiceKind::Bio))
                    }
                    button class="btn" type="submit" { (profile.kind.submit_label("Save")) }
                }
            }
            section class="panel" {
                h2 { "Your gifts" }
                (my_gifts_block(gifts, csrf))
                form class="stack" method="post" action="/me/gifts" {
                    (csrf_input(csrf))
                    (voice_pass_input(gift.kind))
                    (review_banner(gift.kind))
                    label { "Add a gift"
                        select name="gift_id" required {
                            (unused_gift_options(catalog, gifts, gift.gift_id))
                        }
                    }
                    label { "Note (optional)"
                        input name="note" maxlength="600" placeholder="Thursday nights. Hospital visits." value=(gift.note);
                        (rewrite_row(VoiceKind::GiftNote))
                    }
                    button class="btn btn-quiet" type="submit" { (gift.kind.submit_label("Add")) }
                }
            }
            section {
                h2 { "Your churches" }
                (my_churches(memberships))
            }
            section class="panel" {
                h2 { "Alerts" }
                p class="muted" { "Banners for endorsements, and for needs that match your gifts." }
                p class="muted" { "On iPhone, add the app to your home screen first." }
                button type="button" class="btn" data-alerts { "Turn on alerts" }
                p class="muted" data-alerts-status hidden {}
            }
            form class="account-end" method="post" action="/session/logout" {
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

fn my_churches(memberships: &[(&Church, &Membership)]) -> Markup {
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
