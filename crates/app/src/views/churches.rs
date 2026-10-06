use maud::{Markup, html};

use ecclesia_sdk::db::ClosedNeedGroup;
use ecclesia_sdk::prelude::{
    Church, ChurchCard, ChurchLinkStatus, ChurchMember, NeedCard, PlaceGroup, PrayerCard,
    US_STATES, Viewer, VoiceKind, WEEKDAYS, state_label, visible_church_need_cards,
};
use ecclesia_sdk::story::ChurchSearchHit;

use super::cards::{
    NeedCardPlace, active_member_items, church_index_cards, has_active_member,
    movable_needs_section, need_card_stack, pending_member_cards, pending_people, place_sections,
};
use super::draft::{ChurchDraft, review_banner, voice_pass_input};
use super::flash::Flash;
use super::layout::{
    Icon, Monogram, Nav, OnboardStep, csrf_input, detail_lead, icon, join_href, monogram,
    onboard_steps, page, page_lead, rewrite_row,
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
                    "Register a new church"
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
        "Register a church",
        Some(&viewer.user),
        unread,
        Nav::Churches,
        flash,
        csrf,
        html! {
            (page_lead("Register a church"))
            p class="muted" { "You'll be its pastor." }
            form class="stack" method="post" action="/churches" {
                (csrf_input(csrf))
                (voice_pass_input(draft.kind))
                (review_banner(draft.kind))
                label for="organization" { "Church name"
                    input id="organization" name="name" autocomplete="organization" required placeholder="Grace Covenant Church" maxlength="120" value=(draft.name);
                }
                (address_fields(draft))
                label for="ein" { "Employer identification number"
                    input id="ein" name="ein" autocomplete="off" required placeholder="12-3456789" maxlength="20" value=(draft.ein);
                }
                label for="registry-state" { "Registration state"
                    select id="registry-state" name="registry_state" autocomplete="off" required {
                        (state_options(draft.registry_state))
                    }
                }
                label { "State registration number"
                    input name="registry_number" required placeholder="123456" maxlength="40" value=(draft.registry_number);
                }
                (service_fields(draft))
                label { "About the church"
                    textarea name="description" rows="4" required placeholder="A few sentences. Where you are, who comes, what you're about." { (draft.description) }
                    (rewrite_row(VoiceKind::Church))
                }
                button class="btn" type="submit" { (draft.kind.submit_label("Register a new church")) }
            }
        },
    )
}

fn address_fields(draft: &ChurchDraft<'_>) -> Markup {
    html! {
        input type="hidden" id="country" name="country" autocomplete="country" value="US";
        label for="address-line1" { "Line 1"
            input id="address-line1" name="address-line1" autocomplete="address-line1" required maxlength="100" placeholder="100 Main Street" value=(draft.address_line1);
        }
        label for="address-line2" { "Line 2"
            input id="address-line2" name="address-line2" autocomplete="address-line2" maxlength="100" placeholder="Suite 2" value=(draft.address_line2);
        }
        div class="address-grid" {
            label for="address-level2" { "City"
                input id="address-level2" name="address-level2" autocomplete="address-level2" required maxlength="80" placeholder="Cedar Falls" value=(draft.city);
            }
            label for="address-level1" { "State"
                select id="address-level1" name="address-level1" autocomplete="address-level1" required {
                    (state_options(draft.address_state))
                }
            }
            label for="postal-code" { "ZIP"
                input id="postal-code" name="postal-code" autocomplete="postal-code" required maxlength="10" placeholder="50613" value=(draft.postal_code);
            }
        }
    }
}

fn service_fields(draft: &ChurchDraft<'_>) -> Markup {
    let count = shown_services(&draft.service_days, &draft.service_times);
    html! {
        fieldset class="service-times" data-service-times {
            legend { "Service times" }
            div class="service-list-fields" data-service-list {
                @for index in 0..count {
                    (service_row(
                        index,
                        draft.service_days[index],
                        draft.service_times[index],
                        remove_for(count),
                    ))
                }
            }
            button class="btn btn-quiet" type="button" data-service-add { "Add a service" }
            template id="service-row" {
                (service_row(0, "", "", RemoveControl::Shown))
            }
        }
    }
}

