use maud::{Markup, html};

use ecclesia_sdk::prelude::{
    Church, DomainError, Gift, NeedCard, NeedReplyCard, NeedStatus, Viewer, VoiceKind,
    is_need_steward,
};

use super::cards::{StewardView, church_options, gift_options, scope_label, scope_mark};
use super::draft::{NeedDraft, OfferDraft, review_banner, voice_pass_input};
use super::flash::Flash;
use super::layout::{
    Monogram, Nav, csrf_input, detail_lead, monogram, page, page_lead, rewrite_row, share_button,
};

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
    let Some(church) = viewer.active_church() else {
        return html! {
            div class="empty" {
                p { "Join a church first, then post from there." }
                a class="btn" href="/churches/join" { "Find your church" }
            }
        };
    };
    html! {
        form class="stack" method="post" action="/needs" {
            (csrf_input(csrf))
            (voice_pass_input(draft.kind))
            (review_banner(draft.kind))
            label { "Church"
                select name="church_id" required {
                    (church_options(std::iter::once(church), Some(draft.church_id).filter(|id| !id.is_empty())))
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
                    span { strong { "Nearby churches" } " Within 40 km." }
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
    replies: &[NeedReplyCard],
    can_reply: Result<(), DomainError>,
    unread: i64,
    flash: Option<Flash>,
    csrf: &str,
    draft: &OfferDraft<'_>,
    place: Option<(&str, &str)>,
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
            div class="card-top" {
                (scope_mark(&need.scope))
                p class="eyebrow" {
                    (scope_label(&need.scope)) " · "
                    a href={ "/churches/" (church.id) } { (church.name) }
                }
            }
            (detail_lead(&need.title))
            p class="lede" { (need.body) }
            div class="card-foot need-meta" {
                span class="byline" {
                    (monogram(&need.author_id, &need.author_name, Monogram::PersonSmall))
                    span { "Posted by " a href={ "/members/" (need.author_id) } { (need.author_name) } }
                }
                @if let Some(gift) = &need.gift_name { span class="chip" { (gift) } }
                (closed_chip(need))
            }
            (matching_gift_pill(viewer, need))
            div class="page-actions" {
                (share_button("Share", &need.title, &need.body, ""))
                (close_form(need, steward, csrf))
            }
            (reply_panel(need, can_reply, csrf, draft, place))
            section {
                h2 { "Replies" }
                (reply_list(replies))
            }
        },
    )
}

fn closed_chip(need: &NeedCard) -> Markup {
    if need.status() == Some(NeedStatus::Closed) {
        html! { span class="chip chip-closed" { "Closed" } }
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

fn reply_panel(
    need: &NeedCard,
    can_reply: Result<(), DomainError>,
    csrf: &str,
    draft: &OfferDraft<'_>,
    place: Option<(&str, &str)>,
) -> Markup {
    if !need.is_open() {
        return html! {};
    }
    match can_reply {
        Ok(()) => html! {
            section class="panel" {
                h2 { "Reply" }
                form class="stack" method="post" action={ "/needs/" (need.id) "/replies" } {
                    (csrf_input(csrf))
                    (place_fields(place))
                    (voice_pass_input(draft.kind))
                    (review_banner(draft.kind))
                    label { "Reply"
                        textarea name="body" rows="3" required maxlength="600" placeholder="I can bring dinner Thursday." { (draft.message) }
                        (rewrite_row(VoiceKind::Reply))
                    }
                    button class="btn" type="submit" { (draft.kind.submit_label("Reply")) }
                }
            }
        },
        Err(reason) => html! {
            p class="muted" { (reason) }
        },
    }
}

fn place_fields(place: Option<(&str, &str)>) -> Markup {
    let Some((lat, lng)) = place else {
        return html! {};
    };
    html! {
        input type="hidden" name="lat" value=(lat);
        input type="hidden" name="lng" value=(lng);
    }
}

fn reply_list(replies: &[NeedReplyCard]) -> Markup {
    if replies.is_empty() {
        return html! {
            div class="empty" { p { "No replies yet." } }
        };
    }
    html! {
        div class="stack" {
            @for reply in replies {
                article class="card" {
                    p class="meta" {
                        a href={ "/members/" (reply.author_id) } { (reply.author_name) }
                    }
                    p { (reply.body) }
                }
            }
        }
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

