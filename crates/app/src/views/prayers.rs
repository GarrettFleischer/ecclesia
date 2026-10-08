use maud::{Markup, html};

use ecclesia_sdk::prelude::{PrayerCard, PriorPrayerMark, Viewer, VoiceKind};

use super::conversation::{AvatarSize, avatar_face, person_avatar};

use super::draft::{PrayerDraft, review_banner, voice_pass_input};
use super::flash::Flash;
use super::layout::{Icon, Nav, csrf_input, icon, page, page_lead, rewrite_row};

#[derive(Clone, Copy)]
pub enum PrayEmpty {
    Finished,
    WaitingChurch,
}

#[derive(Clone, Copy)]
pub enum PrayerCount {
    Hidden,
    Shown { others: i64 },
}

pub fn prayer_tally(kind: Option<&str>, prayed_count: i64) -> PrayerCount {
    match PriorPrayerMark::of_kind(kind) {
        PriorPrayerMark::Prayed => PrayerCount::Shown {
            others: prayed_count.saturating_sub(1),
        },
        PriorPrayerMark::None | PriorPrayerMark::Seen => PrayerCount::Hidden,
    }
}

#[derive(Clone, Copy)]
pub enum PrayerControls {
    Mark,
    Answer,
    MarkAndAnswer,
    Quiet,
}

#[derive(Clone, Copy)]
enum PrayHands {
    Quiet,
    Pressed,
}

impl PrayHands {
    fn from_count(tally: PrayerCount) -> Self {
        match tally {
            PrayerCount::Hidden => Self::Quiet,
            PrayerCount::Shown { .. } => Self::Pressed,
        }
    }
}

pub fn pray_page(
    viewer: &Viewer,
    flash: Option<Flash>,
    card: Option<&PrayerCard>,
    tally: PrayerCount,
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
                (prayer_face(card, tally, controls, csrf, None))
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
    tally: PrayerCount,
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
            (prayer_face(card, tally, controls, csrf, place))
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
                    "Without my name"
                }
            }
            button class="btn" type="submit" { (draft.kind.submit_label("Post prayer")) }
        }
    }
}

fn prayer_face(
    card: &PrayerCard,
    tally: PrayerCount,
    controls: PrayerControls,
    csrf: &str,
    place: Option<(&str, &str)>,
) -> Markup {
    let can_pray = matches!(
        controls,
        PrayerControls::Mark | PrayerControls::MarkAndAnswer
    );
    html! {
        article class="card prayer-card" {
            @if card.author_id.is_some() {
                p class="eyebrow" { (card.church_name) }
            }
            p class="lede" { (card.body) }
            @if card.author_id.is_some() || can_pray {
                div class="prayer-foot" {
                    @if let (Some(author_id), Some(name)) = (&card.author_id, &card.author_name) {
                        p class="byline" {
                            (person_avatar(author_id, name, avatar_face(card.author_avatar_id.as_deref()), AvatarSize::Small))
                            a href={ "/members/" (author_id) } { (name) }
                        }
                    }
                    @if can_pray {
                        div class="prayer-react" {
                            (pray_others(tally))
                            (pray_control(PrayHands::from_count(tally), card, csrf, place))
                        }
                    }
                }
            }
            @if let Some(praise) = &card.praise {
                h2 { "Praise report" }
                p { (praise) }
            }
            @if can_pray {
                form class="prayer-next" method="post" action={ "/prayers/" (card.id) "/next" } {
                    (csrf_input(csrf))
                    (place_fields(place))
                    button class="btn btn-quiet" type="submit" { "Next prayer" }
                }
            }
        }
    }
}

fn pray_control(
    hands: PrayHands,
    card: &PrayerCard,
    csrf: &str,
    place: Option<(&str, &str)>,
) -> Markup {
    match hands {
        PrayHands::Quiet => html! {
            form class="pray-form" method="post" action={ "/prayers/" (card.id) "/pray" } {
                (csrf_input(csrf))
                (place_fields(place))
                button class="pray-mark" type="submit" aria-label="Pray" aria-pressed="false" {
                    (icon(Icon::Pray))
                }
            }
        },
        PrayHands::Pressed => html! {
            button class="pray-mark is-pressed" type="button" aria-label="Pray" aria-pressed="true" disabled {
                (icon(Icon::Pray))
            }
        },
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

fn pray_others(tally: PrayerCount) -> Markup {
    match tally {
        PrayerCount::Shown { others } if others > 0 => html! {
            span class="pray-count" { (others) }
        },
        PrayerCount::Hidden | PrayerCount::Shown { .. } => html! {},
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