fn shown_services(days: &[&str], times: &[&str]) -> usize {
    let mut count = 0;
    for index in 0..days.len() {
        let day = days.get(index).copied().unwrap_or("");
        let time = times.get(index).copied().unwrap_or("");
        if !day.is_empty() || !time.is_empty() {
            count = index + 1;
        }
    }
    count.max(1)
}

enum RemoveControl {
    Hidden,
    Shown,
}

fn remove_for(count: usize) -> RemoveControl {
    if count == 1 {
        RemoveControl::Hidden
    } else {
        RemoveControl::Shown
    }
}

fn service_row(index: usize, day: &str, time: &str, remove: RemoveControl) -> Markup {
    let hidden = matches!(remove, RemoveControl::Hidden);
    let day_name = format!("service_day_{index}");
    let time_name = format!("service_time_{index}");
    html! {
        div class="service-row" data-service-row {
            label { "Day"
                select name=(day_name) autocomplete="off" {
                    (day_options(day))
                }
            }
            label { "Time"
                input type="time" name=(time_name) step="60" value=(time);
            }
            button class="btn btn-quiet" type="button" data-service-remove hidden[hidden] { "Remove service" }
        }
    }
}

fn day_options(selected: &str) -> Markup {
    html! {
        option value="" selected[selected.is_empty()] {}
        @for day in WEEKDAYS {
            option value=(day) selected[day == selected] { (day) }
        }
    }
}

fn state_options(selected: &str) -> Markup {
    html! {
        option value="" disabled selected[selected.is_empty()] {}
        @for &(code, name) in US_STATES {
            option value=(code) selected[code == selected] { (name) }
        }
    }
}

