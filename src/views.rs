use maud::{html, Markup, DOCTYPE};

use crate::domain::{
    Church, ChurchMember, EndorsementCard, Gift, MemberGift, Membership, NeedCard, NeedScope,
    Notification, User, Viewer,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Home,
    Churches,
    Body,
    Inbox,
    You,
    None,
}

pub fn page(
    title: &str,
    user: Option<&User>,
    unread: i64,
    nav: Nav,
    flash: Option<(bool, String)>,
    main: Markup,
) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover";
                title { (title) " · Ecclesia" }
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin;
                link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Fraunces:opsz,wght@9..144,500;9..144,640&family=Source+Sans+3:ital,wght@0,400;0,600;1,400&display=swap";
                link rel="stylesheet" href="/static/app.css";
            }
            body {
                @if nav != Nav::None {
                    header class="topbar" {
                        a class="mark" href="/home" { span { "Ecclesia" } }
                        @if let Some(user) = user {
                            a class="who" href="/me" { (user.name) }
                        }
                    }
                }
                @if let Some((ok, text)) = flash {
                    div class={ "flash " (if ok { "flash-ok" } else { "flash-err" }) } { (text) }
                }
                main class={ "sheet" (if nav == Nav::None { " sheet-wide" } else { "" }) } {
                    (main)
                }
                @if nav != Nav::None {
                    nav class="dock" aria-label="Primary" {
                        (dock_link("/", "Home", nav == Nav::Home, None))
                        (dock_link("/churches", "Churches", nav == Nav::Churches, None))
                        (dock_link("/the-body", "The body", nav == Nav::Body, None))
                        (dock_link("/inbox", "Inbox", nav == Nav::Inbox, if unread > 0 { Some(unread) } else { None }))
                        (dock_link("/me", "You", nav == Nav::You, None))
                    }
                }
            }
        }
    }
}

fn dock_link(href: &str, label: &str, active: bool, badge: Option<i64>) -> Markup {
    html! {
        a class={ "dock-link" (if active { " is-active" } else { "" }) } href=(href) {
            span { (label) }
            @if let Some(n) = badge {
                span class="badge" { (n) }
            }
        }
    }
}

pub fn flash_from(ok: Option<String>, err: Option<String>) -> Option<(bool, String)> {
    if let Some(code) = err {
        Some((false, flash_err(&code)))
    } else {
        ok.map(|code| (true, flash_ok(&code)))
    }
}

fn flash_ok(code: &str) -> String {
    match code {
        "welcome" => "You have a place at the table.".into(),
        "joined_request" => "Your request is with the pastor. They will let you in.".into(),
        "invited" => "The invite is waiting for them.".into(),
        "redeemed" => "This church invited you. Accept it below, or from your home.".into(),
        "approved" => "They are in. The body just got a little less thin.".into(),
        "declined" => "Recorded. No one is left guessing.".into(),
        "need_posted" => "The need is visible to the people you chose.".into(),
        "applied" => "They will see that you can help.".into(),
        "application_accepted" => "Good. Someone is actually coming.".into(),
        "need_closed" => "This need is closed.".into(),
        "endorsed" => "They will decide whether to wear that word.".into(),
        "endorsement_accepted" => "That gift is now on your life in this church.".into(),
        "endorsement_declined" => "You let it go. That is allowed.".into(),
        "gift_added" => "Named. People can find you by it.".into(),
        "gift_removed" => "Removed from your list.".into(),
        "saved" => "Saved.".into(),
        "church_planted" => "The group exists. Invite the first people.".into(),
        "invite_accepted" => "You are in. Look around for who needs you.".into(),
        other => other.to_string(),
    }
}

fn flash_err(code: &str) -> String {
    match code {
        "missing" => "A few required fields are empty.".into(),
        "email" => "That email is already at the table. Switch into that person instead.".into(),
        "auth" => "Sign in first.".into(),
        "not_found" => "We could not find that.".into(),
        "forbidden" => "That is not yours to decide.".into(),
        "self" => "You cannot do that for yourself.".into(),
        "already" => "That is already in motion.".into(),
        "not_member" => "You need an approved place in a church first.".into(),
        "scope" => "This need is not open to you yet.".into(),
        "own_need" => "You posted this need. Wait for someone else.".into(),
        "closed" => "That need is no longer open.".into(),
        "invite" => "That invite code does not match a church.".into(),
        "pending" => "There is nothing pending to decide.".into(),
        other => other.to_string(),
    }
}

