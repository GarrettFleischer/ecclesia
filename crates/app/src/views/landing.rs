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

/// Primary join path once the visitor has read what Ecclesia is for.
fn landing_intro_cta() -> Markup {
    html! {
        div class="landing-intro-cta" {
            p class="lede" {
                "Join your community in sharing needs, offering help, and praying for one another."
            }
            div class="landing-cta" {
                a class="btn" href="/register" { "Create an account" }
                a class="btn btn-quiet" href="/session/new" { "Sign in" }
            }
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
            p class="landing-intro" {
                "Ecclesia Together is a shared place where Christians and churches can find one another when help is needed. Here, people can share practical needs, offer what they have to give, and carry one another in prayer\u{2014}whether someone needs a ride after surgery, a meal during a difficult week, help with a home repair, or simply someone willing to listen. The help may already be sitting in the next pew, or at a church down the road. We need a place where needs and willing hands can find each other, where churches can share their gifts and resources, and where no one has to carry a burden alone. That\u{2019}s what it means to live as one body in Christ."
            }
            (landing_intro_cta())
            div class="landing-phones" {
                (handrail_post())
            }
            h2 id="story-title" { "The Church Takes Care of Its Own" }
            p {
                "The church was never meant to be just a place we attend. It is a people who belong to one another."
            }
            p {
                "From the beginning, Christians understood that when someone was in need, the rest of the community had a responsibility to respond. They shared what they had, carried one another\u{2019}s burdens, and made sure people were not left to face hardship alone."
            }
            p {
                "Sometimes that care looks like a ride home after surgery, a meal for an exhausted parent, help repairing a home, or someone willing to sit and listen. These may seem like small things, but to someone who is struggling, they can mean everything."
            }
            p {
                "The challenge is that needs are easy to miss. People don\u{2019}t always know who is struggling, and those who need help don\u{2019}t always know whom to ask."
            }
            div class="landing-phones" {
                (sarah_phone())
            }
            h3 { "No One Should Slip Through the Cracks" }
            p {
                "As the early church grew, some widows were being overlooked in the daily distribution of food. The apostles didn\u{2019}t ignore the problem or leave it for someone else to solve. They organized a response to make sure those women received the care they needed."
            }
            (scripture::verse(&scripture::ACTS_6_1_NEGLECT))
            p { "That example still matters today." }
            p {
                "There will always be people outside our churches who need to experience the love of Christ. Serving them is part of our calling. But so is noticing the people already among us."
            }
            p {
                "A congregation can be busy serving its community while a struggling family, an isolated older member, or someone quietly facing a crisis goes without help. Good intentions matter, but they aren\u{2019}t always enough. We need ways to see the needs around us and connect them with people who can respond."
            }
            div class="landing-phones" {
                (david_phone())
            }
            h2 { "Churches Don\u{2019}t Have to Do It Alone" }
            p {
                "A congregation may have needs it cannot meet by itself. Another church may have willing volunteers, useful skills, available resources, or people ready to serve\u{2014}but no way of knowing that help is needed."
            }
            p {
                "One church may have tools while another needs a repair. One congregation may have volunteers available while another is struggling to care for a family in crisis. Often, the people and resources are already there. They simply aren\u{2019}t connected."
            }
            p {
                "Ecclesia Together helps bridge that gap by giving people and churches a shared place to communicate needs, offer help, and care for one another."
            }
            p {
                "No individual can do everything. No congregation can meet every need alone. But when we work together, we can do far more than we can separately."
            }
            p {
                "The goal isn\u{2019}t to replace the local church. It\u{2019}s to help the wider church live more fully as one body."
            }
            (scripture::verse(&scripture::ACTS_4_34))
            (church_map())
            h2 { "A Church That Cares Is a Witness" }
            p {
                "When Christians notice one another, respond to real needs, and keep showing up for one another, fellowship becomes more than a Sunday gathering."
            }
            p { "It becomes a community." }
            p {
                "Tertullian recorded the reaction of outsiders to the way Christians treated one another:"
            }
            blockquote class="verse" {
                p { "\u{201c}See how they love one another.\u{201d}" }
            }
            (scripture::verse(&scripture::FIRST_CORINTHIANS_12_26))
            p {
                "The way we treat one another says something about the faith we profess. When people see a community that shares its resources, carries its burdens, and refuses to leave others behind, they see the love of Christ made visible."
            }
            p {
                "Caring for one another isn\u{2019}t a distraction from the church\u{2019}s mission. It is one of the ways we live it out."
            }
            h2 { "Prayer Matters" }
            p {
                "Not every burden can be solved with a meal, a ride, or a helping hand. Sometimes people need someone to listen, to remember them, and to pray with them through an uncertain season."
            }
            p {
                "Prayer is part of how we carry one another\u{2019}s burdens. It reminds us that even when we cannot fix a situation, we do not have to face it alone."
            }
            (prayer_phone())
            (scripture::verse(&scripture::GALATIANS_6_2))
            p {
                "Ecclesia Together gives people and congregations a shared place to lift up prayer requests, stand with those who are struggling, and remind one another that they are not forgotten."
            }
            p {
                "Practical help and prayer belong together. Both are ways of putting our faith into action."
            }
            h2 { "One Body" }
            p {
                "Jesus did not call us to live as isolated individuals or as separate congregations that happen to share the same faith. He called us to be His body."
            }
            (scripture::verse(&scripture::FIRST_CORINTHIANS_12_27))
            p {
                "That means caring for the people in our own congregation, the churches around us, and our neighbors beyond our walls."
            }
            p {
                "Some of us can offer time. Others can share a skill, a meal, a ride, a resource, encouragement, or prayer. Each of us has something to give, and each of us may need help along the way."
            }
            p {
                "We don\u{2019}t have to meet every need ourselves. We simply have to be willing to notice, to respond, and to work together."
            }
            p { "The people who can help are already here." }
            p strong { "They just need to find one another." }
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
                (named_byline("sarah", "Sarah", AvatarFace::Initials, "Covenant Church"))
            }
            div class="stack" {
                (plain_reply("mark", "Mark", "St. Luke\u{2019}s", "I can pick you up Thursday."))
                (plain_reply("rachel", "Rachel", "Hope Chapel", "I'll bring dinner Friday."))
                (plain_reply(
                    "tom",
                    "Tom",
                    "Mercy Chapel",
                    "I'll take Saturday. And don't worry about dishes.",
                ))
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
                (named_byline(
                    "naomi-ellis",
                    "Naomi Ellis",
                    AvatarFace::Initials,
                    "St. Luke\u{2019}s",
                ))
            }
            div class="stack" {
                (plain_reply(
                    "david-brennan",
                    "David Brennan",
                    "Grace Fellowship",
                    "My wife has been very sick. I've been home with her.",
                ))
                (plain_reply(
                    "peter-lang",
                    "Peter Lang",
                    "Hope Chapel",
                    "I'll bring dinner tomorrow.",
                ))
                (plain_reply(
                    "hannah-brooks",
                    "Hannah Brooks",
                    "Covenant Church",
                    "I can mow the lawn this weekend.",
                ))
                (plain_reply(
                    "ruth-alvarez",
                    "Ruth Alvarez",
                    "Mercy Chapel",
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
                    SampleByline::Named {
                        church: "Grace Fellowship",
                        name: "Margaret Hale",
                    },
                    "My daughter called last night. We're having a hard time understanding each other right now. I don't know whether to give her space or reach out again.",
                    SampleHands::Quiet,
                ))
                (prayer_card(
                    SampleByline::Anonymous,
                    "I lost my job on Friday. I've been applying all week, but nothing has come through, and I'm trying to figure out what we do next.",
                    SampleHands::Prayed("11"),
                ))
                (prayer_card(
                    SampleByline::Named {
                        church: "Hope Chapel",
                        name: "Claire Bennett",
                    },
                    "My husband starts treatment Monday. I'm trying to keep everything at home running normally for him and the kids.",
                    SampleHands::Quiet,
                ))
                (prayer_card(
                    SampleByline::Named {
                        church: "St. Luke\u{2019}s",
                        name: "Mary Collins",
                    },
                    "One of our older members has been feeling pretty isolated lately. I keep thinking about how easy it is for someone to slip through the cracks.",
                    SampleHands::Prayed("7"),
                ))
                (prayer_card(
                    SampleByline::Named {
                        church: "Covenant Church",
                        name: "Rachel Thompson",
                    },
                    "We're still getting things back in order after the storm. A lot of little repairs have turned into a pretty long list.",
                    SampleHands::Prayed("3"),
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
            p class="shot-time" { "Tuesday at 9:14 AM" }
            h3 class="shot-title" { "Handrail for my front steps" }
            p class="shot-body" {
                "The railing on my front steps came loose last winter. I'm having knee surgery next month and would like to replace it before I come home."
            }
            (photo_gallery(&before, GalleryRemoval::Closed, PhotoReach::Fixed))
            div class="card-foot need-meta" {
                (named_byline(
                    "linda-harper",
                    "Linda Harper",
                    static_avatar("linda-harper", LINDA_AVATAR),
                    "Grace Fellowship",
                ))
                span class="chip" { "Carpentry & repairs" }
            }
            div class="stack" {
                (example_reply(
                    "emily-lunch",
                    "emily-carter",
                    "Emily Carter",
                    "Hope Chapel",
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
                    "Grace Fellowship",
                    ReplyKind::Message,
                    "Saturday works. Three people would be wonderful. Thank you both.",
                    LINDA_AVATAR,
                    NO_PHOTOS,
                ))
                (example_reply(
                    "linda-met",
                    "linda-harper",
                    "Linda Harper",
                    "Grace Fellowship",
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
                (named_byline(
                    "caleb-morgan",
                    "Caleb Morgan",
                    static_avatar("caleb-morgan", CALEB_AVATAR),
                    "Mercy Chapel",
                ))
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
    church: &'static str,
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
        church: Some(church),
    })
}

/// Name with the church under it, in the same markup as Caleb Morgan.
fn named_byline(
    person_id: &'static str,
    name: &'static str,
    face: AvatarFace<'static>,
    church: &'static str,
) -> Markup {
    html! {
        div class="byline" {
            (person_avatar(person_id, name, face, AvatarSize::Small))
            p class="meta" {
                (name)
                span class="reply-church" { (church) }
            }
        }
    }
}

fn plain_reply(
    person_id: &'static str,
    name: &'static str,
    church: &'static str,
    body: &'static str,
) -> Markup {
    html! {
        article class="card" {
            (named_byline(person_id, name, AvatarFace::Initials, church))
            p { (body) }
        }
    }
}

/// A named sample prayer shows its church once. An unnamed prayer shows neither church nor a name.
#[derive(Clone, Copy)]
enum SampleByline {
    Named {
        church: &'static str,
        name: &'static str,
    },
    Anonymous,
}

/// Pressed hands carry a count. Quiet hands do not.
#[derive(Clone, Copy)]
enum SampleHands {
    Quiet,
    Prayed(&'static str),
}

fn prayer_card(byline: SampleByline, request: &'static str, hands: SampleHands) -> Markup {
    let (church, name) = match byline {
        SampleByline::Named { church, name } => (Some(church), Some(name)),
        SampleByline::Anonymous => (None, None),
    };
    html! {
        article class="card prayer-card" {
            @if let Some(church) = church {
                p class="eyebrow" { (church) }
            }
            p class="prayer-request" { (request) }
            div class="prayer-foot" {
                @if let Some(name) = name {
                    p class="byline" {
                        (person_avatar(name, name, AvatarFace::Initials, AvatarSize::Small))
                        (name)
                    }
                }
                div class="prayer-react" {
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
        SampleHands::Prayed(count) => html! {
            span class="pray-mark is-pressed" aria-hidden="true" { (icon(Icon::Pray)) }
            span class="pray-count" { (count) }
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
