//! Shared avatar, gallery, and reply rendering.
//!
//! Need pages and the public example use these. A description is the image
//! alternative. An undescribed image uses an empty alternative when the
//! surrounding text already names the context.
//!
//! `source` is a static image for the public example. When it is absent, the
//! address stays `/media/{id}/thumb` or `/media/{id}/full` and the markup
//! matches member pages.
//!
//! A member's name links to their profile. An example name is text.

use maud::{Markup, html};

use ecclesia_sdk::db::AttachmentRow;
use ecclesia_sdk::prelude::{NeedReplyCard, ReplyKind};

use super::layout::{Monogram, monogram};

/// How large an avatar paints.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AvatarSize {
    Small,
    Regular,
    Large,
}

/// A stored profile photo. `source` replaces the media URL for the public example.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AvatarPhoto<'a> {
    pub id: &'a str,
    pub source: Option<&'a str>,
}

/// A profile photo, or initials when the person has none.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AvatarFace<'a> {
    Photo(AvatarPhoto<'a>),
    Initials,
}

/// One attached or staged photo.
///
/// `source` is a static path for the public example. Member pages leave it
/// unset.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Photo<'a> {
    pub id: &'a str,
    pub description: Option<&'a str>,
    pub source: Option<&'a str>,
}

/// A profile link, or the name as text.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NameLink {
    /// The member profile.
    Profile,
    /// The name, with no link.
    Plain,
}

/// Whether a posted photo opens the viewer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PhotoReach {
    /// A button opens the large photo.
    Opens,
    /// The picture stays in the post.
    Fixed,
}

/// Who may detach a posted photo. Staged previews use their own control.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GalleryRemoval<'a> {
    Closed,
    FromNeed { need_id: &'a str },
    FromReply { reply_id: &'a str },
}

/// A reply the conversation renderer can paint without loading the store.
#[derive(Clone, Copy)]
pub struct ReplyFace<'a> {
    pub id: &'a str,
    pub author_id: &'a str,
    pub author_name: &'a str,
    pub name_link: NameLink,
    pub kind: ReplyKind,
    pub body: &'a str,
    pub photos: &'a [Photo<'a>],
    pub avatar: AvatarFace<'a>,
    pub removal: GalleryRemoval<'a>,
    pub reach: PhotoReach,
    pub church: Option<&'a str>,
}

/// Photos already staged, kept on a review of the same form.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct KeptPhoto<'a> {
    pub id: &'a str,
    pub description: &'a str,
}

/// A reply row plus the photos and avatar the page already loaded.
pub struct LoadedReply {
    pub card: NeedReplyCard,
    pub photos: Vec<AttachmentRow>,
    pub avatar_media_id: Option<String>,
}

/// Profile photo when `media_id` is set, otherwise initials.
pub fn avatar_face(media_id: Option<&str>) -> AvatarFace<'_> {
    match media_id {
        Some(id) => AvatarFace::Photo(AvatarPhoto { id, source: None }),
        None => AvatarFace::Initials,
    }
}

/// Profile photo from a static path, for the public example.
pub fn static_avatar<'a>(id: &'a str, source: &'a str) -> AvatarFace<'a> {
    AvatarFace::Photo(AvatarPhoto {
        id,
        source: Some(source),
    })
}

/// The person's photo, or their initials.
pub fn person_avatar(
    person_id: &str,
    name: &str,
    face: AvatarFace<'_>,
    size: AvatarSize,
) -> Markup {
    match face {
        AvatarFace::Photo(photo) => avatar_image(photo, size),
        AvatarFace::Initials => monogram(person_id, name, monogram_size(size)),
    }
}

