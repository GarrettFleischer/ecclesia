//! Flash copy. HTTP only ever passes a code; this file owns the sentences.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Flash {
    Ok(String),
    Err(String),
}

impl Flash {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok(_))
    }

    pub fn text(&self) -> &str {
        match self {
            Self::Ok(text) | Self::Err(text) => text,
        }
    }

    pub fn class_name(&self) -> &'static str {
        match self {
            Self::Ok(_) => "flash flash-ok",
            Self::Err(_) => "flash flash-err",
        }
    }
}

pub fn flash_from(ok: Option<String>, err: Option<String>) -> Option<Flash> {
    flash_for(ok, err, None)
}

/// `joined_request` names the church when the page already loaded it.
pub fn flash_for(ok: Option<String>, err: Option<String>, church: Option<&str>) -> Option<Flash> {
    if ok.as_deref() == Some("joined_request") {
        return Some(Flash::Ok(join_request_sent(church)));
    }
    if let Some(code) = err {
        Some(Flash::Err(flash_err(&code)))
    } else {
        ok.map(|code| Flash::Ok(flash_ok(&code)))
    }
}

fn join_request_sent(church: Option<&str>) -> String {
    match church.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => format!("Your request to join {name} has been sent."),
        None => "Your request to join has been sent.".into(),
    }
}

fn flash_ok(code: &str) -> String {
    match code {
        "welcome" => "Account created.".into(),
        "joined_request" => join_request_sent(None),
        "invited" => "Invite sent.".into(),
        "redeemed" => "You're in.".into(),
        "approved" => "Approved.".into(),
        "declined" => "Declined.".into(),
        "need_posted" => "Posted.".into(),
        "prayer_posted" => "Posted.".into(),
        "prayed" => "Prayed.".into(),
        "next" => "Next.".into(),
        "answered" => "Answered.".into(),
        "replied" => "Reply posted.".into(),
        "need_met" => "Need met.".into(),
        "photo_updated" => "Photo updated.".into(),
        "photo_removed" => "Photo removed.".into(),
        "applied" => "Sent.".into(),
        "application_accepted" => "Accepted.".into(),
        "need_closed" => "Closed.".into(),
        "need_reopened" => "Reopened.".into(),
        "endorsed" => "Sent.".into(),
        "endorsement_accepted" => "Accepted.".into(),
        "endorsement_declined" => "Declined.".into(),
        "gift_added" => "Added.".into(),
        "gift_removed" => "Removed.".into(),
        "saved" => "Saved.".into(),
        "church_planted" => "Registered.".into(),
        "left" => "Left.".into(),
        "closed" => "Closed.".into(),
        "transferred" => "Transferred.".into(),
        "moved" => "Moved.".into(),
        "invite_accepted" => "You're in.".into(),
        _ => "Done.".into(),
    }
}

/// Approved photo sentences. `not_found` stays the generic missing-page line.
pub fn photo_error_sentence(code: &str) -> Option<&'static str> {
    Some(match code {
        "unreadable" | "animated" | "type_mismatch" => {
            "We couldn't read that photo. Choose a JPEG, PNG, or WebP."
        }
        "too_many_bytes" => "That photo is larger than 12 MB. Choose a smaller file.",
        "too_many_pixels" => "That photo is too large to process. Choose a smaller image.",
        "storage_unavailable" | "store" => {
            "Photos are unavailable right now. Post without them or try again later."
        }
        "upload_expired" => "That upload expired. Add the photo again.",
        "not_owned" | "not_allowed" => "That photo couldn't be attached. Add it again.",
        "attachments" => "You can add up to five photos.",
        "description" => "That description is too long. Keep it under 300 characters.",
        _ => return None,
    })
}

/// Flash code for a media failure. A missing asset uses `upload_expired`
/// so a missing page can keep `not_found`.
pub fn media_flash_code(error: &ecclesia_sdk::media::MediaError) -> &'static str {
    use ecclesia_sdk::media::MediaError;
    match error {
        MediaError::NotFound => "upload_expired",
        MediaError::ClosingReplyNotOnNeed => "miss",
        MediaError::StorageUnavailable => "storage_unavailable",
        MediaError::Store => "store",
        MediaError::Unreadable => "unreadable",
        MediaError::TooManyBytes => "too_many_bytes",
        MediaError::TooManyPixels => "too_many_pixels",
        MediaError::Animated => "animated",
        MediaError::TypeMismatch => "type_mismatch",
        MediaError::NotAllowed => "not_allowed",
        MediaError::NotOwned => "not_owned",
    }
}

