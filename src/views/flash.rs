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
        "welcome" => "You have a place at the table.".into(),
        "joined_request" => "Your request is with the pastor. They will let you in.".into(),
        "invited" => "The invite is waiting for them.".into(),
        "redeemed" => "This church invited you. Accept it below, or from your home.".into(),
        "approved" => "They are in. The body just got a little less thin.".into(),
        "declined" => "Recorded. No one is left guessing.".into(),
        "need_posted" => "The need is visible to the people you chose.".into(),
        "applied" => "They will see that you can help.".into(),
        "application_accepted" => "Good. Someone is actually coming.".into(),
        "need_closed" => "This need is closed.".into(),
        "endorsed" => "They will decide whether to wear that word.".into(),
        "endorsement_accepted" => "That gift is now on your life in this church.".into(),
        "endorsement_declined" => "You let it go. That is allowed.".into(),
        "gift_added" => "Named. People can find you by it.".into(),
        "gift_removed" => "Removed from your list.".into(),
        "saved" => "Saved.".into(),
        "church_planted" => "The group exists. Invite the first people.".into(),
        "invite_accepted" => "You are in. Look around for who needs you.".into(),
        other => other.to_string(),
    }
}

fn flash_err(code: &str) -> String {
    match code {
        "missing" => "A few required fields are empty.".into(),
        "email" => "That email is already at the table. Switch into that person instead.".into(),
        "auth" => "Sign in first.".into(),
        "not_found" => "We could not find that.".into(),
        "forbidden" => "That is not yours to decide.".into(),
        "self" => "You cannot do that for yourself.".into(),
        "already" => "That is already in motion.".into(),
        "not_member" => "You need an approved place in a church first.".into(),
        "scope" => "This need is not open to you yet.".into(),
        "own_need" => "You posted this need. Wait for someone else.".into(),
        "closed" => "That need is no longer open.".into(),
        "invite" => "That invite code does not match a church.".into(),
        "pending" => "There is nothing pending to decide.".into(),
        "bad_email" => "That does not look like an email we can use.".into(),
        "csrf" => "This form went stale. Refresh and try once more.".into(),
        other => other.to_string(),
    }
}