/// One photo paints large. Several paint as thumbnails.
pub fn photo_gallery(
    photos: &[Photo<'_>],
    removal: GalleryRemoval<'_>,
    reach: PhotoReach,
) -> Markup {
    if photos.is_empty() {
        return html! {};
    }
    html! {
        div class="photo-set" data-photo-set {
            (gallery_frames(photos, removal, reach))
        }
    }
}

/// Accessible viewer. The page includes it once, outside the live fragment.
pub fn photo_viewer() -> Markup {
    html! {
        dialog class="photo-viewer" aria-label="Photo viewer" data-photo-viewer {
            div class="photo-viewer-bar" {
                p data-photo-count {}
                button type="button" class="btn btn-quiet" data-photo-close { "Close photo" }
            }
            img data-photo-frame alt="";
            div class="photo-viewer-nav" {
                button type="button" class="btn btn-quiet" data-photo-prev { "Previous photo" }
                button type="button" class="btn btn-quiet" data-photo-next { "Next photo" }
            }
        }
    }
}

/// The photo field shared by need, reply, and completion forms.
pub fn photo_fields(kept: &[KeptPhoto<'_>]) -> Markup {
    html! {
        fieldset class="photo-field" data-photo-field {
            legend { "Photos" }
            label class="photo-add" {
                span { "Add photos" }
                input type="file" name="photos" accept="image/jpeg,image/png,image/webp" multiple;
            }
            p class="field-note" { "Up to five photos. JPEG, PNG, or WebP." }
            p class="field-error" data-photo-error hidden {}
            div class="photo-previews" data-photo-previews {
                @for photo in kept {
                    (staged_preview(photo))
                }
            }
        }
    }
}

/// One reply, including a completion.
pub fn reply_article(reply: &ReplyFace<'_>) -> Markup {
    html! {
        article class="card" data-reply=(reply.id) {
            div class="byline" {
                (person_avatar(reply.author_id, reply.author_name, reply.avatar, AvatarSize::Small))
                p class="meta" {
                    (author_name(reply))
                    @if let Some(church) = reply.church {
                        span class="reply-church" { (church) }
                    }
                    (completion_mark(reply.kind))
                }
            }
            p { (reply.body) }
            (photo_gallery(reply.photos, reply.removal, reply.reach))
        }
    }
}

/// The replies block. A refresh replaces this node.
pub fn conversation_fragment(
    need_id: &str,
    viewer_id: &str,
    replies: &[LoadedReply],
    place: Option<(&str, &str)>,
) -> Markup {
    let (lat, lng) = place.unwrap_or(("", ""));
    html! {
        div data-conversation data-need=(need_id) data-lat=(lat) data-lng=(lng) {
            h2 { "Replies" }
            (reply_list(viewer_id, replies))
        }
    }
}

pub fn photos_from(rows: &[AttachmentRow]) -> Vec<Photo<'_>> {
    let mut photos = Vec::with_capacity(rows.len());
    for row in rows {
        photos.push(Photo {
            id: &row.media_id,
            description: row.description.as_deref(),
            source: None,
        });
    }
    photos
}

fn staged_preview(photo: &KeptPhoto<'_>) -> Markup {
    html! {
        figure class="photo-preview" data-staged=(photo.id) {
            img src=(media_path(photo.id, "thumb")) alt=(alt_text(Some(photo.description)));
            input type="hidden" name="staged_id" value=(photo.id);
            label { "Photo description (optional)"
                input name="description" maxlength="300" placeholder="Loose railing beside the front steps" value=(photo.description);
            }
            button type="button" class="btn btn-quiet" data-discard-staged { "Remove photo" }
        }
    }
}

fn reply_list(viewer_id: &str, replies: &[LoadedReply]) -> Markup {
    if replies.is_empty() {
        return html! {
            div class="empty" { p { "No replies yet." } }
        };
    }
    html! {
        div class="stack" data-replies {
            @for reply in replies {
                (paint_loaded(viewer_id, reply))
            }
        }
    }
}

fn paint_loaded(viewer_id: &str, reply: &LoadedReply) -> Markup {
    let photos = photos_from(&reply.photos);
    reply_article(&ReplyFace {
        id: &reply.card.id,
        author_id: &reply.card.author_id,
        author_name: &reply.card.author_name,
        name_link: NameLink::Profile,
        kind: reply.card.kind,
        body: &reply.card.body,
        photos: &photos,
        avatar: avatar_face(reply.avatar_media_id.as_deref()),
        removal: removal_for(viewer_id, reply),
        reach: PhotoReach::Opens,
        church: reply.card.author_church.as_deref(),
    })
}

fn removal_for<'a>(viewer_id: &str, reply: &'a LoadedReply) -> GalleryRemoval<'a> {
    if viewer_id == reply.card.author_id {
        GalleryRemoval::FromReply {
            reply_id: &reply.card.id,
        }
    } else {
        GalleryRemoval::Closed
    }
}

