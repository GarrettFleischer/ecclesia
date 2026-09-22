use maud::{Markup, html};

use ecclesia_sdk::prelude::{
    ApplicationCard, Church, DomainError, Gift, NeedCard, NeedStatus, OfferState, Viewer,
    VoiceKind, is_need_steward,
};

use super::cards::{StewardView, application_cards, church_options, gift_options, scope_label};
use super::draft::{NeedDraft, OfferDraft, review_banner, voice_pass_input};
use super::flash::Flash;
use super::layout::{Nav, csrf_input, page, page_lead, rewrite_row, share_button};

pub fn need_new(
    viewer: &Viewer,
    gifts: &[Gift],
    unread: i64,
    flash: Option<Flash>,
    csrf: &str,
    draft: &NeedDraft<'_>,
) -> Markup {
    page(
        "Post a need",
        Some(&viewer.user),
        unread,
        Nav::Home,
        flash,
        csrf,
        html! {
            (page_lead("Post a need"))
            (need_form_or_empty(viewer, gifts, csrf, draft))
        },
    )
}

fn need_form_or_empty(
    viewer: &Viewer,
    gifts: &[Gift],
    csrf: &str,
    draft: &NeedDraft<'_>,
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
            (voice_pass_input(draft.kind))
            (review_banner(draft.kind))
            label { "Church"
                select name="church_id" required {
                    (church_options(churches, Some(draft.church_id).filter(|id| !id.is_empty())))
                }
            }
            label { "Title"
                input name="title" required maxlength="120" placeholder="Dinners for the Okonkwos this week" value=(draft.title);
                (rewrite_row(VoiceKind::Need))
            }
            label { "Details"
                textarea name="body" rows="5" required maxlength="2000" placeholder="Five nights this week. Side door after 5. Fridge on the porch." { (draft.body) }
                (rewrite_row(VoiceKind::Need))
            }
            label { "Gift needed"
                select name="gift_id" {
                    option value="" { "Anyone" }
                    (gift_options(gifts, draft.gift_id))
                }
            }
            fieldset class="scopes" {
                legend { "Who can see this" }
                label class="choice" {
                    input type="radio" name="scope" value="church" checked[draft.scope != "neighboring" && draft.scope != "body"];
                    span { strong { "This church" } " Members only." }
                }
                label class="choice" {
                    input type="radio" name="scope" value="neighboring" checked[draft.scope == "neighboring"];
                    span { strong { "Nearby churches" } " Same city or region." }
                }
                label class="choice" {
                    input type="radio" name="scope" value="body" checked[draft.scope == "body"];
                    span { strong { "Everyone" } }
                }
            }
            button class="btn" type="submit" { (draft.kind.submit_label("Post need")) }
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
    draft: &OfferDraft<'_>,
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
            (page_lead(&need.title))
            p class="lede" { (need.body) }
            p class="meta" {
                "Posted by " a href={ "/members/" (need.author_id) } { (need.author_name) }
                @if let Some(gift) = &need.gift_name { " · " (gift) }
                (closed_mark(need))
            }
            (matching_gift_pill(viewer, need))
            div class="page-actions" {
                (share_button("Share", &need.title, &need.body))
                (close_form(need, steward, csrf))
            }
            (offer_panel(need, can_help, offer, steward, csrf, draft))
            (already_offered(offer))
            section {
                h2 { "Offers" }
                (who_offered(applications, steward, csrf))
            }
        },
    )
}

fn closed_mark(need: &NeedCard) -> Markup {
    if need.status() == Some(NeedStatus::Closed) {
        html! { " · Closed" }
    } else {
        html! {}
    }
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
    draft: &OfferDraft<'_>,
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
                    (voice_pass_input(draft.kind))
                    (review_banner(draft.kind))
                    label { "Message"
                        textarea name="message" rows="3" required maxlength="600" placeholder="When you're free and what you can do." { (draft.message) }
                        (rewrite_row(VoiceKind::Offer))
                    }
                    button class="btn" type="submit" { (draft.kind.submit_label("Apply to help")) }
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
            button class="btn-text" type="submit" { "Close need" }
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
