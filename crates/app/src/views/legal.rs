//! Privacy, Terms, and the Give placeholder for guests and signed-in members.

use maud::{Markup, html};

use super::flash::Flash;
use super::layout::{Nav, mark_glyph, page, page_lead};

const SUPPORT_EMAIL: &str = "support@ecclesiatogether.org";
const LEGAL_UPDATED: &str = "Last updated October 7, 2026.";

enum LegalInline {
    Plain(&'static str),
    Email,
}

enum LegalBlock {
    Paragraph(&'static [LegalInline]),
    List(&'static [&'static str]),
}

struct LegalSection {
    heading: &'static str,
    blocks: &'static [LegalBlock],
}

struct LegalPage {
    title: &'static str,
    intro: &'static [LegalInline],
    sections: &'static [LegalSection],
}

/// Privacy policy page.
pub fn privacy(flash: Option<Flash>, csrf: &str) -> Markup {
    legal_shell("Privacy", flash, csrf, &PRIVACY_PAGE)
}

/// Terms of use page.
pub fn terms(flash: Option<Flash>, csrf: &str) -> Markup {
    legal_shell("Terms", flash, csrf, &TERMS_PAGE)
}

/// Give placeholder. No payment form until a charity can receive the gift.
pub fn give(flash: Option<Flash>, csrf: &str) -> Markup {
    page(
        "Give",
        None,
        0,
        Nav::Legal,
        flash,
        csrf,
        html! {
            (home_mark())
            (page_lead("Give"))
            article class="legal" {
                p { "Ecclesia is free for churches and members. Giving is not set up yet." }
                p { "A gift made here is not tax-deductible until Ecclesia is a recognized charity." }
                p {
                    "Questions go to "
                    a href={ "mailto:" (SUPPORT_EMAIL) } { (SUPPORT_EMAIL) }
                    "."
                }
            }
        },
    )
}

fn home_mark() -> Markup {
    html! {
        p class="auth-home" {
            a class="mark" href="/" {
                (mark_glyph())
                span { "Ecclesia" }
            }
        }
    }
}

fn legal_shell(title: &str, flash: Option<Flash>, csrf: &str, deck: &'static LegalPage) -> Markup {
    page(
        title,
        None,
        0,
        Nav::Legal,
        flash,
        csrf,
        html! {
            (home_mark())
            (page_lead(deck.title))
            (legal_article(deck))
        },
    )
}

fn legal_article(deck: &'static LegalPage) -> Markup {
    html! {
        article class="legal" {
            p class="legal-updated" { (LEGAL_UPDATED) }
            (legal_paragraph(deck.intro))
            (legal_sections(deck.sections))
        }
    }
}

fn legal_sections(sections: &'static [LegalSection]) -> Markup {
    html! {
        @for section in sections {
            (legal_section(section))
        }
    }
}

fn legal_section(section: &LegalSection) -> Markup {
    html! {
        h2 { (section.heading) }
        (legal_blocks(section.blocks))
    }
}

fn legal_blocks(blocks: &'static [LegalBlock]) -> Markup {
    html! {
        @for block in blocks {
            (legal_block(block))
        }
    }
}

fn legal_block(block: &LegalBlock) -> Markup {
    match block {
        LegalBlock::Paragraph(inlines) => legal_paragraph(inlines),
        LegalBlock::List(items) => legal_list(items),
    }
}

fn legal_paragraph(inlines: &'static [LegalInline]) -> Markup {
    html! {
        p {
            (legal_inlines(inlines))
        }
    }
}

fn legal_inlines(inlines: &'static [LegalInline]) -> Markup {
    html! {
        @for inline in inlines {
            (legal_inline(inline))
        }
    }
}

fn legal_inline(inline: &LegalInline) -> Markup {
    match inline {
        LegalInline::Plain(text) => html! { (text) },
        LegalInline::Email => html! {
            a href="mailto:support@ecclesiatogether.org" { (SUPPORT_EMAIL) }
        },
    }
}

fn legal_list(items: &'static [&'static str]) -> Markup {
    html! {
        ul {
            @for item in items {
                li { (item) }
            }
        }
    }
}

