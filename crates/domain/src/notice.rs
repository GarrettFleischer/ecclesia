//! Notice drafts that leaves attach to an effect. The SDK writes them later.

use std::sync::Arc;

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
        title: Arc::from(title),
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
    let title: Arc<str> = Arc::from(title);
    for governor in governor_ids {
        effect.notices.push(NoticeDraft {
            user_id: governor.clone(),
            kind,
            title: title.clone(),
            body,
            href: href.into(),
        });
    }
}
