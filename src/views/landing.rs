use maud::{Markup, html};

use crate::leaf::{DemoSeat, User, VoiceKind};

use super::cards::persona_grid;
use super::draft::{RegisterDraft, review_banner, voice_pass_input};
use super::flash::Flash;
use super::layout::{Nav, csrf_input, page, page_lead, rewrite_row};

pub fn landing(
    flash: Option<Flash>,
    csrf: &str,
    demo: DemoSeat,
    draft: &RegisterDraft<'_>,
) -> Markup {
    page(
        "Ecclesia",
        None,
        0,
        Nav::None,
        flash,
        csrf,
        html! {
            (landing_story(demo))
            (register_panel(csrf, draft))
        },
    )
}

pub fn guest_home(
    users: &[User],
    flash: Option<Flash>,
    csrf: &str,
    demo: DemoSeat,
    draft: &RegisterDraft<'_>,
) -> Markup {
    page(
        "Home",
        None,
        0,
        Nav::None,
        flash,
        csrf,
        html! {
            (page_lead("Home"))
            (demo_panel(users, csrf, demo))
            (register_panel(csrf, draft))
        },
    )
}

fn landing_story(demo: DemoSeat) -> Markup {
    html! {
        section class="hero hero-illum" {
            div class="vesica" aria-hidden="true" {
                svg viewBox="0 0 80 80" width="80" height="80" {
                    circle cx="40" cy="40" r="30" fill="none" stroke="#b1842c" stroke-width="1.4" {}
                    circle cx="40" cy="40" r="22" fill="none" stroke="#3a4d39" stroke-width="1.1" opacity="0.55" {}
                    path fill="#b1842c" d="M38.8 18h2.4v20.8H62v2.4H41.2V62h-2.4V41.2H18v-2.4h20.8z" {}
                }
            }
            (page_lead("The Body of Christ"))
        }
        section class="landing-story" {
            p {
                "We are the ecclesia, one body called together, bound by a shared inheritance. "
                "We labor together to build His kingdom. Each of us carries something the other lacks. "
                "Each of us lacks something another carries. This is by design."
            }
            p {
                "The first church understood this. They lived as one people, woven together in daily life, "
                "and when one among them had need, the others did not look away. They gave. "
                "They carried each other's burdens as though they were their own, and the world took notice. "
                "That love was the witness."
            }
            p {
                "People came because they were met exactly where they stood, in whatever need or brokenness "
                "had brought them there. They stayed because they discovered they had something to give, "
                "that they belonged as family, not as strangers passing through. A family grew, out of "
                "belonging freely offered and freely returned."
            }
            p {
                "This is still how the body grows. Members carry one another, and in that carrying, "
                "in the quiet, ordinary faithfulness of showing up for each other, the body is built."
            }
        }
        section class="landing-ways" {
            h2 { "Needs and Gifts" }
            p {
                "A member names what they lack. Others answer with what they carry. "
                "The church was never meant to stand as an island, so the need does not stay hidden "
                "within a single congregation's walls. It is seen by the wider body, by every church "
                "bound together in this family, so that no need goes unmet simply because it was unseen."
            }
            (landing_ctas(demo))
        }
    }
}

fn landing_ctas(demo: DemoSeat) -> Markup {
    html! {
        div class="landing-cta" {
            a class="btn" href="#account" { "Create an account" }
            @if let DemoSeat::Open = demo {
                a class="btn btn-quiet" href="/home" { "Open the demo" }
            }
        }
    }
}

fn register_panel(csrf: &str, draft: &RegisterDraft<'_>) -> Markup {
    html! {
        section class="panel" id="account" {
            h2 { "Create an account" }
            form class="stack" method="post" action="/register" {
                (csrf_input(csrf))
                (voice_pass_input(draft.kind))
                (review_banner(draft.kind))
                label { "Name" input name="name" required autocomplete="name" autocapitalize="words" placeholder="Your name" maxlength="80" value=(draft.name); }
                label { "Email" input type="email" name="email" required autocomplete="email" placeholder="you@church.org" maxlength="120" value=(draft.email); }
                div class="split" {
                    label { "City" input name="city" required autocomplete="address-level2" autocapitalize="words" placeholder="Cedar Falls" maxlength="80" value=(draft.city); }
                    label { "State or region" input name="region" required autocomplete="address-level1" autocapitalize="words" placeholder="Iowa" maxlength="80" value=(draft.region); }
                }
                label { "About you"
                    textarea name="bio" rows="3" maxlength="800" placeholder="Two kids. I have a van on Saturdays." { (draft.bio) }
                    (rewrite_row(VoiceKind::Bio))
                }
                button class="btn" type="submit" { (draft.kind.submit_label("Create account")) }
            }
        }
    }
}

fn demo_panel(users: &[User], csrf: &str, demo: DemoSeat) -> Markup {
    match demo {
        DemoSeat::Sealed => html! {},
        DemoSeat::Open => html! {
            section class="panel" {
                h2 { "People in Cedar Falls" }
                p class="muted" { "Pick a person." }
                (persona_grid(users, csrf))
            }
        },
    }
}