fn flash_err(code: &str) -> String {
    if let Some(sentence) = photo_error_sentence(code) {
        return sentence.into();
    }
    match code {
        "missing" => "Fill in the required fields.".into(),
        "ein" => "Check the employer identification number.".into(),
        "registry" => "Check the state registration number.".into(),
        "address" => "We couldn't place that address. Add the city and ZIP.".into(),
        "postal" => "Check the ZIP code.".into(),
        "service" => "Check the service time.".into(),
        "email" => "An account with that email already exists.".into(),
        "auth" => "Sign in first.".into(),
        "not_found" => "Not found.".into(),
        "forbidden" => "Only the pastor can do that.".into(),
        "steward" => "Author or pastor only.".into(),
        "not_yours" => "Not yours.".into(),
        "self" => "You can't do that for yourself.".into(),
        "already" => "Already done.".into(),
        "not_member" => "Join a church first.".into(),
        "scope" => "This need isn't open to your church.".into(),
        "own_need" => "This is your need.".into(),
        "closed" => "This need is closed.".into(),
        "open" => "This need is open.".into(),
        "archived" => "This need is archived.".into(),
        "prayer_answered" => "This prayer is already answered.".into(),
        "invite" => "No church has that code.".into(),
        "pending" => "Nothing to decide.".into(),
        "bad_email" => "Check the email address.".into(),
        "csrf" => "The form expired. Try again.".into(),
        "tone" => "Write it so it lifts someone up.".into(),
        "rate" => "Try again shortly.".into(),
        "miss" => "Try again.".into(),
        "mail" => "Check your email.".into(),
        "password" => "Pick a stronger password.".into(),
        "pastor" => "Close the church or name the next pastor.".into(),
        "already_pastor" => "That person already pastors this church. Pick someone else.".into(),
        "still_open" => "That church is still open.".into(),
        _ => "That didn't work.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_sec_03_unknown_flash_codes_stay_generic() {
        let phish = flash_from(Some("<script>alert(1)</script>".into()), None);
        assert_eq!(phish.unwrap().text(), "Done.");
        let bait = flash_from(None, Some("Visit https://evil.example".into()));
        assert_eq!(bait.unwrap().text(), "That didn't work.");
        let known = flash_from(Some("saved".into()), None);
        assert_eq!(known.unwrap().text(), "Saved.");
    }

    #[test]
    fn media_flashes_use_the_approved_sentences() {
        assert_eq!(
            flash_from(Some("replied".into()), None).unwrap().text(),
            "Reply posted."
        );
        assert_eq!(
            flash_from(Some("need_met".into()), None).unwrap().text(),
            "Need met."
        );
        assert_eq!(
            flash_from(Some("photo_updated".into()), None)
                .unwrap()
                .text(),
            "Photo updated."
        );
        assert_eq!(
            flash_from(Some("photo_removed".into()), None)
                .unwrap()
                .text(),
            "Photo removed."
        );
        assert_eq!(
            flash_from(None, Some("attachments".into())).unwrap().text(),
            "You can add up to five photos."
        );
        assert_eq!(
            flash_from(None, Some("description".into())).unwrap().text(),
            "That description is too long. Keep it under 300 characters."
        );
        assert_eq!(
            flash_from(None, Some("upload_expired".into()))
                .unwrap()
                .text(),
            "That upload expired. Add the photo again."
        );
        assert_eq!(
            flash_from(None, Some("not_found".into())).unwrap().text(),
            "Not found."
        );
    }

    #[test]
    fn join_request_names_the_church() {
        let named = flash_for(
            Some("joined_request".into()),
            None,
            Some("Grace Fellowship"),
        );
        assert_eq!(
            named.unwrap().text(),
            "Your request to join Grace Fellowship has been sent."
        );
        let plain = flash_from(Some("joined_request".into()), None);
        assert_eq!(plain.unwrap().text(), "Your request to join has been sent.");
    }
}
