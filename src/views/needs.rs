use maud::{html, Markup};

use crate::leaf::{
    is_need_steward, ApplicationCard, Church, DomainError, Gift, NeedCard, OfferState, Viewer,
    VoiceKind,
};

use super::cards::{application_cards, church_options, gift_options, scope_label, StewardView};
use super::flash::Flash;
use super::layout::{csrf_input, page, rewrite_row, share_button, Nav};

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
        csrf,
        html! {
            h1 { "Post a need" }
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
                p { "Join a church first, then post from there." }
                a class="btn" href="/churches" { "Find your church" }
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
            label { "Title" input name="title" required placeholder="Dinners for the Okonkwos this week"; }
            label { "Details"
                textarea name="body" rows="5" required placeholder="What, when, and where. Anything that helps someone decide if they can do it." {}
                (rewrite_row(VoiceKind::Need))
            }
            label { "Gift needed"
                select name="gift_id" {
                    option value="" { "Anyone" }
                    (gift_options(gifts))
                }
            }
            fieldset class="scopes" {
                legend { "Who can see this" }
                label class="choice" {
                    input type="radio" name="scope" value="church" checked;
                    span { strong { "This church" } " Members only." }
                }
                label class="choice" {
                    input type="radio" name="scope" value="neighboring";
                    span { strong { "Churches nearby" } " Same city or region." }
                }
                label class="choice" {
                    input type="radio" name="scope" value="body";
                    span { strong { "Everyone on Ecclesia" } }
                }
            }
            button class="btn" type="submit" { "Post need" }
        }
    }
}

pub fn need_show(
    viewer: &Viewer,
    need: &NeedCard,
    church: &Church,
    applications: &[&ApplicationCard],
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
        csrf,
        html! {
            p class="eyebrow" {
                (scope_label(&need.scope)) " · "
                a href={ "/churches/" (church.id) } { (church.name) }
            }
            h1 { (need.title) }
            (share_button("Share", &need.title, &need.body))
            p class="lede" { (need.body) }
            p class="meta" {
                "Posted by " a href={ "/members/" (need.author_id) } { (need.author_name) }
                @if let Some(gift) = &need.gift_name { " · " (gift) }
                " · " (need.status)
            }
            (matching_gift_pill(viewer, need))
            (offer_panel(need, can_help, offer, steward, csrf))
            (already_offered(offer))
            (close_form(need, steward, csrf))
            section {
                h2 { "Offers" }
                (who_offered(applications, steward, csrf))
            }
        },
    )
}

fn steward_of(viewer: &Viewer, need: &NeedCard) -> StewardView {
    if is_need_steward(viewer, need.sight()) {
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
    html! { p class="pill" { "This matches a gift on your profile." } }
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
                    label { "Message"
                        textarea name="message" rows="3" required maxlength="600" placeholder="When you're free and what you can do." {}
                        (rewrite_row(VoiceKind::Offer))
                    }
                    button class="btn" type="submit" { "Apply to help" }
                }
            }
        },
        Err(reason) => html! {
            p class="muted" { (reason) }
        },
    }
}

fn already_offered(offer: OfferState) -> Markup {
    match offer {
        OfferState::AlreadyOffered => {
            html! { p class="pill" { "You applied. They'll see your offer." } }
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
            button class="btn btn-quiet" type="submit" { "Close need" }
        }
    }
}

fn who_offered(applications: &[&ApplicationCard], steward: StewardView, csrf: &str) -> Markup {
    if applications.is_empty() {
        return html! {
            div class="empty" { p { "No offers yet." } }
        };
    }
    html! {
        div class="stack" {
            (application_cards(applications.iter().copied(), steward, csrf))
        }
    }
}