pub fn church_show(
    viewer: &Viewer,
    church: &Church,
    members: &[ChurchMember],
    needs: &[NeedCard],
    answered: &[PrayerCard],
    next_need_cursor: Option<&str>,
    next_member_cursor: Option<&str>,
    movable: &[ClosedNeedGroup],
    flash: Option<Flash>,
    unread: i64,
    csrf: &str,
    origin: &str,
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
            (church_return_hold(&church.id))
            div class="profile-head" {
                (monogram(&church.id, &church.name, Monogram::ChurchLarge))
                div {
                    p class="eyebrow" { (church.address) }
                    (detail_lead(&church.name))
                }
            }
            p class="lede" { (church.description) }
            @if !church.gathering.is_empty() {
                div class="meta meta-line service-when" {
                    span class="meta-icon" aria-hidden="true" { (icon(Icon::Clock)) }
                    ul class="service-list" {
                        @for line in church.gathering.lines() {
                            @if !line.trim().is_empty() {
                                li { (line) }
                            }
                        }
                    }
                }
            }
            (membership_status(viewer, church, csrf))
            (movable_needs_section(movable, viewer, csrf))
            (governor_door(church, door, csrf, origin))
            (people_section(&church.id, members, door, csrf, &church_path, next_member_cursor))
            (needs_section(viewer, church, needs, &church_path, next_need_cursor))
            (answered_section(answered))
            (church_return_place())
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
    let Some(link) = viewer.user.membership_in(&church.id) else {
        return ask_to_join(church, csrf);
    };
    match link.status() {
        Some(ChurchLinkStatus::Active) => {
            html! { p class="pill" { (role_line(link.role.as_str())) } }
        }
        Some(ChurchLinkStatus::Pending) => {
            html! { p class="pill pill-wait" { "Your request to join " (church.name) " has been sent." } }
        }
        Some(ChurchLinkStatus::Invited) => html! {
            form method="post" action="/churches/join/accept" {
                (csrf_input(csrf))
                input type="hidden" name="church_id" value=(church.id);
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

fn registration_note(church: &Church) -> Markup {
    if church.ein.is_empty() {
        return html! {};
    }
    html! {
        p class="meta" { "EIN " (church.ein) }
        @if !church.registry_number.is_empty() {
            p class="meta" { (state_label(church.registry_state.as_str())) " registration " (church.registry_number) }
        }
    }
}

fn governor_door(church: &Church, door: DoorKeep, csrf: &str, origin: &str) -> Markup {
    if !matches!(door, DoorKeep::Keeps) {
        return html! {};
    }
    html! {
        section class="panel" {
            (registration_note(church))
            h2 { "Invite people" }
            (invite_sheet(church, origin, csrf))
        }
    }
}

fn invite_sheet(church: &Church, origin: &str, csrf: &str) -> Markup {
    let url = join_url(origin, &church.invite_code);
    html! {
        div class="invite-sheet" data-invite-sheet data-church-name=(church.name) data-join=(url) {
            (invite_qr(&url))
            div class="invite-actions" {
                button class="btn btn-quiet invite-tool" type="button" data-print-qr aria-label="Print code" {
                    span class="btn-icon" aria-hidden="true" { (icon(Icon::Print)) }
                }
                button class="btn btn-quiet invite-tool" type="button" data-invite-email aria-label="Email" {
                    span class="btn-icon" aria-hidden="true" { (icon(Icon::Mail)) }
                }
            }
            (invite_mail_dialog(church, csrf))
        }
    }
}

fn invite_mail_dialog(church: &Church, csrf: &str) -> Markup {
    let action = format!("/churches/{}/invite", church.id);
    html! {
        dialog class="invite-mail" data-invite-mail aria-labelledby="invite-mail-title" {
            form class="invite-mail-sheet" method="post" action=(action) {
                (csrf_input(csrf))
                div class="invite-mail-head" {
                    h2 id="invite-mail-title" { "Send invite" }
                    button class="btn btn-quiet" type="button" data-invite-close { "Close" }
                }
                div class="invite-drop" data-invite-drop {
                    p class="meta" { "Drop a CSV with an Email column, or open a file." }
                    button class="btn btn-quiet" type="button" data-invite-file { "Open file" }
                    input class="invite-csv" type="file" accept=".csv,text/csv" data-invite-csv tabindex="-1";
                    p class="meta invite-file-error" data-invite-csv-error hidden aria-live="polite" {}
                }
                div class="invite-rows" data-invite-rows {
                    (invite_email_row())
                }
                button class="btn" type="submit" { "Send invite" }
            }
        }
    }
}

fn invite_email_row() -> Markup {
    html! {
        label data-invite-row {
            "Email"
            input type="email" name="email" maxlength="120" placeholder="name@church.org" autocomplete="email";
        }
    }
}

fn join_url(origin: &str, code: &str) -> String {
    format!("{}{}", origin.trim_end_matches('/'), join_href(code))
}

fn invite_qr(payload: &str) -> Markup {
    let Ok(code) = qrcode::QrCode::new(payload.as_bytes()) else {
        return html! {};
    };
    let modules = code.width();
    let quiet = 4usize;
    let span = modules + quiet * 2;
    let mut marks = String::new();
    for y in 0..modules {
        for x in 0..modules {
            if code[(x, y)] != qrcode::Color::Dark {
                continue;
            }
            marks.push_str(&format!(
                r#"<rect x="{}" y="{}" width="1" height="1"/>"#,
                x + quiet,
                y + quiet
            ));
        }
    }
    let svg = format!(
        r##"<svg class="invite-qr" viewBox="0 0 {span} {span}" role="img" aria-label="Join code" fill="#102018"><rect width="{span}" height="{span}" fill="#fff"/>{marks}</svg>"##
    );
    html! { (maud::PreEscaped(svg)) }
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
        section data-church-needs=(church.id) {
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
    let mut visible =
        visible_church_need_cards(viewer, needs, std::slice::from_ref(church)).peekable();
    if visible.peek().is_none() {
        return html! {
            p class="muted" { "No needs." }
        };
    }
    need_card_stack(visible, viewer, NeedCardPlace::Church)
}

fn church_return_hold(church_id: &str) -> Markup {
    html! {
        div data-church-return=(church_id) hidden {}
        script { (maud::PreEscaped(CHURCH_RETURN_HOLD)) }
    }
}

fn church_return_place() -> Markup {
    html! { script { (maud::PreEscaped(CHURCH_RETURN_PLACE)) } }
}

const CHURCH_RETURN_HOLD: &str = r#"(function () {
  var root = document.querySelector("[data-church-return]");
  if (!root) return;
  var params = new URLSearchParams(window.location.search);
  if (params.get("return") !== "1") return;
  var raw = null;
  try { raw = sessionStorage.getItem("ecclesia.needReturn"); } catch (error) { return; }
  if (!raw) return;
  var saved = null;
  try { saved = JSON.parse(raw); } catch (error) { return; }
  if (!saved || saved.churchId !== root.getAttribute("data-church-return")) return;
  document.documentElement.classList.add("need-return");
  if (window.history && "scrollRestoration" in window.history) {
    window.history.scrollRestoration = "manual";
  }
  window.setTimeout(function () {
    document.documentElement.classList.remove("need-return");
  }, 800);
})();"#;

const CHURCH_RETURN_PLACE: &str = r#"(function () {
  var params = new URLSearchParams(window.location.search);
  if (params.get("return") !== "1") return;
  var root = document.querySelector("[data-church-needs]");
  var raw = null;
  try { raw = sessionStorage.getItem("ecclesia.needReturn"); } catch (error) { raw = null; }
  var saved = null;
  if (raw) {
    try { saved = JSON.parse(raw); } catch (error) { saved = null; }
  }
  if (root && saved && saved.churchId === root.getAttribute("data-church-needs")) {
    var needId = typeof saved.needId === "string" && /^[A-Za-z0-9_-]+$/.test(saved.needId) ? saved.needId : "";
    var card = needId ? document.getElementById("need-" + needId) : null;
    if (typeof saved.scrollY === "number" && isFinite(saved.scrollY)) {
      window.scrollTo(0, saved.scrollY);
    } else if (card && card.scrollIntoView) {
      card.scrollIntoView({ block: "center", behavior: "instant" });
    }
    if (saved.flash && card) {
      card.classList.add("is-met");
      window.setTimeout(function () { card.classList.remove("is-met"); }, 1600);
    }
    try { sessionStorage.removeItem("ecclesia.needReturn"); } catch (error) {}
  }
  document.documentElement.classList.remove("need-return");
  params.delete("return");
  var next = window.location.pathname;
  var query = params.toString();
  if (query) next += "?" + query;
  if (window.location.hash) next += window.location.hash;
  window.history.replaceState(window.history.state, "", next);
})();"#;

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
    movable: &[ClosedNeedGroup],
) -> Markup {
    let nav = if viewer.user.has_church() {
        Nav::Churches
    } else {
        Nav::Join
    };
    page(
        "Find your church",
        Some(&viewer.user),
        unread,
        nav,
        flash,
        csrf,
        html! {
            @if !viewer.user.has_church() {
                (onboard_steps(OnboardStep::Church))
            }
            (page_lead("Find your church"))
            (movable_needs_section(movable, viewer, csrf))
            a class="btn btn-quiet" href="/churches/new" { "Register a new church" }
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
            @if !viewer.user.has_church() {
                form class="account-end" method="post" action="/session/logout" {
                    (csrf_input(csrf))
                    button class="btn btn-quiet" type="submit" { "Sign out" }
                }
            }
            script src="/static/join.js?v=6" defer {}
        },
    )
}

fn join_results(hits: &[ChurchSearchHit], query: &str, lat: &str, lng: &str, csrf: &str) -> Markup {
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
