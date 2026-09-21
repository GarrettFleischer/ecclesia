use maud::{html, Markup};

use crate::leaf::{DemoSeat, User};

use super::cards::persona_grid;
use super::flash::Flash;
use super::layout::{csrf_input, page, Nav};

pub fn landing(users: &[User], flash: Option<Flash>, csrf: &str, demo: DemoSeat) -> Markup {
    page(
        "The body, together",
        None,
        0,
        Nav::None,
        flash,
        html! {
            section class="hero hero-illum" {
                div class="vesica" aria-hidden="true" {
                    svg viewBox="0 0 80 80" width="80" height="80" {
                        circle cx="40" cy="40" r="30" fill="none" stroke="#b1842c" stroke-width="1.4" {}
                        circle cx="40" cy="40" r="22" fill="none" stroke="#3a4d39" stroke-width="1.1" opacity="0.55" {}
                        path fill="#b1842c" d="M38.8 18h2.4v20.8H62v2.4H41.2V62h-2.4V41.2H18v-2.4h20.8z" {}
                    }
                }
                p class="eyebrow" { "For churches who refuse to be islands" }
                h1 { "The ecclesia is a people, not a campus." }
                hr class="gold-rule";
                p class="lede" {
                    "Each church keeps its own household. Members ask or are invited in, and a pastor approves. "
                    "People name their gifts. Needs are posted in the open. Whoever can help, offers. "
                    "Neighboring churches can see what the next parish cannot carry alone."
                }
            }
            (demo_panel(users, csrf, demo))
            section class="panel" {
                h2 { "Take your own seat" }
                form class="stack" method="post" action="/register" {
                    (csrf_input(csrf))
                    label { "Name" input name="name" required placeholder="Your name" maxlength="80"; }
                    label { "Email" input type="email" name="email" required placeholder="you@church.org" maxlength="120"; }
                    div class="split" {
                        label { "City" input name="city" required placeholder="Cedar Falls" maxlength="80"; }
                        label { "Region" input name="region" required placeholder="Iowa" maxlength="80"; }
                    }
                    label { "How do you serve?"
                        textarea name="bio" rows="3" maxlength="800" placeholder="The gifts you already practice, even if no one has ordained them." {}
                    }
                    button class="btn" type="submit" { "Create my place" }
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
                h2 { "Walk through the Cedar Falls valley" }
                p class="muted" {
                    "Demo seats have no passwords. Enter as a pastor, a member, a neighbor, or the person still waiting in the doorway. Production turns this off."
                }
                (persona_grid(users, csrf))
            }
        },
    }
}
