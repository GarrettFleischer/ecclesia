use maud::{Markup, html};

use ecclesia_sdk::prelude::{PrayerCard, PrayerSource, Viewer, VoiceKind};

use super::draft::{PrayerDraft, review_banner, voice_pass_input};
use super::flash::Flash;
use super::layout::{Nav, csrf_input, page, page_lead, rewrite_row};

#[derive(Clone, Copy)]
pub enum PrayEmpty {
    Finished,
    WaitingChurch,
}

#[derive(Clone, Copy)]
pub enum PrayerControls {
    Mark,
    Answer,
    MarkAndAnswer,
    Quiet,
}

pub fn pray_page(
    viewer: &Viewer,
    flash: Option<Flash>,
    card: Option<&PrayerCard>,
    source: Option<PrayerSource>,
    empty: PrayEmpty,
    controls: PrayerControls,
    unread: i64,
    csrf: &str,
) -> Markup {
    page(
        "Pray",
        Some(&viewer.user),
        unread,
        Nav::Pray,
        flash,
        csrf,
        html! {
            div class="page-head" {
                div { (page_lead("Pray")) }
                a class="btn" href="/prayers/new" { "Ask for prayer" }
            }
            @if let Some(card) = card {
                (prayer_face(card, source, controls, csrf, None))
            } @else {
                (pray_empty(empty))
            }
            @if viewer.is_active_anywhere() {
                (pray_toast())
            }
        },
    )
}

pub fn prayer_show(
    viewer: &Viewer,
    flash: Option<Flash>,
    card: &PrayerCard,
    controls: PrayerControls,
    unread: i64,
    csrf: &str,
    place: Option<(&str, &str)>,
    draft_praise: &str,
    praise_kind: super::draft::DraftKind,
) -> Markup {
    page(
        "Prayer",
        Some(&viewer.user),
        unread,
        Nav::Pray,
        flash,
        csrf,
        html! {
            (page_lead("Prayer"))
            (prayer_face(card, None, controls, csrf, place))
            (praise_panel(card, controls, csrf, draft_praise, praise_kind))
        },
    )
}

pub fn prayer_new(
    viewer: &Viewer,
    unread: i64,
    flash: Option<Flash>,
    csrf: &str,
    draft: &PrayerDraft<'_>,
) -> Markup {
    page(
        "Ask for prayer",
        Some(&viewer.user),
        unread,
        Nav::Pray,
        flash,
        csrf,
        html! {
            (page_lead("Ask for prayer"))
            (prayer_form(viewer, csrf, draft))
        },
    )
}

fn prayer_form(viewer: &Viewer, csrf: &str, draft: &PrayerDraft<'_>) -> Markup {
    let churches: Vec<_> = viewer.active_churches().collect();
    if churches.is_empty() {
        return html! {
            div class="empty" {
                p { "Join a church first, then ask from there." }
                a class="btn" href="/churches/join" { "Find your church" }
            }
        };
    }
    html! {
        form class="stack" method="post" action="/prayers" {
            (csrf_input(csrf))
            label { "Church"
                select name="church_id" required {
                    (super::cards::church_options(churches, Some(draft.church_id).filter(|id| !id.is_empty())))
                }
            }
            (voice_pass_input(draft.kind))
            (review_banner(draft.kind))
            label { "Prayer"
                textarea name="body" rows="5" required maxlength="2000" placeholder="Surgery on Thursday." { (draft.body) }
                (rewrite_row(VoiceKind::Prayer))
            }
            fieldset class="stack" {
                legend { "Name" }
                label {
                    input type="radio" name="byline" value="signed" checked[draft.byline != "unnamed"];
                    "With my name"
                }
                label {
                    input type="radio" name="byline" value="unnamed" checked[draft.byline == "unnamed"];
                    "No name"
                }
            }
            button class="btn" type="submit" { (draft.kind.submit_label("Post prayer")) }
        }
    }
}

fn prayer_face(
    card: &PrayerCard,
    source: Option<PrayerSource>,
    controls: PrayerControls,
    csrf: &str,
    place: Option<(&str, &str)>,
) -> Markup {
    html! {
        article class="card prayer-card" {
            @if let Some(source) = source {
                p class="eyebrow" { (source_label(source)) }
            }
            p class="lede" { (card.body) }
            p class="meta" {
                a href={ "/churches/" (card.church_id) } { (card.church_name) }
                @if let (Some(author_id), Some(name)) = (&card.author_id, &card.author_name) {
                    " · "
                    a href={ "/members/" (author_id) } { (name) }
                }
            }
            (prayed_line(card.prayed_count))
            @if let Some(praise) = &card.praise {
                h2 { "Praise report" }
                p { (praise) }
            }
            @if matches!(controls, PrayerControls::Mark | PrayerControls::MarkAndAnswer) {
                div class="page-actions" {
                    form method="post" action={ "/prayers/" (card.id) "/pray" } {
                        (csrf_input(csrf))
                        (place_fields(place))
                        button class="btn" type="submit" { "I prayed" }
                    }
                    form method="post" action={ "/prayers/" (card.id) "/next" } {
                        (csrf_input(csrf))
                        (place_fields(place))
                        button class="btn btn-quiet" type="submit" { "Next prayer" }
                    }
                }
            }
        }
    }
}

fn praise_panel(
    card: &PrayerCard,
    controls: PrayerControls,
    csrf: &str,
    praise: &str,
    kind: super::draft::DraftKind,
) -> Markup {
    if !matches!(
        controls,
        PrayerControls::Answer | PrayerControls::MarkAndAnswer
    ) {
        return html! {};
    }
    html! {
        section class="panel" {
            h2 { "Praise report" }
            form class="stack" method="post" action={ "/prayers/" (card.id) "/answer" } {
                (csrf_input(csrf))
                (voice_pass_input(kind))
                (review_banner(kind))
                label { "What happened"
                    textarea name="praise" rows="4" required maxlength="600" placeholder="The surgery went well." { (praise) }
                    (rewrite_row(VoiceKind::Prayer))
                }
                button class="btn" type="submit" { (kind.submit_label("Mark answered")) }
            }
        }
    }
}

fn pray_empty(empty: PrayEmpty) -> Markup {
    match empty {
        PrayEmpty::WaitingChurch => html! {
            div class="empty" {
                p { "Once you're in a church, its prayers show up here." }
                a class="btn" href="/churches/join" { "Find your church" }
            }
        },
        PrayEmpty::Finished => html! {
            div class="empty" {
                p { "You've prayed through today's requests." }
            }
        },
    }
}

fn pray_toast() -> Markup {
    html! {
        div class="pray-toast" data-pray-toast hidden {
            p { "Pray for churches where you are." }
            a class="btn" href="/nearby" { "Nearby" }
            button class="btn btn-quiet" type="button" data-pray-toast-dismiss { "Not now" }
        }
    }
}

fn source_label(source: PrayerSource) -> &'static str {
    match source {
        PrayerSource::Church => "Your church",
        PrayerSource::Surrounding => "Surrounding church",
    }
}

fn prayed_line(count: i64) -> Markup {
    match count {
        0 => html! {},
        1 => html! { p class="meta" { "1 person prayed" } },
        n => html! { p class="meta" { (n) " people prayed" } },
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
