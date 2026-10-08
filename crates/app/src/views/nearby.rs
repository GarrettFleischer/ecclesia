use maud::{Markup, html};

use ecclesia_sdk::prelude::{NeedCard, PrayerCard, Viewer};

use super::cards::{NeedCardPlace, need_card_stack};
use super::flash::Flash;
use super::layout::{Nav, page, page_lead};

pub fn nearby_page(
    viewer: &Viewer,
    flash: Option<Flash>,
    needs: &[NeedCard],
    prayers: &[PrayerCard],
    place: Option<(&str, &str)>,
    unread: i64,
    csrf: &str,
) -> Markup {
    page(
        "Nearby",
        Some(&viewer.user),
        unread,
        Nav::Body,
        flash,
        csrf,
        html! {
            (page_lead("Nearby"))
            @if place.is_none() {
                div class="empty" {
                    p { "Share where you are to see needs and prayers around you." }
                    (share_place("nearby"))
                }
            } @else {
                section {
                    h2 { "Needs" }
                    (nearby_needs(needs, viewer))
                }
                section {
                    h2 { "Prayer requests" }
                    (nearby_prayers(prayers, place))
                }
            }
        },
    )
}

fn nearby_needs(needs: &[NeedCard], viewer: &Viewer) -> Markup {
    let mut cards = needs.iter();
    if cards.next().is_none() {
        return html! {
            div class="empty" { p { "No needs nearby." } }
        };
    }
    need_card_stack(needs.iter(), viewer, NeedCardPlace::Feed)
}

fn nearby_prayers(prayers: &[PrayerCard], place: Option<(&str, &str)>) -> Markup {
    if prayers.is_empty() {
        return html! {
            div class="empty" { p { "No prayers nearby." } }
        };
    }
    html! {
        div class="stack" {
            @for prayer in prayers {
                (prayer_link(prayer, place))
            }
        }
    }
}

fn prayer_link(prayer: &PrayerCard, place: Option<(&str, &str)>) -> Markup {
    let href = match place {
        Some((lat, lng)) => format!("/prayers/{}?lat={lat}&lng={lng}", prayer.id),
        None => format!("/prayers/{}", prayer.id),
    };
    html! {
        a class="card card-link" href=(href) {
            p { (prayer.body) }
            p class="meta" {
                @if prayer.author_id.is_some() {
                    (prayer.church_name)
                    @if let Some(name) = &prayer.author_name {
                        " · " (name)
                    }
                }
            }
        }
    }
}

fn share_place(path: &str) -> Markup {
    html! {
        button type="button" class="btn" data-share-place=(path) { "Share location" }
        p class="muted" data-place-status {}
    }
}
