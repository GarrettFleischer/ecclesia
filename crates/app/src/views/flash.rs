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
    if let Some(code) = err {
        Some(Flash::Err(flash_err(&code)))
    } else {
        ok.map(|code| Flash::Ok(flash_ok(&code)))
    }
}

fn flash_ok(code: &str) -> String {
    match code {
        "welcome" => "Account created.".into(),
        "joined_request" => "Request sent. The pastor will approve or decline.".into(),
        "invited" => "Invite sent.".into(),
        "redeemed" => "Found it. Accept the invite below.".into(),
        "approved" => "Approved.".into(),
        "declined" => "Declined.".into(),
        "need_posted" => "Posted.".into(),
        "applied" => "Sent. They'll see your offer.".into(),
        "application_accepted" => "Accepted.".into(),
        "need_closed" => "Closed.".into(),
        "endorsed" => "Sent.".into(),
        "endorsement_accepted" => "Accepted.".into(),
        "endorsement_declined" => "Declined.".into(),
        "gift_added" => "Added.".into(),
        "gift_removed" => "Removed.".into(),
        "saved" => "Saved.".into(),
        "church_planted" => "Church added.".into(),
        "invite_accepted" => "You're in.".into(),
        _ => "Done.".into(),
    }
}

fn flash_err(code: &str) -> String {
    match code {
        "missing" => "Fill in the required fields.".into(),
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
        "invite" => "No church has that code.".into(),
        "pending" => "Nothing to decide.".into(),
        "bad_email" => "Check the email address.".into(),
        "csrf" => "The form expired. Try again.".into(),
        "tone" => "Write it so it lifts someone up.".into(),
        "rate" => "Try again shortly.".into(),
        "miss" => "Try again.".into(),
        "mail" => "Check your email.".into(),
        "password" => "Pick a stronger password.".into(),
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
}
