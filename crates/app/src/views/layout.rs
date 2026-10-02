//! Chrome: document shell, backdrop, top bar, dock, icons, monograms, CSRF field.

use maud::{DOCTYPE, Markup, html};

use ecclesia_sdk::prelude::{User, VoiceKind, display_name};

use super::flash::Flash;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Landing,
    Home,
    Churches,
    Body,
    Inbox,
    You,
    None,
    Account,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DockState {
    Current,
    Idle,
}

impl DockState {
    pub fn for_nav(here: Nav, item: Nav) -> Self {
        if here == item {
            Self::Current
        } else {
            Self::Idle
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Home,
    Church,
    Pin,
    Inbox,
    Person,
    Globe,
    Clock,
    Plus,
    Check,
    Alert,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Monogram {
    PersonSmall,
    Person,
    PersonLarge,
    ChurchSmall,
    Church,
    ChurchLarge,
}

impl Monogram {
    fn class(self) -> &'static str {
        match self {
            Self::PersonSmall => "avatar avatar-sm",
            Self::Person => "avatar",
            Self::PersonLarge => "avatar avatar-xl",
            Self::ChurchSmall => "church-mark church-mark-sm",
            Self::Church => "church-mark",
            Self::ChurchLarge => "church-mark church-mark-lg",
        }
    }
}

const TONES: [&str; 8] = [
    "tone-0", "tone-1", "tone-2", "tone-3", "tone-4", "tone-5", "tone-6", "tone-7",
];

pub fn csrf_input(csrf: &str) -> Markup {
    html! { input type="hidden" name="csrf" value=(csrf); }
}

pub fn rewrite_row(kind: VoiceKind) -> Markup {
    html! {
        span class="rewrite-row" {
            button type="button" class="rewrite" data-rewrite data-kind=(kind.as_str()) { "Rewrite" }
            span class="rewrite-status" hidden {}
        }
    }
}

pub fn password_field(label: &str, name: &str, autocomplete: &str) -> Markup {
    html! {
        div class="password-field" {
            label for=(name) { (label) }
            div class="password-row" {
                input type="password" id=(name) name=(name) required autocomplete=(autocomplete);
                button type="button" class="password-toggle" data-password-toggle aria-label="Show password" aria-pressed="false" {
                    span class="password-icon password-icon-show" aria-hidden="true" {
                        svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" {
                            path d="M1.75 12S5.5 4.75 12 4.75 22.25 12 22.25 12 18.5 19.25 12 19.25 1.75 12 1.75 12z" {}
                            circle cx="12" cy="12" r="3" {}
                        }
                    }
                    span class="password-icon password-icon-hide" hidden aria-hidden="true" {
                        svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" {
                            path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" {}
                            line x1="1" y1="1" x2="23" y2="23" {}
                        }
                    }
                }
            }
        }
    }
}

pub fn page_lead(title: &str) -> Markup {
    html! { h1 { (title) } }
}

pub fn share_button(label: &str, title: &str, text: &str) -> Markup {
    html! {
        button type="button" class="btn btn-quiet" data-share data-share-title=(title) data-share-text=(text) {
            (label)
        }
    }
}

pub fn icon(glyph: Icon) -> Markup {
    html! {
        svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false" {
            (icon_paths(glyph))
        }
    }
}

fn icon_paths(glyph: Icon) -> Markup {
    match glyph {
        Icon::Home => html! {
            path d="M4 10.2 12 4l8 6.2V19a1 1 0 0 1-1 1h-4.5v-5.5h-5V20H5a1 1 0 0 1-1-1z" {}
        },
        Icon::Church => html! {
            path d="M12 2.75v3.5M10.4 4.25h3.2" {}
            path d="M5 20.25V11.5L12 7l7 4.5v8.75" {}
            path d="M3 20.25h18" {}
            path d="M10 20.25v-3.5a2 2 0 0 1 4 0v3.5" {}
        },
        Icon::Pin => html! {
            path d="M12 21s-6.25-5.6-6.25-10.75a6.25 6.25 0 1 1 12.5 0C18.25 15.4 12 21 12 21z" {}
            circle cx="12" cy="10.25" r="2.25" {}
        },
        Icon::Inbox => html! {
            path d="M3.75 13.5 6.1 6.2a1.5 1.5 0 0 1 1.4-.95h9a1.5 1.5 0 0 1 1.4.95l2.35 7.3" {}
            path d="M3.75 13.5v4.75a1.5 1.5 0 0 0 1.5 1.5h13.5a1.5 1.5 0 0 0 1.5-1.5V13.5h-4.6l-1.4 2.25h-4.5L8.35 13.5z" {}
        },
        Icon::Person => html! {
            circle cx="12" cy="8.25" r="3.75" {}
            path d="M4.75 20c.9-3.6 3.8-5.75 7.25-5.75S18.35 16.4 19.25 20" {}
        },
        Icon::Globe => html! {
            circle cx="12" cy="12" r="8.5" {}
            path d="M3.5 12h17M12 3.5c2.4 2.3 3.6 5.1 3.6 8.5s-1.2 6.2-3.6 8.5c-2.4-2.3-3.6-5.1-3.6-8.5S9.6 5.8 12 3.5z" {}
        },
        Icon::Clock => html! {
            circle cx="12" cy="12" r="8.5" {}
            path d="M12 7.5V12l3 2" {}
        },
        Icon::Plus => html! {
            path d="M12 5.5v13M5.5 12h13" {}
        },
        Icon::Check => html! {
            path d="m5.5 12.5 4.25 4.25L18.5 8" {}
        },
        Icon::Alert => html! {
            path d="M12 7.5v5.5M12 16.5h.01" {}
        },
    }
}

