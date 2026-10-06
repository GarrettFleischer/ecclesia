use maud::{Markup, html};

use ecclesia_sdk::prelude::{
    Church, DomainError, Gift, NeedCard, NeedReplyCard, NeedShelf, NeedStatus, Viewer, VoiceKind,
};

use super::cards::{church_options, gift_options, scope_label, scope_mark};
use super::draft::{NeedDraft, OfferDraft, ReplyIntent, review_banner, voice_pass_input};
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
    let churches: Vec<_> = viewer.active_churches().collect();
    if churches.is_empty() {
        return html! {
            div class="empty" {
                p { "Join a church first, then post from there." }
                a class="btn" href="/churches/join" { "Find your church" }
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
    share_url: &str,
    share_mint: &str,
    mark: NeedMark,
) -> Markup {
    let actions = reply_actions(viewer, need, can_reply);
    page(
        &need.title,
        Some(&viewer.user),
        unread,
        Nav::Home,
        flash,
        csrf,
        html! {
            (back_to_needs(church, need, mark))
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
                (reopen_form(viewer, need, csrf))
            }
            (praise_report(need))
            (matching_gift_pill(viewer, need))
            div class="page-actions" {
                (share_button("Share", &need.title, &need.body, share_url, share_mint))
            }
            (reply_panel(need, actions, csrf, draft, place))
            (need_return_script())
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
    actions: Result<ReplyActions, DomainError>,
    csrf: &str,
    draft: &OfferDraft<'_>,
    place: Option<(&str, &str)>,
) -> Markup {
    if !need.is_open() || need.shelf == NeedShelf::Archived {
        return html! {};
    }
    match actions {
        Ok(actions) => reply_form(need, actions, csrf, draft, place),
        Err(reason) => html! {
            p class="muted" { (reason) }
        },
    }
}

fn reply_form(
    need: &NeedCard,
    actions: ReplyActions,
    csrf: &str,
    draft: &OfferDraft<'_>,
    place: Option<(&str, &str)>,
) -> Markup {
    let action = format!("/needs/{}/replies", need.id);
    let (heading, placeholder) = match actions {
        ReplyActions::Met => ("Praise report", "The dinners are covered."),
        ReplyActions::Reply | ReplyActions::ReplyAndMet => {
            ("Reply", "I can bring dinner Thursday.")
        }
    };
    html! {
        section class="panel" {
            h2 { (heading) }
            form class="stack" method="post" action=(action) data-reply-form {
                (csrf_input(csrf))
                (place_fields(place))
                (voice_pass_input(draft.kind))
                (review_banner(draft.kind))
                label { (heading)
                    textarea name="body" rows="3" required maxlength="600" placeholder=(placeholder) { (draft.message) }
                    (rewrite_row(box_voice(actions)))
                }
                div class="reply-actions" {
                    @if matches!(actions, ReplyActions::Met | ReplyActions::ReplyAndMet) {
                        label class="met-check" {
                            input type="checkbox" name="met" value="1" data-mark-met checked[draft.intent == ReplyIntent::Met] required[matches!(actions, ReplyActions::Met)];
                            span { "This need has been met" }
                        }
                    }
                    button class="btn" type="submit" { (draft.kind.submit_label("Reply")) }
                }
            }
        }
    }
}

fn box_voice(actions: ReplyActions) -> VoiceKind {
    match actions {
        ReplyActions::Met => VoiceKind::Praise,
        ReplyActions::Reply | ReplyActions::ReplyAndMet => VoiceKind::Reply,
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

fn praise_report(need: &NeedCard) -> Markup {
    let Some(praise) = need.praise.as_deref().filter(|text| !text.is_empty()) else {
        return html! {};
    };
    html! {
        section class="panel" {
            h2 { "Praise report" }
            p { (praise) }
        }
    }
}

fn reopen_form(viewer: &Viewer, need: &NeedCard, csrf: &str) -> Markup {
    if viewer.user.id != need.author_id
        || need.status() != Some(NeedStatus::Closed)
        || need.shelf != NeedShelf::Listed
    {
        return html! {};
    }
    html! {
        form class="reopen-form" method="post" action={ "/needs/" (need.id) "/reopen" } {
            (csrf_input(csrf))
            button class="btn btn-quiet" type="submit" { "Reopen need" }
        }
    }
}

fn back_to_needs(church: &Church, need: &NeedCard, mark: NeedMark) -> Markup {
    html! {
        a class="back-link" href={ "/churches/" (church.id) "?return=1" } data-need-back data-church=(church.id) data-need=(need.id) data-just-met[matches!(mark, NeedMark::JustMet)] data-reopened[matches!(mark, NeedMark::Reopened)] { "All needs" }
    }
}

fn need_return_script() -> Markup {
    html! { script { (maud::PreEscaped(NEED_RETURN_SCRIPT)) } }
}

const NEED_RETURN_SCRIPT: &str = r#"(function () {
  var link = document.querySelector("[data-need-back]");
  if (!link) return;
  var marking = link.hasAttribute("data-just-met");
  var reopened = link.hasAttribute("data-reopened");
  if (!marking && !reopened) return;
  var churchId = link.getAttribute("data-church") || "";
  var needId = link.getAttribute("data-need") || "";
  var key = "ecclesia.needReturn";
  var saved = null;
  try { saved = JSON.parse(sessionStorage.getItem(key) || "null"); } catch (error) { saved = null; }
  if (marking) {
    if (!saved || saved.churchId !== churchId) {
      saved = { churchId: churchId, scrollY: null, needId: needId, flash: true };
    } else {
      saved.needId = needId;
      saved.flash = true;
    }
    try { sessionStorage.setItem(key, JSON.stringify(saved)); } catch (error) {}
    return;
  }
  if (saved && saved.churchId === churchId) {
    saved.flash = false;
    try { sessionStorage.setItem(key, JSON.stringify(saved)); } catch (error) {}
  }
})();"#;

#[derive(Clone, Copy)]
enum ReplyActions {
    Reply,
    ReplyAndMet,
    Met,
}

fn reply_actions(
    viewer: &Viewer,
    need: &NeedCard,
    can_reply: Result<(), DomainError>,
) -> Result<ReplyActions, DomainError> {
    if viewer.user.id == need.author_id {
        return Ok(author_actions(can_reply));
    }
    can_reply.map(|()| ReplyActions::Reply)
}

fn author_actions(can_reply: Result<(), DomainError>) -> ReplyActions {
    match can_reply {
        Ok(()) => ReplyActions::ReplyAndMet,
        Err(_) => ReplyActions::Met,
    }
}

pub enum NeedMark {
    None,
    JustMet,
    Reopened,
}
