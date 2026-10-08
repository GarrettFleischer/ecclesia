use maud::{Markup, html};

use ecclesia_sdk::db::AttachmentRow;
use ecclesia_sdk::prelude::{
    Church, DomainError, Gift, NeedCard, NeedShelf, NeedStatus, Viewer, VoiceKind,
};

use super::cards::{church_options, gift_options, scope_label, scope_mark};
use super::conversation::{
    AvatarSize, GalleryRemoval, KeptPhoto, LoadedReply, PhotoReach, avatar_face, conversation_fragment,
    person_avatar, photo_fields, photo_gallery, photo_viewer, photos_from,
};
use super::draft::{NeedDraft, OfferDraft, ReplyIntent, review_banner, voice_pass_input};
use super::flash::Flash;
use super::layout::{Nav, csrf_input, detail_lead, page, page_lead, rewrite_row, share_button};

pub fn need_new(
    viewer: &Viewer,
    gifts: &[Gift],
    unread: i64,
    flash: Option<Flash>,
    csrf: &str,
    draft: &NeedDraft<'_>,
    kept: &[KeptPhoto<'_>],
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
            (need_form_or_empty(viewer, gifts, csrf, draft, kept))
        },
    )
}

fn need_form_or_empty(
    viewer: &Viewer,
    gifts: &[Gift],
    csrf: &str,
    draft: &NeedDraft<'_>,
    kept: &[KeptPhoto<'_>],
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
        form class="stack" method="post" action="/needs" enctype="multipart/form-data" {
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
            (photo_fields(kept))
            button class="btn" type="submit" { (draft.kind.submit_label("Post need")) }
        }
    }
}

pub fn need_show(
    viewer: &Viewer,
    need: &NeedCard,
    church: &Church,
    need_photos: &[AttachmentRow],
    author_avatar: Option<&str>,
    replies: &[LoadedReply],
    can_reply: Result<(), DomainError>,
    unread: i64,
    flash: Option<Flash>,
    csrf: &str,
    draft: &OfferDraft<'_>,
    kept: &[KeptPhoto<'_>],
    place: Option<(&str, &str)>,
    share_url: &str,
    share_mint: &str,
    mark: NeedMark,
) -> Markup {
    let photos = photos_from(need_photos);
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
            (photo_gallery(&photos, need_removal(viewer, need), PhotoReach::Opens))
            div class="card-foot need-meta" {
                div class="byline" {
                    (person_avatar(&need.author_id, &need.author_name, avatar_face(author_avatar), AvatarSize::Small))
                    p class="meta" {
                        "Posted by "
                        a href={ "/members/" (need.author_id) } { (need.author_name) }
                        span class="reply-church" { (church.name) }
                    }
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
            (reply_panel(viewer, need, can_reply, csrf, draft, kept, place))
            (need_return_script())
            (conversation_fragment(&need.id, &viewer.user.id, replies, place))
            (photo_viewer())
        },
    )
}

fn need_removal<'a>(viewer: &Viewer, need: &'a NeedCard) -> GalleryRemoval<'a> {
    if viewer.user.id == need.author_id {
        GalleryRemoval::FromNeed { need_id: &need.id }
    } else {
        GalleryRemoval::Closed
    }
}

fn closed_chip(need: &NeedCard) -> Markup {
    if need.status() == Some(NeedStatus::Closed) {
        html! { span class="chip chip-closed" { "Met" } }
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
    viewer: &Viewer,
    need: &NeedCard,
    can_reply: Result<(), DomainError>,
    csrf: &str,
    draft: &OfferDraft<'_>,
    kept: &[KeptPhoto<'_>],
    place: Option<(&str, &str)>,
) -> Markup {
    if !need.is_open() || need.shelf == NeedShelf::Archived {
        return html! {};
    }
    let (reply_kept, completion_kept) = kept_for(draft.intent, kept);
    html! {
        (reply_slot(viewer, need, can_reply, csrf, draft, reply_kept, place))
        (completion_slot(viewer, need, csrf, draft, completion_kept))
    }
}

fn kept_for<'a>(
    intent: ReplyIntent,
    kept: &'a [KeptPhoto<'a>],
) -> (&'a [KeptPhoto<'a>], &'a [KeptPhoto<'a>]) {
    match intent {
        ReplyIntent::Met => (&[], kept),
        ReplyIntent::Reply => (kept, &[]),
    }
}

fn reply_slot(
    viewer: &Viewer,
    need: &NeedCard,
    can_reply: Result<(), DomainError>,
    csrf: &str,
    draft: &OfferDraft<'_>,
    kept: &[KeptPhoto<'_>],
    place: Option<(&str, &str)>,
) -> Markup {
    match can_reply {
        Ok(()) => reply_form(need, csrf, draft, kept, place),
        Err(_) if viewer.user.id == need.author_id => html! {},
        Err(reason) => html! {
            p class="muted" { (reason) }
        },
    }
}

fn completion_slot(
    viewer: &Viewer,
    need: &NeedCard,
    csrf: &str,
    draft: &OfferDraft<'_>,
    kept: &[KeptPhoto<'_>],
) -> Markup {
    if viewer.user.id != need.author_id {
        return html! {};
    }
    completion_form(need, csrf, draft, kept)
}

fn reply_form(
    need: &NeedCard,
    csrf: &str,
    draft: &OfferDraft<'_>,
    kept: &[KeptPhoto<'_>],
    place: Option<(&str, &str)>,
) -> Markup {
    let action = format!("/needs/{}/replies", need.id);
    html! {
        section class="panel" {
            form class="stack" method="post" action=(action) enctype="multipart/form-data" data-reply-form {
                (csrf_input(csrf))
                (place_fields(place))
                (voice_pass_input(draft.kind))
                (review_banner(draft.kind))
                label { "Add a reply"
                    textarea name="body" rows="3" required maxlength="600" placeholder="I can measure the steps Thursday afternoon." { (reply_message(draft)) }
                    (rewrite_row(VoiceKind::Reply))
                }
                (photo_fields(kept))
                div class="reply-actions" {
                    button class="btn" type="submit" { (draft.kind.submit_label("Post reply")) }
                }
            }
        }
    }
}

fn completion_form(
    need: &NeedCard,
    csrf: &str,
    draft: &OfferDraft<'_>,
    kept: &[KeptPhoto<'_>],
) -> Markup {
    let action = format!("/needs/{}/complete", need.id);
    html! {
        details class="mark-met" open[draft.intent == ReplyIntent::Met] {
            summary class="btn btn-quiet" { "Mark this need met" }
            form class="stack" method="post" action=(action) enctype="multipart/form-data" data-reply-form {
                (csrf_input(csrf))
                (voice_pass_input(draft.kind))
                (review_banner(draft.kind))
                label { "How was the need met?"
                    textarea name="body" rows="3" required maxlength="600" placeholder="The new handrail is installed and ready to use." { (completion_message(draft)) }
                    (rewrite_row(VoiceKind::Reply))
                }
                (photo_fields(kept))
                button class="btn" type="submit" { (draft.kind.submit_label("Post completion")) }
            }
        }
    }
}

fn reply_message<'a>(draft: &'a OfferDraft<'a>) -> &'a str {
    match draft.intent {
        ReplyIntent::Reply => draft.message,
        ReplyIntent::Met => "",
    }
}

fn completion_message<'a>(draft: &'a OfferDraft<'a>) -> &'a str {
    match draft.intent {
        ReplyIntent::Met => draft.message,
        ReplyIntent::Reply => "",
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

pub enum NeedMark {
    None,
    JustMet,
    Reopened,
}