pub fn mark_glyph() -> Markup {
    html! {
        svg class="mark-glyph" viewBox="0 0 24 24" width="24" height="24" aria-hidden="true" focusable="false" {
            circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" stroke-width="1.6" {}
            circle cx="12" cy="12" r="3.2" fill="currentColor" {}
        }
    }
}

pub fn monogram(seed: &str, name: &str, shape: Monogram) -> Markup {
    html! {
        span class={ (shape.class()) " " (tone_class(seed)) } aria-hidden="true" { (initials(name)) }
    }
}

fn tone_class(seed: &str) -> &'static str {
    TONES[seed_number(seed) % TONES.len()]
}

fn seed_number(seed: &str) -> usize {
    seed.bytes().fold(17usize, |sum, byte| {
        sum.wrapping_mul(31).wrapping_add(usize::from(byte))
    })
}

pub fn page(
    title: &str,
    user: Option<&User>,
    unread: i64,
    nav: Nav,
    flash: Option<Flash>,
    csrf: &str,
    main: Markup,
) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover";
                meta name="color-scheme" content="light dark";
                meta name="theme-color" media="(prefers-color-scheme: light)" content="#f4f0e8";
                meta name="theme-color" media="(prefers-color-scheme: dark)" content="#0c1411";
                meta name="mobile-web-app-capable" content="yes";
                meta name="apple-mobile-web-app-capable" content="yes";
                meta name="apple-mobile-web-app-status-bar-style" content="default";
                meta name="apple-mobile-web-app-title" content="Ecclesia";
                meta name="application-name" content="Ecclesia";
                link rel="manifest" href="/static/manifest.webmanifest";
                link rel="icon" href="/static/favicon.svg" type="image/svg+xml";
                link rel="icon" href="/static/icon-192.png" type="image/png" sizes="192x192";
                link rel="apple-touch-icon" href="/static/apple-touch-icon.png";
                title { (document_title(title)) }
                link rel="preload" href="/static/fonts/inter-latin.woff2" as="font" type="font/woff2" crossorigin;
                link rel="preload" href="/static/fonts/instrument-serif-normal-latin.woff2" as="font" type="font/woff2" crossorigin;
                (scene_preload(nav))
                link rel="stylesheet" href="/static/app.css?v=13";
                meta name="csrf" content=(csrf);
                meta name="unread" content=(unread);
            }
            body class=(site_class(nav)) {
                a class="skip" href="#content" { "Skip to content" }
                (backdrop())
                @if nav == Nav::Landing {
                    (install_bar())
                    (install_help_dialog())
                }
                @if app_chrome(nav) {
                    (topbar(user))
                }
                @if let Some(flash) = flash {
                    (flash_note(&flash))
                }
                main id="content" class={ "sheet page-rise" (sheet_extra(nav)) } {
                    (main)
                }
                @if app_chrome(nav) {
                    (dock(nav, unread))
                }
                script src="/static/app.js?v=11" defer {}
            }
        }
    }
}

fn scene_preload(nav: Nav) -> Markup {
    if !matches!(nav, Nav::Landing | Nav::Account) {
        return html! {};
    }
    html! {
        link rel="preload" href="/static/img/dawn-tall.webp" as="image" type="image/webp" media="(orientation: portrait), (max-width: 699px)" fetchpriority="high";
        link rel="preload" href="/static/img/dawn-wide.webp" as="image" type="image/webp" media="(orientation: landscape) and (min-width: 700px)" fetchpriority="high";
    }
}

