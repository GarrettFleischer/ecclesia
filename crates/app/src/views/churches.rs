use maud::{Markup, html};

use ecclesia_sdk::prelude::{
    Church, ChurchCard, ChurchLinkStatus, ChurchMember, NeedCard, PlaceGroup, PrayerCard, Viewer,
    VoiceKind, visible_need_cards,
};
use ecclesia_sdk::story::ChurchSearchHit;

use super::cards::{
    NeedCardPlace, active_member_items, church_index_cards, has_active_member, need_card_stack,
    pending_member_cards, pending_people, place_sections,
};
use super::draft::{ChurchDraft, review_banner, voice_pass_input};
use super::flash::Flash;
use super::layout::{
    Icon, Monogram, Nav, OnboardStep, csrf_input, icon, join_href, monogram, onboard_steps, page,
    detail_lead, page_lead, rewrite_row, share_button,
};

pub fn churches_index(
    viewer: &Viewer,
    flash: Option<Flash>,
    churches: &[ChurchCard],
    next_cursor: Option<&str>,
    unread: i64,
    csrf: &str,
) -> Markup {
    page(
        "Churches",
        Some(&viewer.user),
        unread,
        Nav::Churches,
        flash,
        csrf,
        html! {
            div class="page-head" {
                div {
                    (page_lead("Churches"))
                }
                a class="btn btn-quiet" href="/churches/new" {
                    span class="btn-icon" aria-hidden="true" { (icon(Icon::Plus)) }
                    "Add your church"
                }
            }
            (church_list(churches))
            (super::more_churches("/churches", next_cursor))
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

pub fn church_new(
    viewer: &Viewer,
    unread: i64,
    flash: Option<Flash>,
    csrf: &str,
    draft: &ChurchDraft<'_>,
) -> Markup {
    page(
        "Add your church",
        Some(&viewer.user),
        unread,
        Nav::Churches,
        flash,
        csrf,
        html! {
            (page_lead("Add your church"))
            p class="muted" { "You'll be its pastor." }
            form class="stack" method="post" action="/churches" {
                (csrf_input(csrf))
                (voice_pass_input(draft.kind))
                (review_banner(draft.kind))
                label { "Church name" input name="name" required placeholder="Grace Covenant Church" maxlength="120" value=(draft.name); }
                label { "Address"
                    textarea name="address" rows="3" required maxlength="400" placeholder="100 Main Street" { (draft.address) }
                }
                div class="split" {
                    label { "Latitude" input name="latitude" required inputmode="decimal" placeholder="42.5349" value=(draft.latitude); }
                    label { "Longitude" input name="longitude" required inputmode="decimal" placeholder="-92.4453" value=(draft.longitude); }
                }
                label { "When you meet" input name="gathering" placeholder="Sundays, 10 a.m." maxlength="120" value=(draft.gathering); }
                label { "About the church"
                    textarea name="description" rows="4" required placeholder="A few sentences. Where you are, who comes, what you're about." { (draft.description) }
                    (rewrite_row(VoiceKind::Church))
                }
                button class="btn" type="submit" { (draft.kind.submit_label("Add church")) }
            }
        },
    )
}

pub fn church_show(
    viewer: &Viewer,
    church: &Church,
    members: &[ChurchMember],
    needs: &[NeedCard],
    answered: &[PrayerCard],
    next_need_cursor: Option<&str>,
    next_member_cursor: Option<&str>,
    flash: Option<Flash>,
    unread: i64,
    csrf: &str,
) -> Markup {
    let door = door_keep(viewer, church);
    let church_path = format!("/churches/{}", church.id);
    page(
        &church.name,
        Some(&viewer.user),
        unread,
        Nav::Churches,
        flash,
        csrf,
        html! {
            div class="profile-head" {
                (monogram(&church.id, &church.name, Monogram::ChurchLarge))
                div {
                    p class="eyebrow" { (church.address) }
                    (detail_lead(&church.name))
                }
            }
            p class="lede" { (church.description) }
            @if !church.gathering.is_empty() {
                p class="meta meta-line" {
                    span class="meta-icon" aria-hidden="true" { (icon(Icon::Clock)) }
                    (church.gathering)
                }
            }
            (membership_status(viewer, church, csrf))
            (governor_door(church, door, csrf))
            (people_section(&church.id, members, door, csrf, &church_path, next_member_cursor))
            (needs_section(viewer, church, needs, &church_path, next_need_cursor))
            (answered_section(answered))
        },
    )
}

fn answered_section(prayers: &[PrayerCard]) -> Markup {
    if prayers.is_empty() {
        return html! {};
    }
    html! {
        section {
            h2 { "Answered prayers" }
            div class="stack" {
                @for prayer in prayers {
                    article class="card" {
                        p { (prayer.body) }
                        @if let Some(name) = &prayer.author_name {
                            @if let Some(author_id) = &prayer.author_id {
                                p class="meta" {
                                    a href={ "/members/" (author_id) } { (name) }
                                }
                            }
                        }
                        @if let Some(praise) = &prayer.praise {
                            p { (praise) }
                        }
                    }
                }
            }
        }
    }
}

fn membership_status(viewer: &Viewer, church: &Church, csrf: &str) -> Markup {
    if viewer.user.church_id.as_deref() != Some(church.id.as_str()) {
        return ask_to_join(church, csrf);
    }
    match viewer.user.link_status() {
        Some(ChurchLinkStatus::Active) => {
            html! { p class="pill" { (role_line(viewer.user.church_role.as_deref().unwrap_or("member"))) } }
        }
        Some(ChurchLinkStatus::Pending) => {
            html! { p class="pill pill-wait" { "Your request to join " (church.name) " has been sent." } }
        }
        Some(ChurchLinkStatus::Invited) => html! {
            form method="post" action="/churches/join/accept" {
                (csrf_input(csrf))
                button class="btn" type="submit" { "Accept invite" }
            }
        },
        None => ask_to_join(church, csrf),
    }
}

fn ask_to_join(church: &Church, csrf: &str) -> Markup {
    html! {
        form method="post" action={ "/churches/" (church.id) "/join" } {
            (csrf_input(csrf))
            button class="btn" type="submit" { "Ask to join" }
        }
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
            p class="muted" { "Share the code, or send it by email." }
            div class="share-row" {
                p class="code" { (church.invite_code) }
                (share_button(
                    "Share code",
                    &church.name,
                    &format!("Join {} on Ecclesia. Code: {}", church.name, church.invite_code),
                    &join_href(&church.invite_code),
                ))
            }
            form class="row-form" method="post" action={ "/churches/" (church.id) "/invite" } {
                (csrf_input(csrf))
                label { "Email"
                    input type="email" name="email" required placeholder="name@church.org" maxlength="120";
                }
                button class="btn btn-quiet" type="submit" { "Send invite" }
            }
        }
    }
}

fn people_section(
    church_id: &str,
    members: &[ChurchMember],
    door: DoorKeep,
    csrf: &str,
    church_path: &str,
    next_cursor: Option<&str>,
) -> Markup {
    html! {
        section {
            h2 { "People" }
            (pending_people_block(church_id, members, door, csrf))
            ul class="people" {
                (active_member_items(members))
            }
            (no_active_members(members))
            (super::more_people(church_path, next_cursor))
        }
    }
}

fn pending_people_block(
    church_id: &str,
    members: &[ChurchMember],
    door: DoorKeep,
    csrf: &str,
) -> Markup {
    if !matches!(door, DoorKeep::Keeps) {
        return html! {};
    }
    let mut pending = pending_people(members).peekable();
    if pending.peek().is_none() {
        return html! {};
    }
    html! {
        div class="stack" {
            (pending_member_cards(church_id, pending, csrf))
        }
    }
}

fn no_active_members(members: &[ChurchMember]) -> Markup {
    if has_active_member(members) {
        return html! {};
    }
    html! { p class="muted" { "No members yet." } }
}

fn needs_section(
    viewer: &Viewer,
    church: &Church,
    needs: &[NeedCard],
    church_path: &str,
    next_cursor: Option<&str>,
) -> Markup {
    html! {
        section {
            div class="toolbar" {
                h2 { "Needs" }
                @if viewer.is_active_in(&church.id) {
                    a class="btn btn-quiet" href={ "/needs/new?church_id=" (church.id) } {
                        span class="btn-icon" aria-hidden="true" { (icon(Icon::Plus)) }
                        "Post a need"
                    }
                }
            }
            (church_needs(needs, viewer, church))
            (super::more_needs(church_path, next_cursor))
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
    need_card_stack(visible, viewer, NeedCardPlace::Church)
}

pub fn the_body(
    viewer: &Viewer,
    groups: &[PlaceGroup],
    next_cursor: Option<&str>,
    unread: i64,
    csrf: &str,
) -> Markup {
    page(
        "Churches nearby",
        Some(&viewer.user),
        unread,
        Nav::Body,
        None,
        csrf,
        html! {
            (page_lead("Churches nearby"))
            (body_groups(groups))
            (super::more_churches("/the-body", next_cursor))
        },
    )
}

pub fn join_church_page(
    viewer: &Viewer,
    flash: Option<Flash>,
    unread: i64,
    csrf: &str,
    hits: &[ChurchSearchHit],
    query: &str,
    lat: &str,
    lng: &str,
) -> Markup {
    let nav = match viewer.user.church_id {
        Some(_) => Nav::Churches,
        None => Nav::Join,
    };
    page(
        "Find your church",
        Some(&viewer.user),
        unread,
        nav,
        flash,
        csrf,
        html! {
            @if viewer.user.church_id.is_none() {
                (onboard_steps(OnboardStep::Church))
            }
            (page_lead("Find your church"))
            a class="btn btn-quiet" href="/churches/new" { "Add your church" }
            section {
                h2 { "Name or city" }
                form class="stack" method="get" action="/churches/join" data-join-finder {
                    input type="hidden" name="lat" value=(lat);
                    input type="hidden" name="lng" value=(lng);
                    div class="search-line" {
                        label { "Name or city"
                            input type="search" name="q" value=(query) maxlength="120" placeholder="Cedar Falls" autocomplete="off" data-join-query;
                        }
                        button class="btn btn-quiet scan-btn" type="button" data-scan-code aria-label="Scan church code" {
                            span class="btn-icon" aria-hidden="true" { (icon(Icon::Qr)) }
                        }
                    }
                }
                p class="scan-note" data-scan-status role="status" {}
                div class="scan-sheet" data-scan-sheet hidden {
                    video data-scan-video autoplay playsinline muted {}
                    button class="btn" type="button" data-scan-close { "Close" }
                }
                div class="join-results" data-join-results aria-live="polite" {
                    (join_results(hits, query, lat, lng, csrf))
                }
            }
            @if viewer.user.church_id.is_none() {
                form class="account-end" method="post" action="/session/logout" {
                    (csrf_input(csrf))
                    button class="btn btn-quiet" type="submit" { "Sign out" }
                }
            }
            script src="/static/join.js?v=5" defer {}
        },
    )
}

fn join_results(
    hits: &[ChurchSearchHit],
    query: &str,
    lat: &str,
    lng: &str,
    csrf: &str,
) -> Markup {
    let named = !query.trim().is_empty();
    let located = !lat.trim().is_empty() && !lng.trim().is_empty();
    if hits.is_empty() {
        if named {
            return html! { p class="muted" { "No churches match." } };
        }
        if located {
            return html! { p class="muted" { "No churches nearby." } };
        }
        return html! {};
    }
    html! {
        @if !named && located {
            h2 { "Nearby" }
        }
        div class="stack" {
            @for hit in hits {
                (join_hit(hit, csrf))
            }
        }
    }
}

fn join_hit(hit: &ChurchSearchHit, csrf: &str) -> Markup {
    html! {
        article class="card" {
            h3 { (hit.name) }
            p class="muted" { (hit.address) }
            form method="post" action="/churches/join" {
                (csrf_input(csrf))
                input type="hidden" name="church_id" value=(hit.id);
                button class="btn" type="submit" { "Ask to join" }
            }
        }
    }
}

fn body_groups(groups: &[PlaceGroup]) -> Markup {
    if groups.is_empty() {
        return html! {
            div class="empty" { p { "No churches yet." } }
        };
    }
    place_sections(groups)
}