pub fn landing(users: &[User], flash: Option<(bool, String)>) -> Markup {
    page(
        "The body, together",
        None,
        0,
        Nav::None,
        flash,
        html! {
            section class="hero" {
                p class="eyebrow" { "For churches who refuse to be islands" }
                h1 { "The ecclesia is a people, not a campus." }
                p class="lede" {
                    "Each church keeps its own household. Members ask or are invited in, and a pastor approves. "
                    "People name their gifts. Needs are posted in the open. Whoever can help, offers. "
                    "Neighboring churches can see what the next parish cannot carry alone."
                }
            }
            section class="panel" {
                h2 { "Walk through the Cedar Falls valley" }
                p class="muted" {
                    "This first slice has no passwords. Enter as someone already in the story — pastor, member, neighbor, or the person still waiting in the doorway."
                }
                div class="persona-grid" {
                    @for user in users {
                        form method="post" action="/session" {
                            input type="hidden" name="user_id" value=(user.id);
                            button class="persona" type="submit" {
                                strong { (user.name) }
                                span { (persona_line(user)) }
                            }
                        }
                    }
                }
            }
            section class="panel" {
                h2 { "Or take your own seat" }
                form class="stack" method="post" action="/register" {
                    label { "Name" input name="name" required placeholder="Your name"; }
                    label { "Email" input type="email" name="email" required placeholder="you@church.org"; }
                    div class="split" {
                        label { "City" input name="city" required placeholder="Cedar Falls"; }
                        label { "Region" input name="region" required placeholder="Iowa"; }
                    }
                    label { "How do you serve?"
                        textarea name="bio" rows="3" placeholder="The gifts you already practice, even if no one has ordained them." {}
                    }
                    button class="btn" type="submit" { "Create my place" }
                }
            }
        },
    )
}

fn persona_line(user: &User) -> &'static str {
    match user.id.as_str() {
        "user_miriam" => "Pastor · Grace Covenant · approves members",
        "user_daniel" => "Member · posted a neighboring repair need",
        "user_ruth" => "Member · has a hospitality endorsement waiting",
        "user_samuel" => "Rector · St. Luke's · asking neighbors for worship",
        "user_james" => "Steward · carpenter from St. Luke's",
        "user_keisha" => "Pastor · New Mercy · Waterloo, same valley",
        "user_elena" => "Member · translator who can cross the river",
        "user_peter" => "Still waiting · requested Grace Covenant",
        _ => "Member of the body",
    }
}

pub fn home(
    viewer: &Viewer,
    flash: Option<(bool, String)>,
    pending: &[(Membership, Church)],
    needs: &[NeedCard],
    unread: i64,
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
            @if !viewer.is_active_anywhere() && pending.is_empty() {
                div class="empty" {
                    p { "You are not yet in a church household. Ask to join one, redeem an invite, or plant a group if you are the pastor who will keep it." }
                    a class="btn" href="/churches" { "Find a church" }
                }
            }
            @if !pending.is_empty() {
                section class="panel" {
                    h2 { "At the door" }
                    @for (membership, church) in pending {
                        article class="card" {
                            @if membership.status == "pending_request" {
                                p { "You asked to join " a href={ "/churches/" (church.id) } { (church.name) } ". A pastor still has to open the door." }
                            } @else {
                                p { (church.name) " invited you." }
                                form method="post" action={ "/memberships/" (membership.id) "/accept-invite" } {
                                    button class="btn" type="submit" { "Accept and come in" }
                                }
                            }
                        }
                    }
                }
            }
            @if viewer.is_active_anywhere() {
                div class="toolbar" {
                    a class="btn" href="/needs/new" { "Post a need" }
                    a class="btn btn-quiet" href="/the-body" { "See neighboring churches" }
                }
            }
            section {
                h2 { "Needs the body can carry" }
                @if needs.is_empty() {
                    div class="empty" {
                        p { "No open needs you can see. That can mean rest — or that a church is still trying to carry everything alone." }
                    }
                }
                div class="stack" {
                    @for need in needs {
                        (need_card(need, viewer))
                    }
                }
            }
        },
    )
}