fn backdrop() -> Markup {
    html! {
        div class="backdrop" aria-hidden="true" {
            span class="glow glow-a" {}
            span class="glow glow-b" {}
        }
    }
}

fn install_bar() -> Markup {
    html! {
        div class="install-bar" hidden {
            (mark_glyph())
            p class="install-bar-copy" {
                span class="install-bar-lead" { "Add Ecclesia to your home screen." }
                span class="install-bar-ios" hidden { "Share, then Add to Home Screen." }
            }
            button class="btn btn-quiet install-bar-help" type="button" data-install-help hidden aria-label="Show add to home screen steps" { "?" }
            button class="btn" type="button" data-install { "Install" }
            button class="btn btn-quiet" type="button" data-install-dismiss { "Not now" }
        }
    }
}

fn install_help_dialog() -> Markup {
    html! {
        dialog class="install-help" aria-labelledby="install-help-title" {
            form class="install-help-sheet" method="dialog" {
                header class="install-help-head" {
                    h2 id="install-help-title" { "Add to home screen" }
                    button class="btn btn-quiet install-help-close" type="submit" value="cancel" aria-label="Close dialog" { "Close" }
                }
                ol class="install-help-steps" {
                    li { "In Safari, open Share." }
                    li { "Choose Add to Home Screen." }
                    li { "Tap Add." }
                }
                div class="install-help-figures" {
                    figure data-install-device="iphone" hidden {
                        img src="/static/img/ios-install-iphone-share.svg" alt="Safari bottom bar with Share highlighted" width="390" height="200";
                        figcaption { "Share sits in the bar at the bottom on iPhone." }
                    }
                    figure data-install-device="iphone" hidden {
                        img src="/static/img/ios-install-iphone-add.svg" alt="Share menu with Add to Home Screen highlighted" width="390" height="320";
                        figcaption { "Scroll the menu if you do not see it." }
                    }
                    figure data-install-device="ipad" hidden {
                        img src="/static/img/ios-install-ipad-share.svg" alt="Safari toolbar with Share highlighted" width="390" height="200";
                        figcaption { "Share sits in the toolbar on iPad." }
                    }
                    figure data-install-device="ipad" hidden {
                        img src="/static/img/ios-install-ipad-add.svg" alt="Share menu with Add to Home Screen highlighted" width="390" height="320";
                        figcaption { "Scroll the menu if you do not see it." }
                    }
                }
                button class="btn install-help-done" type="submit" value="ok" { "Close" }
            }
        }
    }
}

fn flash_note(flash: &Flash) -> Markup {
    html! {
        div class=(flash.class_name()) role="status" {
            span class="flash-icon" aria-hidden="true" { (icon(flash_icon(flash))) }
            span class="flash-text" { (flash.text()) }
        }
    }
}

fn flash_icon(flash: &Flash) -> Icon {
    if flash.is_ok() { Icon::Check } else { Icon::Alert }
}

fn site_class(nav: Nav) -> &'static str {
    match nav {
        Nav::Landing => "site site-guest site-landing",
        Nav::None => "site site-guest",
        Nav::Home => "site site-app site-home",
        Nav::Churches => "site site-app site-churches",
        Nav::Body => "site site-app site-body",
        Nav::Inbox => "site site-app site-inbox",
        Nav::You => "site site-app site-you",
        Nav::Account => "site site-guest site-account",
    }
}

fn app_chrome(nav: Nav) -> bool {
    !matches!(nav, Nav::Landing | Nav::None | Nav::Account)
}

fn sheet_extra(nav: Nav) -> &'static str {
    match nav {
        Nav::Landing => " sheet-landing",
        Nav::Account => " sheet-auth",
        _ => "",
    }
}

fn topbar(user: Option<&User>) -> Markup {
    html! {
        header class="topbar" {
            a class="mark" href="/home" { (mark_glyph()) span { "Ecclesia" } }
            @if let Some(user) = user {
                a class="who" href="/me" {
                    (monogram(&user.id, &shown_name(user), Monogram::Person))
                    span class="who-name" { (shown_name(user)) }
                }
            }
        }
    }
}

fn dock(nav: Nav, unread: i64) -> Markup {
    html! {
        nav class="dock" aria-label="Primary" {
            (dock_link("/home", "Home", Icon::Home, DockState::for_nav(nav, Nav::Home), None))
            (dock_link("/churches", "Churches", Icon::Church, DockState::for_nav(nav, Nav::Churches), None))
            (dock_link("/the-body", "Nearby", Icon::Pin, DockState::for_nav(nav, Nav::Body), None))
            (dock_link("/inbox", "Inbox", Icon::Inbox, DockState::for_nav(nav, Nav::Inbox), unread_badge(unread)))
            (dock_link("/me", "You", Icon::Person, DockState::for_nav(nav, Nav::You), None))
        }
    }
}

