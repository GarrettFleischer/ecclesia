//! Photo rules for a need, a reply, and a profile.
//!
//! Domain sees media ids and descriptions. It does not see bytes, MIME types,
//! object keys, or the clock. The SDK loads those facts, calls one story, and
//! applies the effect in one transaction.

use super::effect::{DomainError, Effect, Write};
use super::need::{Need, NeedReply};
use super::person::Viewer;

/// Most photos a need or a reply may carry.
pub const ATTACHMENT_LIMIT: usize = 5;

/// Longest photo description, counted in characters after trimming.
pub const ATTACHMENT_DESCRIPTION_MAX: usize = 300;

/// A staged photo the caller wants to attach.
///
/// # Notes
/// `description` is optional. Blank text is dropped. Domain does not check who
/// uploaded the file; the SDK must pass only ids staged by this viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachmentRef<'a> {
    pub media_id: &'a str,
    pub description: Option<&'a str>,
}

/// A photo that passed the attachment rules, in display order.
///
/// # Notes
/// `position` is zero-based. `description` is trimmed and at most
/// [`ATTACHMENT_DESCRIPTION_MAX`] characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub media_id: String,
    pub position: i64,
    pub description: Option<String>,
}

/// Checks a photo list for a need or a reply.
///
/// # Parameters
/// - `drafts`: ids and optional descriptions, in the order the member chose.
///
/// # Returns
/// Owned attachments with positions `0..n`, or an error that refuses the whole list.
///
/// # Notes
/// Zero photos is success. Six photos, a blank id, a repeated id, or a long
/// description returns no list, so the caller must not write a prefix of it.
///
/// # Examples
/// ```
/// use ecclesia_domain::{accept_attachments, AttachmentRef};
///
/// let photos = [AttachmentRef {
///     media_id: "m1",
///     description: Some("  roof "),
/// }];
/// let accepted = accept_attachments(&photos).unwrap();
/// assert_eq!(accepted[0].position, 0);
/// assert_eq!(accepted[0].description.as_deref(), Some("roof"));
/// ```
pub fn accept_attachments(drafts: &[AttachmentRef<'_>]) -> Result<Vec<Attachment>, DomainError> {
    if drafts.len() > ATTACHMENT_LIMIT {
        return Err(DomainError::TooManyAttachments);
    }
    collect_attachments(drafts)
}

/// The author detaches one photo from their need.
///
/// # Parameters
/// - `viewer`: the person removing the photo. Must be the need's author.
/// - `need`: the need that carries the photo.
/// - `media_id`: the attachment to detach.
///
/// # Returns
/// An effect that removes the need-to-photo link, or [`DomainError::NotAuthor`].
///
/// # Notes
/// The write does not delete the stored asset. Another need or reply may still use it.
pub fn remove_need_photo(
    viewer: &Viewer,
    need: &Need,
    media_id: &str,
) -> Result<Effect, DomainError> {
    require_author(&viewer.user.id, &need.author_id)?;
    let media_id = require_media_id(media_id)?;
    Ok(Effect::write(Write::DetachNeedMedia {
        need_id: need.id.clone(),
        media_id,
    }))
}

/// The reply's author detaches one photo from that reply.
///
/// # Parameters
/// - `viewer`: the person removing the photo. Must be the reply's author.
/// - `reply`: the reply that carries the photo.
/// - `media_id`: the attachment to detach.
///
/// # Returns
/// An effect that removes the reply-to-photo link, or [`DomainError::NotAuthor`].
///
/// # Notes
/// The need's author cannot remove a photo from someone else's reply.
/// The write does not delete the stored asset.
pub fn remove_reply_photo(
    viewer: &Viewer,
    reply: &NeedReply,
    media_id: &str,
) -> Result<Effect, DomainError> {
    require_author(&viewer.user.id, &reply.author_id)?;
    let media_id = require_media_id(media_id)?;
    Ok(Effect::write(Write::DetachReplyMedia {
        reply_id: reply.id.clone(),
        media_id,
    }))
}

/// The profile owner removes their photo and returns to initials.
///
/// # Parameters
/// - `viewer`: the signed-in person.
/// - `owner_id`: the profile being changed. It must be `viewer`.
///
/// # Returns
/// An effect that clears the profile photo, or [`DomainError::NotAvatarOwner`].
pub fn remove_avatar_photo(viewer: &Viewer, owner_id: &str) -> Result<Effect, DomainError> {
    require_avatar_owner(&viewer.user.id, owner_id)?;
    Ok(Effect::write(Write::ClearAvatar {
        user_id: viewer.user.id.clone(),
    }))
}

/// The profile owner replaces their photo with one they staged.
///
/// # Parameters
/// - `viewer`: the signed-in person.
/// - `owner_id`: the profile being changed. It must be `viewer`.
/// - `media_id`: the staged photo to show.
///
/// # Returns
/// An effect that points the profile at that photo, or an ownership or id error.
///
/// # Notes
/// Domain does not see the previous photo. The SDK drops the old asset only when
/// nothing else still references it.
pub fn replace_avatar_photo(
    viewer: &Viewer,
    owner_id: &str,
    media_id: &str,
) -> Result<Effect, DomainError> {
    require_avatar_owner(&viewer.user.id, owner_id)?;
    let media_id = require_media_id(media_id)?;
    Ok(Effect::write(Write::SetAvatar {
        user_id: viewer.user.id.clone(),
        media_id,
    }))
}

fn collect_attachments(drafts: &[AttachmentRef<'_>]) -> Result<Vec<Attachment>, DomainError> {
    let mut accepted = Vec::with_capacity(drafts.len());
    for draft in drafts {
        accepted.push(one_attachment(draft, &accepted)?);
    }
    Ok(accepted)
}

fn one_attachment(
    draft: &AttachmentRef<'_>,
    accepted: &[Attachment],
) -> Result<Attachment, DomainError> {
    let media_id = require_media_id(draft.media_id)?;
    if accepted.iter().any(|have| have.media_id == media_id) {
        return Err(DomainError::DuplicateAttachment);
    }
    Ok(Attachment {
        media_id,
        position: accepted.len() as i64,
        description: attachment_description(draft.description)?,
    })
}

pub(crate) fn require_media_id(media_id: &str) -> Result<String, DomainError> {
    let trimmed = media_id.trim();
    if trimmed.is_empty() {
        Err(DomainError::InvalidMediaId)
    } else {
        Ok(trimmed.to_string())
    }
}

fn attachment_description(value: Option<&str>) -> Result<Option<String>, DomainError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > ATTACHMENT_DESCRIPTION_MAX {
        return Err(DomainError::DescriptionTooLong);
    }
    Ok(Some(trimmed.to_string()))
}

fn require_author(viewer_id: &str, author_id: &str) -> Result<(), DomainError> {
    if viewer_id == author_id {
        Ok(())
    } else {
        Err(DomainError::NotAuthor)
    }
}

fn require_avatar_owner(viewer_id: &str, owner_id: &str) -> Result<(), DomainError> {
    if viewer_id == owner_id {
        Ok(())
    } else {
        Err(DomainError::NotAvatarOwner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::{church, user_in_church, viewer_of};
    use crate::{NeedShelf, ReplyKind};

    fn photo(media_id: &'static str, description: Option<&'static str>) -> AttachmentRef<'static> {
        AttachmentRef {
            media_id,
            description,
        }
    }

    fn counted(count: usize) -> Vec<AttachmentRef<'static>> {
        let ids = ["m1", "m2", "m3", "m4", "m5", "m6"];
        ids[..count]
            .iter()
            .map(|media_id| photo(media_id, None))
            .collect()
    }

    #[test]
    fn zero_one_and_five_photos_are_accepted() {
        assert!(accept_attachments(&[]).unwrap().is_empty());

        let one = accept_attachments(&[photo("m1", Some("porch"))]).unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].media_id, "m1");
        assert_eq!(one[0].position, 0);
        assert_eq!(one[0].description.as_deref(), Some("porch"));

        let five = accept_attachments(&counted(5)).unwrap();
        assert_eq!(five.len(), 5);
        assert_eq!(
            five.iter().map(|photo| photo.position).collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4]
        );
    }

    #[test]
    fn a_sixth_photo_is_refused() {
        assert_eq!(
            accept_attachments(&counted(6)),
            Err(DomainError::TooManyAttachments)
        );
    }

    #[test]
    fn descriptions_trim_and_stop_at_three_hundred_characters() {
        let trimmed = accept_attachments(&[photo("m1", Some("  porch roof  "))]).unwrap();
        assert_eq!(trimmed[0].description.as_deref(), Some("porch roof"));

        let blank = accept_attachments(&[photo("m1", Some("   "))]).unwrap();
        assert!(blank[0].description.is_none());

        let missing = accept_attachments(&[photo("m1", None)]).unwrap();
        assert!(missing[0].description.is_none());

        let exact: String = "é".repeat(ATTACHMENT_DESCRIPTION_MAX);
        let accepted = accept_attachments(&[AttachmentRef {
            media_id: "m1",
            description: Some(&exact),
        }])
        .unwrap();
        assert_eq!(
            accepted[0].description.as_deref().unwrap().chars().count(),
            ATTACHMENT_DESCRIPTION_MAX
        );

        let over: String = "a".repeat(ATTACHMENT_DESCRIPTION_MAX + 1);
        assert_eq!(
            accept_attachments(&[AttachmentRef {
                media_id: "m1",
                description: Some(&over),
            }]),
            Err(DomainError::DescriptionTooLong)
        );
    }

    #[test]
    fn a_blank_or_repeated_id_refuses_the_list() {
        assert_eq!(
            accept_attachments(&[photo("  ", None)]),
            Err(DomainError::InvalidMediaId)
        );
        assert_eq!(
            accept_attachments(&[photo(" m1 ", None), photo("m1", None)]),
            Err(DomainError::DuplicateAttachment)
        );
    }

    fn dinner(author: &str) -> Need {
        Need {
            id: "need_dinners".into(),
            church_id: "grace".into(),
            author_id: author.into(),
            title: "Dinners".into(),
            body: "This week.".into(),
            gift_id: None,
            scope: "church".into(),
            status: "open".into(),
            created_at: "t0".into(),
            closed_at: None,
            praise: None,
            shelf: NeedShelf::Listed,
        }
    }

    fn reply(author: &str) -> NeedReply {
        NeedReply {
            id: "r1".into(),
            need_id: "need_dinners".into(),
            author_id: author.into(),
            kind: ReplyKind::Message,
            body: "Thursday.".into(),
            created_at: "t1".into(),
        }
    }

    #[test]
    fn only_the_author_removes_a_need_photo() {
        let miriam = viewer_of(
            user_in_church("miriam", "grace", "owner", "active"),
            Some(church("grace")),
        );
        let pastor = viewer_of(
            user_in_church("peter", "grace", "owner", "active"),
            Some(church("grace")),
        );
        let need = dinner("miriam");
        let effect = remove_need_photo(&miriam, &need, " m1 ").unwrap();
        assert_eq!(
            effect.writes,
            vec![Write::DetachNeedMedia {
                need_id: "need_dinners".into(),
                media_id: "m1".into(),
            }]
        );
        assert_eq!(
            remove_need_photo(&pastor, &need, "m1"),
            Err(DomainError::NotAuthor)
        );
        assert_eq!(
            remove_need_photo(&miriam, &need, " "),
            Err(DomainError::InvalidMediaId)
        );
    }

    #[test]
    fn only_the_reply_author_removes_a_reply_photo() {
        let elena = viewer_of(
            user_in_church("elena", "grace", "member", "active"),
            Some(church("grace")),
        );
        let miriam = viewer_of(
            user_in_church("miriam", "grace", "owner", "active"),
            Some(church("grace")),
        );
        let message = reply("elena");
        let effect = remove_reply_photo(&elena, &message, "m2").unwrap();
        assert_eq!(
            effect.writes,
            vec![Write::DetachReplyMedia {
                reply_id: "r1".into(),
                media_id: "m2".into(),
            }]
        );
        assert_eq!(
            remove_reply_photo(&miriam, &message, "m2"),
            Err(DomainError::NotAuthor)
        );
    }

    #[test]
    fn only_the_profile_owner_removes_or_replaces_an_avatar() {
        let miriam = viewer_of(
            user_in_church("miriam", "grace", "owner", "active"),
            Some(church("grace")),
        );
        let cleared = remove_avatar_photo(&miriam, "miriam").unwrap();
        assert_eq!(
            cleared.writes,
            vec![Write::ClearAvatar {
                user_id: "miriam".into(),
            }]
        );
        let replaced = replace_avatar_photo(&miriam, "miriam", " avatar ").unwrap();
        assert_eq!(
            replaced.writes,
            vec![Write::SetAvatar {
                user_id: "miriam".into(),
                media_id: "avatar".into(),
            }]
        );
        assert_eq!(
            remove_avatar_photo(&miriam, "elena"),
            Err(DomainError::NotAvatarOwner)
        );
        assert_eq!(
            replace_avatar_photo(&miriam, "elena", "avatar"),
            Err(DomainError::NotAvatarOwner)
        );
        assert_eq!(
            replace_avatar_photo(&miriam, "miriam", " "),
            Err(DomainError::InvalidMediaId)
        );
    }
}
