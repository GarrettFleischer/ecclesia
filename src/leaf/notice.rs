//! Notice drafts that leaves attach to an effect. The SDK writes them later.

use super::model::{Effect, NoticeDraft};

pub fn notice(user_id: &str, kind: &str, title: String, body: &str, href: String) -> NoticeDraft {
    NoticeDraft {
        user_id: user_id.into(),
        kind: kind.into(),
        title,
        body: body.into(),
        href,
    }
}

pub fn notice_each_governor(
    effect: &mut Effect,
    governor_ids: &[String],
    kind: &str,
    title: String,
    body: &str,
    href: &str,
) {
    append_governor_notices(effect, governor_ids, kind, title, body, href);
}

fn append_governor_notices(
    effect: &mut Effect,
    governor_ids: &[String],
    kind: &str,
    title: String,
    body: &str,
    href: &str,
) {
    for governor in governor_ids {
        push_governor_notice(effect, governor, kind, title.clone(), body, href);
    }
}

fn push_governor_notice(
    effect: &mut Effect,
    governor: &str,
    kind: &str,
    title: String,
    body: &str,
    href: &str,
) {
    effect
        .notices
        .push(notice(governor, kind, title, body, href.into()));
}