fn unread_badge(unread: i64) -> Option<i64> {
    if unread > 0 { Some(unread) } else { None }
}

fn dock_link(
    href: &str,
    label: &str,
    glyph: Icon,
    state: DockState,
    badge: Option<i64>,
) -> Markup {
    html! {
        a class={ "dock-link" (dock_class(state)) } href=(href) aria-current=[dock_current(state)] {
            (dock_pill(state))
            span class="dock-icon" aria-hidden="true" { (icon(glyph)) }
            span class="dock-label" { (label) }
            @if let Some(n) = badge {
                span class="badge" { (n) }
            }
        }
    }
}

fn dock_pill(state: DockState) -> Markup {
    match state {
        DockState::Current => html! { span class="dock-pill" aria-hidden="true" {} },
        DockState::Idle => html! {},
    }
}

fn dock_class(state: DockState) -> &'static str {
    match state {
        DockState::Current => " is-active",
        DockState::Idle => "",
    }
}

fn dock_current(state: DockState) -> Option<&'static str> {
    match state {
        DockState::Current => Some("page"),
        DockState::Idle => None,
    }
}

fn document_title(title: &str) -> String {
    if title == "Ecclesia" {
        "Ecclesia".into()
    } else {
        format!("{title} · Ecclesia")
    }
}

pub fn initials(name: &str) -> String {
    take_initials(name.split_whitespace())
}

fn take_initials<'a>(parts: impl Iterator<Item = &'a str>) -> String {
    parts
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

pub fn shown_name(user: &User) -> String {
    display_name(&user.first_name, &user.last_name)
}

pub enum SorrySeat<'a> {
    Guest,
    Member {
        user: &'a User,
        unread: i64,
        csrf: &'a str,
    },
}

pub fn error_page(message: &str) -> Markup {
    sorry_page(message, SorrySeat::Guest)
}

pub fn sorry_page(message: &str, seat: SorrySeat<'_>) -> Markup {
    match seat {
        SorrySeat::Guest => page(
            "Sorry",
            None,
            0,
            Nav::None,
            None,
            "",
            sorry_body(message, "/", "Go home"),
        ),
        SorrySeat::Member { user, unread, csrf } => page(
            "Sorry",
            Some(user),
            unread,
            Nav::Home,
            None,
            csrf,
            sorry_body(message, "/home", "Open needs"),
        ),
    }
}

pub fn more_needs(path: &str, after: Option<&str>) -> Markup {
    more_param(path, "after", "More needs", after)
}

pub fn more_churches(path: &str, after: Option<&str>) -> Markup {
    more_param(path, "after", "More churches", after)
}

pub fn more_people(path: &str, after: Option<&str>) -> Markup {
    more_param(path, "members_after", "More people", after)
}

fn more_param(path: &str, name: &str, label: &str, value: Option<&str>) -> Markup {
    let Some(value) = value else {
        return html! {};
    };
    let href = format!("{path}?{name}={}", encode_cursor(value));
    html! {
        p class="more" { a class="btn btn-quiet" href=(href) { (label) } }
    }
}

fn encode_cursor(raw: &str) -> String {
    raw.replace('|', "%7C")
}

#[cfg(test)]
mod tests {
    use super::{more_churches, more_needs, more_people};

    #[test]
    fn us_read_04_more_links_encode_the_pipe_cursor() {
        let needs = more_needs("/home", Some("2026-09-06T12:00:00Z|need_6")).into_string();
        assert!(needs.contains("More needs"));
        assert!(needs.contains("/home?after=2026-09-06T12:00:00Z%7Cneed_6"));
        let churches = more_churches(
            "/the-body",
            Some("Beta City|Paging Church 20|church_page_20"),
        )
        .into_string();
        assert!(churches.contains("More churches"));
        assert!(churches.contains("after=Beta City%7CPaging Church 20%7Cchurch_page_20"));
        let people = more_people(
            "/churches/church_grace",
            Some("Page Member 18|user_page_18"),
        )
        .into_string();
        assert!(people.contains("More people"));
        assert!(people.contains("members_after=Page Member 18%7Cuser_page_18"));
        assert!(more_needs("/home", None).into_string().is_empty());
    }
}

fn sorry_body(message: &str, href: &str, action: &str) -> Markup {
    html! {
        (page_lead("Sorry"))
        p class="lede" { (message) }
        a class="btn" href=(href) { (action) }
    }
}
