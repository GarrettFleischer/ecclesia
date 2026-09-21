//! Notice drafts that leaves attach to an effect. The SDK writes them later.

use super::model::{Effect, NoticeDraft};

pub fn notice(
    user_id: &str,
    kind: &'static str,
    title: String,
    body: &'static str,
    href: String,
) -> NoticeDraft {
    NoticeDraft {
        user_id: user_id.into(),
        kind,
        title,
        body,
        href,
    }
}

pub fn notice_each_governor(
    effect: &mut Effect,
    governor_ids: &[String],
    kind: &'static str,
    title: String,
    body: &'static str,
    href: &str,
) {
    append_governor_notices(effect, governor_ids, kind, title, body, href);
}

fn append_governor_notices(
    effect: &mut Effect,
    governor_ids: &[String],
    kind: &'static str,
    title: String,
    body: &'static str,
    href: &str,
) {
    let Some((first, rest)) = governor_ids.split_first() else {
        return;
    };
    for governor in rest {
        push_governor_notice(effect, governor, kind, title.clone(), body, href);
    }
    push_governor_notice(effect, first, kind, title, body, href);
}

fn push_governor_notice(
    effect: &mut Effect,
    governor: &str,
    kind: &'static str,
    title: String,
    body: &'static str,
    href: &str,
) {
    effect
        .notices
        .push(notice(governor, kind, title, body, href.into()));
}
