use maud::{Markup, html};

use ecclesia_sdk::prelude::{Church, ReplyKind};

use super::conversation::{
    AvatarFace, AvatarSize, GalleryRemoval, NameLink, Photo, PhotoReach, ReplyFace, person_avatar,
    photo_gallery, reply_article, static_avatar,
};
use super::draft::RegisterDraft;
use super::flash::Flash;
use super::layout::{
    Icon, Nav, OnboardStep, csrf_input, icon, mark_glyph, onboard_steps, page, page_lead,
    password_field, vesica_lockup,
};
use super::scripture;

/// Public landing. The story under the hero is the only page.
pub fn landing(flash: Option<Flash>, csrf: &str) -> Markup {
    super::layout::page(
        "Ecclesia",
        None,
        0,
        Nav::Landing,
        flash,
        csrf,
        html! {
            (hero())
            (story_sections())
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

pub fn register_page(
    flash: Option<Flash>,
    csrf: &str,
    draft: &RegisterDraft<'_>,
    church: Option<&Church>,
) -> Markup {
    auth_page(
        "You",
        flash,
        csrf,
        html! {
            @if church.is_none() {
                (onboard_steps(OnboardStep::You))
            }
            @if let Some(church) = church {
                article class="card church-pick" {
                    h2 { (church.name) }
                    p class="muted" { (church.address) }
                }
            }
            (register_form(csrf, draft, church))
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
                (mark_glyph())
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

/// Create an account and Sign in on the hero.
fn landing_cta() -> Markup {
    html! {
        div class="landing-cta" {
            a class="btn btn-light" href="/register" { "Create an account" }
            a class="btn btn-glass" href="/session/new" { "Sign in" }
        }
    }
}

fn hero() -> Markup {
    html! {
        section class="hero" {
            div class="hero-scene" aria-hidden="true" {
                span class="hero-rays" {}
            }
            div class="hero-inner" {
                div class="hero-lockup" {
                    (vesica_lockup())
                    p class="hero-name" { "Ecclesia" }
                }
                (page_lead("The Body of Christ"))
                (landing_cta())
            }
            span class="hero-cue" aria-hidden="true" {}
        }
    }
}

fn story_sections() -> Markup {
    html! {
        (narrative())
        (landing_close())
    }
}

fn narrative() -> Markup {
    html! {
        section class="landing-narrative" aria-labelledby="story-title" {
            (scripture::verse(&scripture::ACTS_2_44))
            div class="landing-phones" {
                (handrail_post())
            }
            h2 id="story-title" { "The Church Takes Care of Its Own" }
            p {
                "The church was a community before it was an organization."
            }
            p {
                "From the beginning, Christians understood that they belonged to one another. When someone was in need, the rest of the church did not simply offer sympathy and move on. They stepped in. What one person lacked, another could provide. What one household could not carry, the wider body helped carry."
            }
            div class="landing-phones" {
                (sarah_phone())
            }
            p { "Paul gave that relationship a name:" }
            (scripture::verse(&scripture::FIRST_CORINTHIANS_12_27_YOU))
            p {
                "He went on to explain what that means in practice. The members of a body cannot be indifferent to one another. When one suffers, the others suffer with them. When one is honored, the others rejoice."
            }
            (scripture::verse(&scripture::FIRST_CORINTHIANS_12_26))
            p { "That kind of care sounds simple. Living it out is harder." }
            h3 { "When someone is overlooked, the church responds" }
            p {
                "The need became obvious as the first church in Jerusalem grew."
            }
            p {
                "The Greek-speaking widows among the believers were being overlooked in the daily distribution of food. The problem was not a lack of activity. The church was growing, preaching, gathering, and serving. But some of its own people were being missed."
            }
            p {
                "The apostles did not treat that as someone else's problem. They appointed others to take responsibility for the daily distribution so that no one in the church would be neglected."
            }
            (scripture::verse(&scripture::ACTS_6_1_NEGLECT))
            p { "That is an important picture of what the church is meant to be." }
            p {
                "There will always be people outside the church who need to hear the gospel, receive help, and experience the love of Christ. That work matters enormously."
            }
            p { "But the people already sitting beside us matter too." }
            p {
                "A church can be passionate about reaching its community while quietly overlooking the person sitting in the next pew. It can organize ministries for the city while a widow, a single parent, an elderly member, or a family facing a crisis goes without help."
            }
            div class="landing-phones" {
                (david_phone())
            }
            p {
                "The early church understood that caring for its own was not a distraction from its mission."
            }
            p strong { "It was part of the mission." }
            h2 { "One church, not a collection of islands" }
            p { "There is another problem that is easy to overlook." }
            p {
                "A congregation can love its people, serve faithfully, and still become an island."
            }
            p {
                "The church down the road may have families who need help. Another congregation may have volunteers with skills that could make a difference. One church may have resources sitting unused while another is struggling to meet a need."
            }
            p { "But if they never know about one another, none of that matters." }
            p {
                "The New Testament gives us a picture of a church where people shared what they had so that no one was left in need."
            }
            (scripture::verse(&scripture::ACTS_4_34))
            p { "That does not mean every need disappeared." }
            p { "It means the needs of the community became the concern of the community." }
            (church_map())
            h2 { "A church that cares is a witness" }
            p { "We see the same pattern in the generations that followed." }
            p {
                "Tertullian recorded the reaction of outsiders to the way Christians treated one another:"
            }
            blockquote class="verse" {
                p { "\u{201c}See how they love one another.\u{201d}" }
            }
            p {
                "The care of its own members was something the world did not do. That care was a light, and a witness, before anyone outside had to be told."
            }
            (scripture::verse(&scripture::FIRST_CORINTHIANS_12_26))
            p {
                "When a congregation knows its people, notices when something is wrong, and responds when someone needs help, fellowship becomes more than a Sunday gathering. It becomes a community."
            }
            p {
                "And that kind of love is still one of the clearest ways the church can show the world who Christ is."
            }
            h2 { "Prayer matters" }
            (prayer_phone())
            p { "You may never meet these people." }
            p { "You may never know how their stories turn out." }
            p { "But for a moment, their burden becomes yours." }
            (scripture::verse(&scripture::GALATIANS_6_2))
            p { "The same body that meets practical needs can carry one another in prayer." }
            h2 { "One Body" }
            p {
                "Jesus did not call us to build isolated communities that happen to believe the same things."
            }
            p { "He called us to be His body." }
            (scripture::verse(&scripture::FIRST_CORINTHIANS_12_27))
            p { "That means we belong to one another." }
            p { "We care for the people in our own congregation." }
            p { "We care for the churches around us." }
            p { "We serve the people outside our walls." }
            p { "We do all of it together." }
            p strong {
                "The church is stronger when we stop acting like islands and start living like one body."
            }
            p { "The people who can help are already there." }
            p { "They just need to find each other." }
        }
    }
}

const LINDA_AVATAR: &str = "/static/img/landing/linda-harper-avatar.webp";
const EMILY_AVATAR: &str = "/static/img/landing/emily-carter-avatar.webp";
const CALEB_AVATAR: &str = "/static/img/landing/caleb-morgan-avatar.webp";

const BEFORE_PHOTO: Photo<'static> = Photo {
    id: "linda-handrail-before",
    description: Some(
        "Linda's front steps with the loose metal railing leaning away from the porch.",
    ),
    source: Some("/static/img/landing/linda-handrail-before.webp"),
};

const INSTALLATION_PHOTO: Photo<'static> = Photo {
    id: "linda-handrail-installation",
    description: Some("Caleb and two volunteers anchoring a wooden handrail beside Linda's steps."),
    source: Some("/static/img/landing/linda-handrail-installation.webp"),
};

const FINISHED_PHOTO: Photo<'static> = Photo {
    id: "linda-handrail-finished",
    description: Some("The finished handrail running from the porch to the bottom step."),
    source: Some("/static/img/landing/linda-handrail-finished.webp"),
};

const GROUP_PHOTO: Photo<'static> = Photo {
    id: "linda-handrail-group",
    description: Some("Linda with Caleb, Marcus, Devon, and Emily beside the finished handrail."),
    source: Some("/static/img/landing/linda-handrail-group.webp"),
};

const MET_PHOTOS: [Photo<'static>; 3] = [INSTALLATION_PHOTO, FINISHED_PHOTO, GROUP_PHOTO];
const NO_PHOTOS: &[Photo<'static>] = &[];

enum PhoneHeight {
    Tall,
    Fits,
}

fn demo_phone(height: PhoneHeight, inner: Markup) -> Markup {
    let scale = match height {
        PhoneHeight::Tall => "product-scale product-scale-long",
        PhoneHeight::Fits => "product-scale product-scale-long product-scale-fit",
    };
    html! {
        figure class="product-frame" {
            div class=(scale) {
                div class="product-screen" {
                    (inner)
                }
            }
        }
    }
}

fn sarah_phone() -> Markup {
    demo_phone(
        PhoneHeight::Fits,
        html! {
            h3 class="shot-title" { "Ride home after surgery" }
            p class="shot-body" {
                "I'm having surgery Thursday and could use a ride home afterward, plus maybe a couple of meals while I'm recovering."
            }
            div class="card-foot" {
                span class="byline" {
                    (person_avatar("sarah", "Sarah", AvatarFace::Initials, AvatarSize::Small))
                    "Sarah"
                }
            }
            div class="stack" {
                (plain_reply("mark", "Mark", "I can pick you up Thursday."))
                (plain_reply("rachel", "Rachel", "I'll bring dinner Friday."))
                (plain_reply("tom", "Tom", "I'll take Saturday. And don't worry about dishes."))
            }
        },
    )
}

fn david_phone() -> Markup {
    demo_phone(
        PhoneHeight::Fits,
        html! {
            h3 class="shot-title" { "I haven't seen the Brennans in a while" }
            p class="shot-body" { "It's been some time. Has anyone heard from them?" }
            div class="card-foot" {
                span class="byline" {
                    (person_avatar("naomi-ellis", "Naomi Ellis", AvatarFace::Initials, AvatarSize::Small))
                    "Naomi Ellis"
                }
            }
            div class="stack" {
                (plain_reply(
                    "david-brennan",
                    "David Brennan",
                    "My wife has been very sick. I've been home with her.",
                ))
                (plain_reply("peter-lang", "Peter Lang", "I'll bring dinner tomorrow."))
                (plain_reply(
                    "hannah-brooks",
                    "Hannah Brooks",
                    "I can mow the lawn this weekend.",
                ))
                (plain_reply(
                    "ruth-alvarez",
                    "Ruth Alvarez",
                    "I can sit with her for a few hours so you can get some rest.",
                ))
            }
        },
    )
}

fn prayer_phone() -> Markup {
    demo_phone(
        PhoneHeight::Fits,
        html! {
            div class="prayer-stack" {
                (prayer_card(
                    Some("Grace Fellowship"),
                    "My daughter hasn't spoken to us in six months. I don't know what to do anymore.",
                    Some(("margaret-hale", "Margaret Hale")),
                    SampleHands::Counted("4"),
                ))
                (prayer_card(
                    None,
                    "We've lost several families this year, and our pastor is exhausted.",
                    None,
                    SampleHands::Counted("8"),
                ))
                (prayer_card(
                    Some("St. Luke\u{2019}s"),
                    "Their son died this week.",
                    Some(("james-porter", "James Porter")),
                    SampleHands::Counted("15"),
                ))
                (prayer_card(
                    None,
                    "I'm trying to keep everything together, but I'm running out of strength.",
                    None,
                    SampleHands::Counted("2"),
                ))
                (prayer_card(
                    Some("Hope Chapel"),
                    "My husband starts treatment on Monday. I need him to sleep, and I need to keep the house steady while he is there.",
                    Some(("claire-bennett", "Claire Bennett")),
                    SampleHands::Counted("6"),
                ))
                (prayer_card(
                    None,
                    "My son has stopped coming. He is angry, and I do not want to make it worse by putting my name on this.",
                    None,
                    SampleHands::Quiet,
                ))
            }
        },
    )
}

fn handrail_post() -> Markup {
    let before = [BEFORE_PHOTO];
    demo_phone(
        PhoneHeight::Tall,
        html! {
            p class="shot-church" { "Grace Fellowship" }
            p class="shot-time" { "Tuesday at 9:14 AM" }
            h3 class="shot-title" { "Handrail for my front steps" }
            p class="shot-body" {
                "The railing on my front steps came loose last winter. I'm having knee surgery next month and would like to replace it before I come home."
            }
            (photo_gallery(&before, GalleryRemoval::Closed, PhotoReach::Fixed))
            div class="card-foot need-meta" {
                span class="byline" {
                    (person_avatar(
                        "linda-harper",
                        "Linda Harper",
                        static_avatar("linda-harper", LINDA_AVATAR),
                        AvatarSize::Small,
                    ))
                    "Linda Harper"
                }
                span class="chip" { "Carpentry & repairs" }
                span class="chip" { "Grace Fellowship" }
            }
            div class="stack" {
                (example_reply(
                    "emily-lunch",
                    "emily-carter",
                    "Emily Carter",
                    ReplyKind::Message,
                    "I'd love to bring lunch for any volunteers. Let me know the day and how many.",
                    EMILY_AVATAR,
                    NO_PHOTOS,
                ))
                (caleb_reply())
                (example_reply(
                    "linda-saturday",
                    "linda-harper",
                    "Linda Harper",
                    ReplyKind::Message,
                    "Saturday works. Three people would be wonderful. Thank you both.",
                    LINDA_AVATAR,
                    NO_PHOTOS,
                ))
                (example_reply(
                    "linda-met",
                    "linda-harper",
                    "Linda Harper",
                    ReplyKind::Completion,
                    "The new handrail is in, and it feels solid. Caleb, Marcus, and Devon finished it before lunch. Emily kept everyone fed. Thank you all.",
                    LINDA_AVATAR,
                    &MET_PHOTOS,
                ))
            }
            p class="shot-status" { "Met Saturday at 12:38 PM" }
        },
    )
}

fn caleb_reply() -> Markup {
    html! {
        div class="example-with-gift" {
            article class="card" data-reply="caleb-crew" {
                div class="byline" {
                    (person_avatar(
                        "caleb-morgan",
                        "Caleb Morgan",
                        static_avatar("caleb-morgan", CALEB_AVATAR),
                        AvatarSize::Small,
                    ))
                    p class="meta" {
                        "Caleb Morgan"
                        span class="reply-church" { "Mercy Chapel" }
                    }
                }
                p { "My crew can come Saturday morning. I'll measure first so we bring the right rail and anchors." }
            }
            p class="example-credential" {
                span class="chip" { "Carpentry & repairs" }
                span { "Licensed contractor" }
            }
        }
    }
}

fn example_reply(
    id: &'static str,
    author_id: &'static str,
    author_name: &'static str,
    kind: ReplyKind,
    body: &'static str,
    avatar: &'static str,
    photos: &'static [Photo<'static>],
) -> Markup {
    reply_article(&ReplyFace {
        id,
        author_id,
        author_name,
        name_link: NameLink::Plain,
        kind,
        body,
        photos,
        avatar: static_avatar(author_id, avatar),
        removal: GalleryRemoval::Closed,
        reach: PhotoReach::Fixed,
    })
}

fn plain_reply(person_id: &'static str, name: &'static str, body: &'static str) -> Markup {
    html! {
        article class="card" {
            div class="byline" {
                (person_avatar(person_id, name, AvatarFace::Initials, AvatarSize::Small))
                p class="meta" { (name) }
            }
            p { (body) }
        }
    }
}

#[derive(Clone, Copy)]
enum SampleHands {
    Quiet,
    Counted(&'static str),
}

fn prayer_card(
    church: Option<&'static str>,
    request: &'static str,
    person: Option<(&'static str, &'static str)>,
    hands: SampleHands,
) -> Markup {
    html! {
        article class="card prayer-card" {
            @if let Some(church) = church {
                p class="eyebrow" { (church) }
            }
            p class="prayer-request" { (request) }
            div class="prayer-foot" {
                @if let Some((person_id, name)) = person {
                    p class="byline" {
                        (person_avatar(person_id, name, AvatarFace::Initials, AvatarSize::Small))
                        (name)
                    }
                }
                div class="prayer-react" {
                    @if let SampleHands::Counted(count) = hands {
                        span class="pray-count" { (count) }
                    }
                    (sample_hands(hands))
                }
            }
        }
    }
}

fn sample_hands(hands: SampleHands) -> Markup {
    match hands {
        SampleHands::Quiet => html! {
            span class="pray-mark" aria-hidden="true" { (icon(Icon::Pray)) }
        },
        SampleHands::Counted(_) => html! {
            span class="pray-mark is-pressed" aria-hidden="true" { (icon(Icon::Pray)) }
        },
    }
}

fn church_map() -> Markup {
    html! {
        div class="church-map" {
            svg class="church-map-lines" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true" focusable="false" {
                line class="map-line" x1="50" y1="46" x2="50" y2="12" {}
                line class="map-line" x1="50" y1="46" x2="26" y2="82" {}
                line class="map-line" x1="50" y1="46" x2="74" y2="78" {}
            }
            ul class="church-map-places" {
                li class="map-place map-luke" {
                    span class="map-name" { "St. Luke\u{2019}s" }
                }
                li class="map-place map-home" {
                    span class="map-name" { "Grace Fellowship" }
                    span class="map-note" { "Home church" }
                }
                li class="map-place map-hope" {
                    span class="map-name" { "Hope Chapel" }
                }
                li class="map-place map-covenant" {
                    span class="map-name" { "Covenant Church" }
                    span class="map-need" { "Roof repair" }
                }
            }
        }
    }
}

fn landing_close() -> Markup {
    html! {
        section class="landing-close" {
            div class="landing-cta" {
                a class="btn" href="/register" { "Create an account" }
                a class="btn btn-quiet" href="/session/new" { "Sign in" }
            }
        }
    }
}

fn sign_in_form(csrf: &str, email: &str) -> Markup {
    html! {
        form class="stack panel auth-card" method="post" action="/session" data-auth="sign-in" novalidate {
            (csrf_input(csrf))
            label { "Email" input type="email" name="email" required autocomplete="email" placeholder="you@church.org" maxlength="120" value=(email); }
            (password_field("Password", "password", "current-password"))
            p class="form-error" id="auth-error" data-auth-error role="alert" hidden {}
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

fn register_form(csrf: &str, draft: &RegisterDraft<'_>, church: Option<&Church>) -> Markup {
    html! {
        form class="stack panel auth-card" method="post" action="/register" data-auth="register" novalidate {
            (csrf_input(csrf))
            @if !draft.code.is_empty() {
                input type="hidden" name="code" value=(draft.code);
            }
            label { "First name" input name="first_name" required autocomplete="given-name" autocapitalize="words" placeholder="Miriam" maxlength="80" value=(draft.first_name); }
            label { "Last name" input name="last_name" required autocomplete="family-name" autocapitalize="words" placeholder="Cole" maxlength="80" value=(draft.last_name); }
            label { "Email" input type="email" name="email" required autocomplete="email" placeholder="you@church.org" maxlength="120" value=(draft.email); }
            (password_field("Password", "password", "new-password"))
            p class="form-error" id="auth-error" data-auth-error role="alert" hidden {}
            button class="btn" type="submit" { (register_button(church)) }
        }
    }
}

fn register_button(church: Option<&Church>) -> String {
    match church {
        Some(church) => format!("Join {}", church.name),
        None => "Find your church".to_string(),
    }
}