const PRIVACY_INTRO: &[LegalInline] = &[
    LegalInline::Plain(
        "This page lists what Ecclesia stores, who can see it, and how to remove it. Questions go to ",
    ),
    LegalInline::Email,
    LegalInline::Plain("."),
];

const PRIVACY_PAGE: LegalPage = LegalPage {
    title: "Privacy",
    intro: PRIVACY_INTRO,
    sections: &[
        LegalSection {
            heading: "What you give us",
            blocks: &[LegalBlock::List(&[
                "Your first and last name, email address, and password. The password is stored as a one-way hash, so no one can read it.",
                "A short bio and profile photo, if you add them.",
                "What you post: needs, replies, completion notes, prayer requests, gifts on your profile, endorsements you send, and any photos or photo descriptions you include.",
                "For a church you register: its name, address, service times, description, employer identification number, and state registration details.",
            ])],
        },
        LegalSection {
            heading: "What we record when you use Ecclesia",
            blocks: &[
                LegalBlock::List(&[
                    "For each device you sign in on: when you signed in, the last day you used it, your IP address, and the first 80 characters of your browser's user agent.",
                    "Your location, only when you tap Share location. It's used for that search and stays off your account. The coordinates travel in the page address, so they can appear in server logs.",
                    "Which prayers you marked as prayed, by day.",
                    "A push subscription for each device where you turn on alerts.",
                    "For a photo: its normalized dimensions and file size, who uploaded it, and where it appears. Ecclesia discards the original file and its embedded metadata after creating new WebP images.",
                ]),
                LegalBlock::Paragraph(&[LegalInline::Plain(
                    "One cookie, ecclesia_sid, keeps you signed in for up to 30 days. If you post a prayer without your name, a second cookie on that device lets you manage it. Your browser also remembers whether you dismissed the install bar and the prayer reminder.",
                )]),
                LegalBlock::Paragraph(&[LegalInline::Plain(
                    "Ecclesia has no analytics or ads. Every script and font comes from our own server.",
                )]),
            ],
        },
        LegalSection {
            heading: "Who can see what you post",
            blocks: &[LegalBlock::List(&[
                "Needs: you choose for each one. This church means members of that church. Nearby churches adds members of nearby churches. Everyone means any member of any church on Ecclesia.",
                "Members who can see a need also see its photos, replies, completion, and the names and profile photos of the people who replied. Photos in a reply follow the same audience as the need.",
                "Share links open only for signed-in members who can already see the need.",
                "Prayer requests: a prayer posted with your name is visible to members of your church and of nearby churches, with your name and your church. A prayer posted without your name shows neither, and a member elsewhere can pray for it.",
                "Your profile shows your name, profile photo, bio, churches, gifts, and the endorsements you accepted. Any signed-in member can open it. Your email address stays off your profile.",
                "Pastors see who asked to join their church.",
            ])],
        },
        LegalSection {
            heading: "Services that handle your data",
            blocks: &[
                LegalBlock::List(&[
                    "Fly.io runs Ecclesia in Ashburn, Virginia.",
                    "Neon stores the database in Oregon.",
                    "Upstash caches church listings and counts requests to slow down abuse. The limit on sign-in emails keeps your email address for one hour.",
                    "Cloudflare R2 stores the normalized photos you attach. The bucket is private, and Ecclesia checks who may view a photo before providing a short-lived link.",
                    "Resend sends sign-in links, password resets, and church invites.",
                    "Alerts go through your browser's push service, and through Firebase Cloud Messaging for the installed phone app.",
                    "The U.S. Census Bureau geocoder turns a church's address into map coordinates when the church is registered.",
                ]),
                LegalBlock::Paragraph(&[LegalInline::Plain(
                    "Ecclesia doesn't sell or rent your information.",
                )]),
            ],
        },
        LegalSection {
            heading: "Removing your information",
            blocks: &[
                LegalBlock::List(&[
                    "Leave a church from You. A pastor can close a church.",
                    "Remove a gift from your profile.",
                    "Remove your profile photo or a photo from your own need or reply. A replaced or removed photo is queued for deletion from storage.",
                    "Sign out of one device or all of them from You.",
                    "Turn off alerts on any device.",
                ]),
                LegalBlock::Paragraph(&[
                    LegalInline::Plain("To delete your account or get a copy of your data, email "),
                    LegalInline::Email,
                    LegalInline::Plain(
                        " from the address on your account. We'll confirm when it's done.",
                    ),
                ]),
                LegalBlock::Paragraph(&[LegalInline::Plain(
                    "Photos uploaded but never attached to a post are deleted after 24 hours. When you leave a church or a church closes, the record is kept and marked removed. Needs, replies, completions, prayers, and attached photos stay until you remove them or ask us to delete them. A met need is archived 30 days after it's marked met.",
                )]),
            ],
        },
        LegalSection {
            heading: "Age",
            blocks: &[LegalBlock::Paragraph(&[LegalInline::Plain(
                "Ecclesia is for people 18 and older.",
            )])],
        },
        LegalSection {
            heading: "Changes",
            blocks: &[LegalBlock::Paragraph(&[LegalInline::Plain(
                "When this page changes, the date at the top changes too.",
            )])],
        },
    ],
};

