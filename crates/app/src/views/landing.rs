use maud::{Markup, html};

use ecclesia_sdk::prelude::VoiceKind;

use super::draft::{RegisterDraft, review_banner, voice_pass_input};
use super::flash::Flash;
use super::layout::{Nav, csrf_input, page, page_lead, rewrite_row};

pub fn landing(flash: Option<Flash>, csrf: &str) -> Markup {
    page(
        "Ecclesia",
        None,
        0,
        Nav::None,
        flash,
        csrf,
        html! {
            (landing_story())
        },
    )
}

pub fn guest_home(flash: Option<Flash>, csrf: &str) -> Markup {
    page(
        "Home",
        None,
        0,
        Nav::Home,
        flash,
        csrf,
        html! {
            (page_lead("Home"))
            div class="empty" {
                p { "Sign in to see open needs from your churches." }
                div class="landing-cta" {
                    a class="btn" href="/register" { "Create an account" }
                    a class="btn btn-quiet" href="/session/new" { "Sign in" }
                }
            }
        },
    )
}

pub fn register_page(flash: Option<Flash>, csrf: &str, draft: &RegisterDraft<'_>) -> Markup {
    auth_page(
        "Create an account",
        flash,
        csrf,
        html! {
            (register_form(csrf, draft))
            (auth_links(&[("/session/new", "Sign in")]))
        },
    )
}

pub fn sign_in_page(flash: Option<Flash>, csrf: &str, email: &str) -> Markup {
    auth_page(
        "Sign in",
        flash,
        csrf,
        html! {
            (sign_in_form(csrf, email))
            (auth_links(&[
                ("/session/link/new", "Email me a link"),
                ("/session/reset/new", "Forgot password"),
                ("/register", "Create an account"),
            ]))
        },
    )
}

pub fn magic_link_page(flash: Option<Flash>, csrf: &str) -> Markup {
    auth_page(
        "Email me a link",
        flash,
        csrf,
        html! {
            p class="lede" { "We email a link when that address is on Ecclesia." }
            (magic_link_form(csrf))
            (auth_links(&[("/session/new", "Sign in")]))
        },
    )
}

pub fn forgot_password_page(flash: Option<Flash>, csrf: &str) -> Markup {
    auth_page(
        "Forgot password",
        flash,
        csrf,
        html! {
            p class="lede" { "We email a reset link when that address is on Ecclesia." }
            (forgot_password_form(csrf))
            (auth_links(&[("/session/new", "Sign in")]))
        },
    )
}

pub fn reset_password(csrf: &str, id: &str, secret: &str) -> Markup {
    auth_page(
        "Set a new password",
        None,
        csrf,
        html! {
            form class="stack panel auth-card" method="post" action={"/session/reset/" (id)} {
                (csrf_input(csrf))
                input type="hidden" name="t" value=(secret);
                label { "New password" input type="password" name="password" required autocomplete="new-password"; }
                button class="btn" type="submit" { "Save password" }
            }
        },
    )
}

fn auth_page(title: &str, flash: Option<Flash>, csrf: &str, body: Markup) -> Markup {
    page(
        title,
        None,
        0,
        Nav::Account,
        flash,
        csrf,
        html! {
            (account_mark())
            (page_lead(title))
            (body)
        },
    )
}

fn account_mark() -> Markup {
    html! {
        p class="auth-home" {
            a class="mark" href="/" {
                span class="mark-dot" aria-hidden="true" {}
                span { "Ecclesia" }
            }
        }
    }
}

fn auth_links(links: &[(&str, &str)]) -> Markup {
    html! {
        p class="auth-links" {
            @for (href, label) in links {
                a href=(href) { (label) }
            }
        }
    }
}

fn landing_story() -> Markup {
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
            div class="landing-cta" {
                a class="btn" href="/register" { "Create an account" }
                a class="btn btn-quiet" href="/session/new" { "Sign in" }
            }
        }
    }
}

fn sign_in_form(csrf: &str, email: &str) -> Markup {
    html! {
        form class="stack panel auth-card" method="post" action="/session" {
            (csrf_input(csrf))
            label { "Email" input type="email" name="email" required autocomplete="email" placeholder="you@church.org" value=(email); }
            label { "Password" input type="password" name="password" required autocomplete="current-password"; }
            button class="btn" type="submit" { "Sign in" }
        }
    }
}

fn magic_link_form(csrf: &str) -> Markup {
    html! {
        form class="stack panel auth-card" method="post" action="/session/link" {
            (csrf_input(csrf))
            label { "Email" input type="email" name="email" required autocomplete="email" placeholder="you@church.org"; }
            button class="btn" type="submit" { "Send link" }
        }
    }
}

fn forgot_password_form(csrf: &str) -> Markup {
    html! {
        form class="stack panel auth-card" method="post" action="/session/reset" {
            (csrf_input(csrf))
            label { "Email" input type="email" name="email" required autocomplete="email" placeholder="you@church.org"; }
            button class="btn" type="submit" { "Send reset link" }
        }
    }
}

fn register_form(csrf: &str, draft: &RegisterDraft<'_>) -> Markup {
    html! {
        form class="stack panel auth-card" method="post" action="/register" {
            (csrf_input(csrf))
            (voice_pass_input(draft.kind))
            (review_banner(draft.kind))
            label { "Name" input name="name" required autocomplete="name" autocapitalize="words" placeholder="Miriam Cole" maxlength="80" value=(draft.name); }
            label { "Email" input type="email" name="email" required autocomplete="email" placeholder="you@church.org" maxlength="120" value=(draft.email); }
            div class="split" {
                label { "City" input name="city" required autocomplete="address-level2" autocapitalize="words" placeholder="Cedar Falls" maxlength="80" value=(draft.city); }
                label { "State or region" input name="region" required autocomplete="address-level1" autocapitalize="words" placeholder="Iowa" maxlength="80" value=(draft.region); }
            }
            label { "About you"
                textarea name="bio" rows="3" maxlength="800" placeholder="Two kids. I have a van on Saturdays." { (draft.bio) }
                (rewrite_row(VoiceKind::Bio))
            }
            label { "Password" input type="password" name="password" required autocomplete="new-password"; }
            button class="btn" type="submit" { (draft.kind.submit_label("Create account")) }
        }
    }
}
