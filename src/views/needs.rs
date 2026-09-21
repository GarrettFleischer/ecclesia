use maud::{html, Markup};

use crate::leaf::{ApplicationCard, Church, DomainError, Gift, NeedCard, OfferState, Viewer};

use super::cards::{application_cards, church_options, gift_options, scope_label, StewardView};
use super::flash::Flash;
use super::layout::{csrf_input, page, Nav};

pub fn need_new(
    viewer: &Viewer,
    gifts: &[Gift],
    selected_church: Option<&str>,
    unread: i64,
    flash: Option<Flash>,
    csrf: &str,
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
            (need_form_or_empty(viewer, gifts, selected_church, csrf))
        },
    )
}

fn need_form_or_empty(
    viewer: &Viewer,
    gifts: &[Gift],
    selected_church: Option<&str>,
    csrf: &str,
) -> Markup {
    let mut churches = viewer.active_churches().peekable();
    if churches.peek().is_none() {
        return html! {
            div class="empty" {
                p { "You can only post from a church where you are an approved member." }
                a class="btn" href="/churches" { "Find a church" }
            }
        };
    }
    html! {
        form class="stack" method="post" action="/needs" {
            (csrf_input(csrf))
            label { "Church"
                select name="church_id" required {
                    (church_options(churches, selected_church))
                }
            }
            label { "Title" input name="title" required placeholder="Meal train for the Okonkwo family"; }
            label { "The actual need"
                textarea name="body" rows="5" required placeholder="When, where, what kind of help, and what would be too much." {}
            }
            label { "Primary gift you are hoping for"
                select name="gift_id" {
                    option value="" { "Any willing hands" }
                    (gift_options(gifts))
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
}

pub fn need_show(
    viewer: &Viewer,
    need: &NeedCard,
    church: &Church,
    applications: &[ApplicationCard],
    can_help: Result<(), DomainError>,
    offer: OfferState,
    unread: i64,
    flash: Option<Flash>,
    csrf: &str,
) -> Markup {
    let steward = steward_of(viewer, need);
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
            (matching_gift_pill(viewer, need))
            (offer_panel(need, can_help, offer, steward, csrf))
            (already_offered(offer))
            (close_form(need, steward, csrf))
            section {
                h2 { "Who offered" }
                (who_offered(applications, steward, csrf))
            }
        },
    )
}

fn steward_of(viewer: &Viewer, need: &NeedCard) -> StewardView {
    if viewer.user.id == need.author_id || viewer.can_govern(&need.church_id) {
        StewardView::Steward
    } else {
        StewardView::Guest
    }
}

fn matching_gift_pill(viewer: &Viewer, need: &NeedCard) -> Markup {
    let Some(gift_id) = &need.gift_id else {
        return html! {};
    };
    if !viewer.has_gift(gift_id) {
        return html! {};
    }
    html! { p class="pill" { "You named this gift. They may be waiting for you." } }
}

fn offer_panel(
    need: &NeedCard,
    can_help: Result<(), DomainError>,
    offer: OfferState,
    steward: StewardView,
    csrf: &str,
) -> Markup {
    if !need.is_open()
        || matches!(steward, StewardView::Steward)
        || matches!(offer, OfferState::AlreadyOffered)
    {
        return html! {};
    }
    match can_help {
        Ok(()) => html! {
            section class="panel" {
                h2 { "Offer to help" }
                form class="stack" method="post" action={ "/needs/" (need.id) "/apply" } {
                    (csrf_input(csrf))
                    label { "How you can carry this"
                        textarea name="message" rows="3" required maxlength="600" placeholder="When you can come, and what you will actually do." {}
                    }
                    button class="btn" type="submit" { "Apply to help" }
                }
            }
        },
        Err(_) => html! {
            p class="muted" { "You can see this, but you cannot apply from where you stand." }
        },
    }
}

fn already_offered(offer: OfferState) -> Markup {
    match offer {
        OfferState::AlreadyOffered => {
            html! { p class="pill" { "You already offered. They have your name." } }
        }
        OfferState::NotYet => html! {},
    }
}

fn close_form(need: &NeedCard, steward: StewardView, csrf: &str) -> Markup {
    if !need.is_open() || !matches!(steward, StewardView::Steward) {
        return html! {};
    }
    html! {
        form method="post" action={ "/needs/" (need.id) "/close" } {
            (csrf_input(csrf))
            button class="btn btn-quiet" type="submit" { "Close this need" }
        }
    }
}

fn who_offered(applications: &[ApplicationCard], steward: StewardView, csrf: &str) -> Markup {
    if applications.is_empty() {
        return html! {
            div class="empty" { p { "No one has applied yet." } }
        };
    }
    html! {
        div class="stack" {
            (application_cards(applications, steward, csrf))
        }
    }
}