pub fn churches_index(
    viewer: &Viewer,
    flash: Option<(bool, String)>,
    churches: &[(Church, i64, i64)],
    unread: i64,
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
                label { "Invite code"
                    input name="code" placeholder="grace-k2m9" autocomplete="off";
                }
                button class="btn btn-quiet" type="submit" { "Redeem" }
            }
            @if churches.is_empty() {
                div class="empty" { p { "No churches yet. The first pastor has to plant one." } }
            }
            div class="stack" {
                @for (church, members, needs) in churches {
                    a class="card card-link" href={ "/churches/" (church.id) } {
                        h3 { (church.name) }
                        p class="muted" { (church.city) ", " (church.region) }
                        p { (church.description) }
                        p class="meta" { (members) " members · " (needs) " open needs" }
                    }
                }
            }
        },
    )
}

pub fn church_new(viewer: &Viewer, unread: i64, flash: Option<(bool, String)>) -> Markup {
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
                label { "Church name" input name="name" required placeholder="Grace Covenant Church"; }
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
    flash: Option<(bool, String)>,
    unread: i64,
) -> Markup {
    let mine = viewer.membership_in(&church.id);
    let governor = viewer.can_govern(&church.id);
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
            @if let Some(membership) = mine {
                @match membership.status.as_str() {
                    "active" => {
                        p class="pill" { "You belong here as " (membership.role) }
                    }
                    "pending_request" => {
                        p class="pill pill-wait" { "Your request is waiting on a pastor" }
                    }
                    "pending_invite" => {
                        form method="post" action={ "/memberships/" (membership.id) "/accept-invite" } {
                            button class="btn" type="submit" { "Accept this church's invite" }
                        }
                    }
                    "declined" => {
                        p class="pill pill-warn" { "A previous request was declined" }
                    }
                    _ => {}
                }
            } @else {
                form method="post" action={ "/churches/" (church.id) "/join" } {
                    button class="btn" type="submit" { "Ask to join" }
                }
            }
            @if governor {
                section class="panel" {
                    h2 { "Keep the door" }
                    p class="muted" { "Share this invite code. They still confirm; you already chose them. Or invite someone already in Ecclesia by email." }
                    p class="code" { (church.invite_code) }
                    form class="row-form" method="post" action={ "/churches/" (church.id) "/invite" } {
                        label { "Invite by email"
                            input type="email" name="email" required placeholder="james@stlukes.test";
                        }
                        button class="btn btn-quiet" type="submit" { "Invite" }
                    }
                }
            }
            section {
                h2 { "People" }
                @let pending_people: Vec<_> = members.iter().filter(|m| m.status == "pending_request" || m.status == "pending_invite").collect();
                @if governor && !pending_people.is_empty() {
                    div class="stack" {
                        @for member in &pending_people {
                            article class="card" {
                                a href={ "/members/" (member.user_id) } { strong { (member.name) } }
                                p class="muted" {
                                    @if member.status == "pending_request" { "Asked to join" } @else { "Invited — waiting on them" }
                                }
                                @if member.status == "pending_request" {
                                    div class="row" {
                                        form method="post" action={ "/memberships/" (member.membership_id) "/approve" } {
                                            button class="btn" type="submit" { "Approve" }
                                        }
                                        form method="post" action={ "/memberships/" (member.membership_id) "/decline" } {
                                            button class="btn btn-quiet" type="submit" { "Decline" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                ul class="people" {
                    @for member in members.iter().filter(|m| m.status == "active") {
                        li {
                            a href={ "/members/" (member.user_id) } { (member.name) }
                            span class="muted" { " · " (member.role) }
                        }
                    }
                }
                @if members.iter().all(|m| m.status != "active") {
                    p class="muted" { "No approved members yet." }
                }
            }
            section {
                div class="toolbar" {
                    h2 { "Needs" }
                    @if viewer.is_active_in(&church.id) {
                        a class="btn btn-quiet" href={ "/needs/new?church_id=" (church.id) } { "Post a need" }
                    }
                }
                @if needs.is_empty() {
                    p class="muted" { "No needs posted. Either they are between crises, or they have not learned to ask." }
                }
                div class="stack" {
                    @for need in needs {
                        (need_card(need, viewer))
                    }
                }
            }
        },
    )
}

pub fn need_new(
    viewer: &Viewer,
    churches: &[Church],
    gifts: &[Gift],
    selected_church: Option<&str>,
    unread: i64,
    flash: Option<(bool, String)>,
) -> Markup {
    page(
        "Post a need",
        Some(&viewer.user),
        unread,
        Nav::Home,
        flash,
        html! {
            h1 { "What does the body need?" }
            p class="muted" {
                "Keep it inside your church, open it to neighboring households in the same city or region, or ask the whole ecclesia."
            }
            @if churches.is_empty() {
                div class="empty" {
                    p { "You can only post from a church where you are an approved member." }
                    a class="btn" href="/churches" { "Find a church" }
                }
            } @else {
                form class="stack" method="post" action="/needs" {
                    label { "Church"
                        select name="church_id" required {
                            @for church in churches {
                                option value=(church.id) selected[selected_church == Some(church.id.as_str())] { (church.name) }
                            }
                        }
                    }
                    label { "Title" input name="title" required placeholder="Meal train for the Okonkwo family"; }
                    label { "The actual need"
                        textarea name="body" rows="5" required placeholder="When, where, what kind of help, and what would be too much." {}
                    }
                    label { "Primary gift you are hoping for"
                        select name="gift_id" {
                            option value="" { "Any willing hands" }
                            @for gift in gifts {
                                option value=(gift.id) { (gift.name) " · " (gift.category) }
                            }
                        }
                    }
                    fieldset class="scopes" {
                        legend { "Who can see this" }
                        label class="choice" {
                            input type="radio" name="scope" value="church" checked;
                            span { strong { "This church" } " Only approved members of the household." }
                        }
                        label class="choice" {
                            input type="radio" name="scope" value="neighboring";
                            span { strong { "Neighboring churches" } " Same city or region. The valley can help." }
                        }
                        label class="choice" {
                            input type="radio" name="scope" value="body";
                            span { strong { "The whole body" } " Any approved member in Ecclesia." }
                        }
                    }
                    button class="btn" type="submit" { "Post the need" }
                }
            }
        },
    )
}

pub fn need_show(
    viewer: &Viewer,
    need: &NeedCard,
    church: &Church,
    applications: &[crate::domain::ApplicationCard],
    can_help: Result<(), crate::domain::DomainError>,
    already: bool,
    unread: i64,
    flash: Option<(bool, String)>,
) -> Markup {
    let mine = viewer.user.id == need.author_id || viewer.can_govern(&need.church_id);
    page(
        &need.title,
        Some(&viewer.user),
        unread,
        Nav::Home,
        flash,
        html! {
            p class="eyebrow" {
                (scope_label(&need.scope)) " · "
                a href={ "/churches/" (church.id) } { (church.name) }
            }
            h1 { (need.title) }
            p class="lede" { (need.body) }
            p class="meta" {
                "Posted by " a href={ "/members/" (need.author_id) } { (need.author_name) }
                @if let Some(gift) = &need.gift_name { " · seeking " (gift) }
                " · " (need.status)
            }
            @if let Some(gift_id) = &need.gift_id {
                @if viewer.has_gift(gift_id) {
                    p class="pill" { "You named this gift. They may be waiting for you." }
                }
            }
            @if need.is_open() && !mine && !already {
                @if can_help.is_ok() {
                    section class="panel" {
                        h2 { "Offer to help" }
                        form class="stack" method="post" action={ "/needs/" (need.id) "/apply" } {
                            label { "How you can carry this"
                                textarea name="message" rows="3" required placeholder="When you can come, and what you will actually do." {}
                            }
                            button class="btn" type="submit" { "Apply to help" }
                        }
                    }
                } @else {
                    p class="muted" { "You can see this, but you cannot apply from where you stand." }
                }
            }
            @if already {
                p class="pill" { "You already offered. They have your name." }
            }
            @if mine && need.is_open() {
                form method="post" action={ "/needs/" (need.id) "/close" } {
                    button class="btn btn-quiet" type="submit" { "Close this need" }
                }
            }
            section {
                h2 { "Who offered" }
                @if applications.is_empty() {
                    div class="empty" { p { "No one has applied yet." } }
                }
                div class="stack" {
                    @for application in applications {
                        article class="card" {
                            a href={ "/members/" (application.user_id) } { strong { (application.user_name) } }
                            p { (application.message) }
                            p class="meta" { (application.status) }
                            @if mine && application.status == "pending" {
                                div class="row" {
                                    form method="post" action={ "/applications/" (application.id) "/accept" } {
                                        button class="btn" type="submit" { "Receive them" }
                                    }
                                    form method="post" action={ "/applications/" (application.id) "/decline" } {
                                        button class="btn btn-quiet" type="submit" { "Not this time" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
    )
}

pub fn member_show(
    viewer: &Viewer,
    person: &User,
    churches: &[(Church, Membership)],
    gifts: &[MemberGift],
    endorsements: &[EndorsementCard],
    catalog: &[Gift],
    unread: i64,
    flash: Option<(bool, String)>,
) -> Markup {
    let self_view = viewer.user.id == person.id;
    page(
        &person.name,
        Some(&viewer.user),
        unread,
        if self_view { Nav::You } else { Nav::Churches },
        flash,
        html! {
            p class="eyebrow" { (person.city) ", " (person.region) }
            h1 { (person.name) }
            @if !person.bio.is_empty() {
                p class="lede" { (person.bio) }
            }
            section {
                h2 { "Households" }
                @if churches.is_empty() {
                    p class="muted" { "Not yet approved in a church." }
                }
                ul class="people" {
                    @for (church, membership) in churches {
                        li {
                            a href={ "/churches/" (church.id) } { (church.name) }
                            span class="muted" { " · " (membership.role) " · " (membership.status) }
                        }
                    }
                }
            }
            section {
                h2 { "Gifts" }
                @if gifts.is_empty() {
                    p class="muted" { "No gifts named yet." }
                }
                div class="stack" {
                    @for gift in gifts {
                        article class="card" {
                            p class="eyebrow" { (gift.category) }
                            h3 { (gift.gift_name) }
                            @if !gift.note.is_empty() { p { (gift.note) } }
                            @let names: Vec<_> = endorsements.iter().filter(|e| e.gift_id == gift.gift_id).collect();
                            @if !names.is_empty() {
                                p class="meta" {
                                    "Endorsed by "
                                    @for (i, endorsement) in names.iter().enumerate() {
                                        @if i > 0 { ", " }
                                        a href={ "/members/" (endorsement.from_user_id) } { (endorsement.from_user_name) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            @if !self_view {
                section class="panel" {
                    h2 { "Endorse a gift" }
                    p class="muted" { "They will be notified and can accept it onto their profile — or decline. You do not get to write their name for them." }
                    form class="stack" method="post" action={ "/members/" (person.id) "/endorse" } {
                        label { "Gift"
                            select name="gift_id" required {
                                @for gift in catalog {
                                    option value=(gift.id) { (gift.name) }
                                }
                            }
                        }
                        label { "What you have actually seen"
                            textarea name="note" rows="3" required placeholder="Be specific. A gift is a life, not a compliment." {}
                        }
                        button class="btn" type="submit" { "Send endorsement" }
                    }
                }
            }
        },
    )
}

pub fn inbox(
    viewer: &Viewer,
    pending: &[EndorsementCard],
    notes: &[Notification],
    unread: i64,
    flash: Option<(bool, String)>,
) -> Markup {
    page(
        "Inbox",
        Some(&viewer.user),
        unread,
        Nav::Inbox,
        flash,
        html! {
            h1 { "What needs your yes" }
            @if pending.is_empty() && notes.is_empty() {
                div class="empty" {
                    p { "Nothing waiting. When someone endorses you, asks to join your church, or offers to help, it will land here." }
                }
            }
            @if !pending.is_empty() {
                section {
                    h2 { "Endorsements to receive" }
                    div class="stack" {
                        @for endorsement in pending {
                            article class="card" {
                                p {
                                    a href={ "/members/" (endorsement.from_user_id) } { (endorsement.from_user_name) }
                                    " named you for " strong { (endorsement.gift_name) } "."
                                }
                                p { (endorsement.note) }
                                div class="row" {
                                    form method="post" action={ "/endorsements/" (endorsement.id) "/accept" } {
                                        button class="btn" type="submit" { "Accept onto my profile" }
                                    }
                                    form method="post" action={ "/endorsements/" (endorsement.id) "/decline" } {
                                        button class="btn btn-quiet" type="submit" { "Decline" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            @if !notes.is_empty() {
                section {
                    h2 { "Notices" }
                    div class="stack" {
                        @for note in notes {
                            a class="card card-link" href=(note.href) {
                                h3 { (note.title) }
                                p { (note.body) }
                                p class="meta" { (note.kind) }
                            }
                        }
                    }
                }
            }
        },
    )
}

pub fn me(
    viewer: &Viewer,
    gifts: &[MemberGift],
    catalog: &[Gift],
    memberships: &[(Church, Membership)],
    unread: i64,
    flash: Option<(bool, String)>,
) -> Markup {
    let used: Vec<&str> = gifts.iter().map(|g| g.gift_id.as_str()).collect();
    page(
        "You",
        Some(&viewer.user),
        unread,
        Nav::You,
        flash,
        html! {
            h1 { (viewer.user.name) }
            form class="stack" method="post" action="/me" {
                label { "Name" input name="name" required value=(viewer.user.name); }
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
                @if gifts.is_empty() {
                    p class="muted" { "Name what you can actually do. This is how a need finds a person." }
                }
                div class="stack" {
                    @for gift in gifts {
                        article class="card row-between" {
                            div {
                                strong { (gift.gift_name) }
                                @if !gift.note.is_empty() { p class="muted" { (gift.note) } }
                            }
                            form method="post" action={ "/me/gifts/" (gift.gift_id) "/remove" } {
                                button class="btn btn-quiet" type="submit" { "Remove" }
                            }
                        }
                    }
                }
                form class="stack" method="post" action="/me/gifts" {
                    label { "Add a gift"
                        select name="gift_id" required {
                            @for gift in catalog {
                                @if !used.contains(&gift.id.as_str()) {
                                    option value=(gift.id) { (gift.name) " · " (gift.category) }
                                }
                            }
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
                @if memberships.is_empty() {
                    p class="muted" { "None yet." }
                    a href="/churches" { "Join or plant one" }
                }
                ul class="people" {
                    @for (church, membership) in memberships {
                        li {
                            a href={ "/churches/" (church.id) } { (church.name) }
                            span class="muted" { " · " (membership.role) " · " (membership.status) }
                        }
                    }
                }
            }
            form method="post" action="/session/logout" {
                button class="btn btn-quiet" type="submit" { "Leave this seat" }
            }
        },
    )
}

pub fn the_body(
    viewer: &Viewer,
    groups: &[(String, Vec<(Church, i64, i64)>)],
    unread: i64,
) -> Markup {
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
            @if groups.is_empty() {
                div class="empty" { p { "No churches have been planted yet." } }
            }
            @for (place, churches) in groups {
                section class="panel" {
                    h2 { (place) }
                    @if churches.len() > 1 {
                        p class="muted" { "These households can already carry neighboring needs for each other." }
                    } @else {
                        p class="muted" { "One household here so far. A neighbor in this city or region would end the island." }
                    }
                    div class="stack" {
                        @for (church, members, needs) in churches {
                            a class="card card-link" href={ "/churches/" (church.id) } {
                                h3 { (church.name) }
                                p { (church.description) }
                                p class="meta" { (members) " members · " (needs) " open needs" }
                            }
                        }
                    }
                }
            }
        },
    )
}

pub fn error_page(message: &str) -> Markup {
    page(
        "Something gave way",
        None,
        0,
        Nav::None,
        None,
        html! {
            h1 { "Something gave way" }
            p { (message) }
            a class="btn" href="/" { "Return" }
        },
    )
}

fn need_card(need: &NeedCard, viewer: &Viewer) -> Markup {
    html! {
        a class="card card-link" href={ "/needs/" (need.id) } {
            p class="eyebrow" {
                (scope_label(&need.scope)) " · " (need.church_name) " · " (need.church_city)
            }
            h3 { (need.title) }
            p { (need.body) }
            p class="meta" {
                @if let Some(gift) = &need.gift_name { (gift) " · " }
                (need.author_name)
                @if let Some(gift_id) = &need.gift_id {
                    @if viewer.has_gift(gift_id) { " · you have this gift" }
                }
            }
        }
    }
}

fn scope_label(scope: &str) -> &'static str {
    NeedScope::parse(scope)
        .map(NeedScope::label)
        .unwrap_or("Need")
}

fn first_name(name: &str) -> &str {
    name.split_whitespace().next().unwrap_or(name)
}
