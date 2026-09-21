use maud::{html, Markup};

use crate::leaf::{DemoSeat, User, VoiceKind};

use super::cards::persona_grid;
use super::flash::Flash;
use super::layout::{csrf_input, page, rewrite_row, Nav};

pub fn landing(users: &[User], flash: Option<Flash>, csrf: &str, demo: DemoSeat) -> Markup {
    page(
        "Ecclesia",
        None,
        0,
        Nav::None,
        flash,
        csrf,
        html! {
            section class="hero hero-illum" {
                div class="vesica" aria-hidden="true" {
                    svg viewBox="0 0 80 80" width="80" height="80" {
                        circle cx="40" cy="40" r="30" fill="none" stroke="#b1842c" stroke-width="1.4" {}
                        circle cx="40" cy="40" r="22" fill="none" stroke="#3a4d39" stroke-width="1.1" opacity="0.55" {}
                        path fill="#b1842c" d="M38.8 18h2.4v20.8H62v2.4H41.2V62h-2.4V41.2H18v-2.4h20.8z" {}
                    }
                }
                p class="eyebrow" { "Ecclesia" }
                h1 { "Ask for help. Offer yours." }
                hr class="gold-rule";
                p class="lede" {
                    "Someone in your church needs five dinners this week, a ramp by Saturday, "
                    "or a Spanish speaker on Thursday night. Post it here. "
                    "People who can help say so, and churches in the same town see each other's needs too."
                }
            }
            (demo_panel(users, csrf, demo))
            section class="panel" {
                h2 { "Create an account" }
                form class="stack" method="post" action="/register" {
                    (csrf_input(csrf))
                    label { "Name" input name="name" required placeholder="Your name" maxlength="80"; }
                    label { "Email" input type="email" name="email" required placeholder="you@church.org" maxlength="120"; }
                    div class="split" {
                        label { "City" input name="city" required placeholder="Cedar Falls" maxlength="80"; }
                        label { "State or region" input name="region" required placeholder="Iowa" maxlength="80"; }
                    }
                    label { "About you"
                        textarea name="bio" rows="3" maxlength="800" placeholder="A line or two. What you do, what you're good at." {}
                        (rewrite_row(VoiceKind::Bio))
                    }
                    button class="btn" type="submit" { "Create account" }
                }
            }
        },
    )
}

fn demo_panel(users: &[User], csrf: &str, demo: DemoSeat) -> Markup {
    match demo {
        DemoSeat::Sealed => html! {},
        DemoSeat::Open => html! {
            section class="panel" {
                h2 { "Or look around as someone in Cedar Falls" }
                p class="muted" { "Pick a person. No password." }
                (persona_grid(users, csrf))
            }
        },
    }
}