const TERMS_INTRO: &[LegalInline] = &[
    LegalInline::Plain("By creating an account, you agree to these terms. Questions go to "),
    LegalInline::Email,
    LegalInline::Plain("."),
];

const TERMS_PAGE: LegalPage = LegalPage {
    title: "Terms",
    intro: TERMS_INTRO,
    sections: &[
        LegalSection {
            heading: "Who can use Ecclesia",
            blocks: &[LegalBlock::List(&[
                "You're 18 or older.",
                "You use your real name and keep one account.",
                "You keep your password to yourself. You're responsible for what's posted from your account.",
            ])],
        },
        LegalSection {
            heading: "Cost",
            blocks: &[LegalBlock::Paragraph(&[LegalInline::Plain(
                "Ecclesia is free for churches and members.",
            )])],
        },
        LegalSection {
            heading: "What you post",
            blocks: &[
                LegalBlock::Paragraph(&[LegalInline::Plain(
                    "You own what you post, including your photos and descriptions. You give Ecclesia permission to process and store that material and show it to the people you chose.",
                )]),
                LegalBlock::Paragraph(&[LegalInline::Plain("Keep these off Ecclesia:")]),
                LegalBlock::List(&[
                    "Threats, harassment, or hate.",
                    "Someone else's private information without their permission.",
                    "A photo of someone who did not agree to be photographed or shown to that audience.",
                    "Requests to send money to people you haven't met.",
                    "Advertising and spam.",
                    "Anything illegal.",
                ]),
                LegalBlock::Paragraph(&[LegalInline::Plain(
                    "Ecclesia blocks some words automatically. We may remove posts or suspend accounts that break these terms.",
                )]),
            ],
        },
        LegalSection {
            heading: "Churches",
            blocks: &[LegalBlock::List(&[
                "If you register a church, you're its pastor on Ecclesia. You confirm you can represent it and that its details are accurate.",
                "Pastors approve or decline requests to join and can close the church.",
            ])],
        },
        LegalSection {
            heading: "Help between members",
            blocks: &[LegalBlock::Paragraph(&[LegalInline::Plain(
                "Ecclesia connects members of churches. We don't check, supervise, or guarantee the help members offer each other. Follow your church's safety policies, especially around children and vulnerable adults.",
            )])],
        },
        LegalSection {
            heading: "The service",
            blocks: &[LegalBlock::List(&[
                "Ecclesia is provided as is. We work to keep it running and your data safe, and we can't promise it will always be available or free of errors.",
                "To the extent the law allows, Ecclesia isn't liable for indirect or consequential damages, or for what happens between members.",
                "You can stop using Ecclesia at any time.",
            ])],
        },
        LegalSection {
            heading: "Changes",
            blocks: &[LegalBlock::Paragraph(&[LegalInline::Plain(
                "When these terms change, the date at the top changes too. Using Ecclesia after a change means you accept it.",
            )])],
        },
    ],
};