fn gallery_frames(
    photos: &[Photo<'_>],
    removal: GalleryRemoval<'_>,
    reach: PhotoReach,
) -> Markup {
    match photos {
        [photo] => photo_frame(photo, 0, FrameSize::Large, removal, reach),
        many => html! {
            ul class="photo-thumbs" {
                @for (index, photo) in many.iter().enumerate() {
                    li { (photo_frame(photo, index, FrameSize::Thumb, removal, reach)) }
                }
            }
        },
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FrameSize {
    Large,
    Thumb,
}

fn photo_frame(
    photo: &Photo<'_>,
    index: usize,
    size: FrameSize,
    removal: GalleryRemoval<'_>,
    reach: PhotoReach,
) -> Markup {
    let alt = alt_text(photo.description);
    html! {
        figure class=(frame_class(size)) {
            (photo_control(photo, index, size, alt, reach))
            (remove_control(photo.id, removal))
        }
    }
}

fn photo_control(
    photo: &Photo<'_>,
    index: usize,
    size: FrameSize,
    alt: &str,
    reach: PhotoReach,
) -> Markup {
    match reach {
        PhotoReach::Fixed => photo_image(photo, size, alt),
        PhotoReach::Opens => {
            let full = photo_full(photo);
            html! {
                button type="button" class="photo-open" data-photo-open data-index=(index) data-full=(full) data-alt=(alt) {
                    (photo_image(photo, size, alt))
                }
            }
        }
    }
}

fn photo_full(photo: &Photo<'_>) -> String {
    match photo.source {
        Some(path) => path.to_string(),
        None => media_path(photo.id, "full"),
    }
}

fn photo_image(photo: &Photo<'_>, size: FrameSize, alt: &str) -> Markup {
    match photo.source {
        Some(path) => html! {
            img class=(image_class(size)) src=(path) alt=(alt) loading="lazy" decoding="async";
        },
        None => html! {
            img class=(image_class(size)) src=(media_path(photo.id, source_variant(size))) alt=(alt);
        },
    }
}

fn avatar_image(photo: AvatarPhoto<'_>, size: AvatarSize) -> Markup {
    match photo.source {
        Some(path) => html! {
            img class=(avatar_class(size)) src=(path) alt="" loading="lazy" decoding="async";
        },
        None => html! {
            img class=(avatar_class(size)) src=(media_path(photo.id, "thumb")) alt="";
        },
    }
}

fn remove_control(media_id: &str, removal: GalleryRemoval<'_>) -> Markup {
    match removal {
        GalleryRemoval::Closed => html! {},
        GalleryRemoval::FromNeed { need_id } => remove_button(media_id, "need", need_id),
        GalleryRemoval::FromReply { reply_id } => remove_button(media_id, "reply", reply_id),
    }
}

fn remove_button(media_id: &str, kind: &str, parent: &str) -> Markup {
    html! {
        button type="button" class="btn btn-quiet photo-remove" data-remove-photo data-media=(media_id) data-parent-kind=(kind) data-parent=(parent) { "Remove photo" }
    }
}

/// The author's name. A profile links to `/members/{id}`. Plain text does not.
fn author_name(reply: &ReplyFace<'_>) -> Markup {
    match reply.name_link {
        NameLink::Profile => html! {
            a href={ "/members/" (reply.author_id) } { (reply.author_name) }
        },
        NameLink::Plain => html! { (reply.author_name) },
    }
}

fn completion_mark(kind: ReplyKind) -> Markup {
    match kind {
        ReplyKind::Completion => html! { span class="chip" { "Met" } },
        ReplyKind::Message => html! {},
    }
}

fn alt_text(description: Option<&str>) -> &str {
    match description.map(str::trim).filter(|text| !text.is_empty()) {
        Some(text) => text,
        None => "",
    }
}

fn media_path(id: &str, variant: &str) -> String {
    format!("/media/{id}/{variant}")
}

fn source_variant(size: FrameSize) -> &'static str {
    match size {
        FrameSize::Large => "full",
        FrameSize::Thumb => "thumb",
    }
}

fn frame_class(size: FrameSize) -> &'static str {
    match size {
        FrameSize::Large => "photo-frame photo-frame-large",
        FrameSize::Thumb => "photo-frame",
    }
}

fn image_class(size: FrameSize) -> &'static str {
    match size {
        FrameSize::Large => "photo-large",
        FrameSize::Thumb => "photo-thumb",
    }
}

fn avatar_class(size: AvatarSize) -> &'static str {
    match size {
        AvatarSize::Small => "avatar avatar-sm",
        AvatarSize::Regular => "avatar",
        AvatarSize::Large => "avatar avatar-xl",
    }
}

fn monogram_size(size: AvatarSize) -> Monogram {
    match size {
        AvatarSize::Small => Monogram::PersonSmall,
        AvatarSize::Regular => Monogram::Person,
        AvatarSize::Large => Monogram::PersonLarge,
    }
}
