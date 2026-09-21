//! Chrome: document shell, dock, avatars, CSRF field.

use maud::{html, Markup, DOCTYPE};

use crate::leaf::User;

use super::flash::Flash;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Home,
    Churches,
    Body,
    Inbox,
    You,
    None,
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
pub enum DockIcon {
    Home,
    Church,
    Body,
    Inbox,
    You,
}

pub fn csrf_input(csrf: &str) -> Markup {
    html! { input type="hidden" name="csrf" value=(csrf); }
}

pub fn share_button(label: &str, title: &str, text: &str) -> Markup {
    html! {
        button type="button" class="btn btn-quiet" data-share data-share-title=(title) data-share-text=(text) {
            (label)
        }
    }
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
                meta name="theme-color" content="#243126";
                meta name="mobile-web-app-capable" content="yes";
                meta name="apple-mobile-web-app-capable" content="yes";
                meta name="apple-mobile-web-app-status-bar-style" content="black-translucent";
                meta name="apple-mobile-web-app-title" content="Ecclesia";
                meta name="application-name" content="Ecclesia";
                link rel="manifest" href="/static/manifest.webmanifest";
                link rel="icon" href="/static/favicon.svg" type="image/svg+xml";
                link rel="icon" href="/static/icon-192.png" type="image/png" sizes="192x192";
                link rel="apple-touch-icon" href="/static/apple-touch-icon.png";
                title { (title) " · Ecclesia" }
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin;
                link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Fraunces:opsz,wght@9..144,520;9..144,640&family=Source+Sans+3:ital,wght@0,400;0,600;1,400&display=swap";
                link rel="stylesheet" href="/static/app.css";
                meta name="csrf" content=(csrf);
                meta name="unread" content=(unread);
            }
            body class=(site_class(nav)) {
                (atmosphere())
                (install_bar())
                @if nav != Nav::None {
                    (topbar(user))
                }
                @if let Some(flash) = flash {
                    div class=(flash.class_name()) { (flash.text()) }
                }
                main class={ "sheet page-rise" (sheet_extra(nav)) } {
                    (main)
                }
                @if nav != Nav::None {
                    (dock(nav, unread))
                }
                script src="/static/app.js" defer {}
            }
        }
    }
}

fn install_bar() -> Markup {
    html! {
        div class="install-bar" hidden {
            p { "Add Ecclesia to your home screen." }
            button class="btn" type="button" data-install { "Install" }
            button class="btn btn-quiet" type="button" data-install-dismiss { "Not now" }
        }
    }
}

fn site_class(nav: Nav) -> &'static str {
    match nav {
        Nav::None => "site site-guest",
        Nav::Home => "site site-app site-home",
        Nav::Churches => "site site-app site-churches",
        Nav::Body => "site site-app site-body",
        Nav::Inbox => "site site-app site-inbox",
        Nav::You => "site site-app site-you",
    }
}

fn atmosphere() -> Markup {
    html! {
        div class="atmosphere" aria-hidden="true" {
            span class="orb orb-a" {}
            span class="orb orb-b" {}
            span class="orb orb-c" {}
        }
        div class="grain" aria-hidden="true" {}
    }
}

fn sheet_extra(nav: Nav) -> &'static str {
    match nav {
        Nav::None => " sheet-wide",
        _ => "",
    }
}

fn topbar(user: Option<&User>) -> Markup {
    html! {
        header class="topbar" {
            a class="mark" href="/home" { span class="mark-dot" aria-hidden="true" {} span { "Ecclesia" } }
            @if let Some(user) = user {
                a class="who" href="/me" {
                    span class="avatar" { (initials(&user.name)) }
                    span class="who-name" { (user.name) }
                }
            }
        }
    }
}

fn dock(nav: Nav, unread: i64) -> Markup {
    html! {
        nav class="dock" aria-label="Primary" {
            (dock_link("/home", "Home", DockIcon::Home, DockState::for_nav(nav, Nav::Home), None))
            (dock_link("/churches", "Churches", DockIcon::Church, DockState::for_nav(nav, Nav::Churches), None))
            (dock_link("/the-body", "Nearby", DockIcon::Body, DockState::for_nav(nav, Nav::Body), None))
            (dock_link("/inbox", "Inbox", DockIcon::Inbox, DockState::for_nav(nav, Nav::Inbox), unread_badge(unread)))
            (dock_link("/me", "You", DockIcon::You, DockState::for_nav(nav, Nav::You), None))
        }
    }
}

fn unread_badge(unread: i64) -> Option<i64> {
    if unread > 0 {
        Some(unread)
    } else {
        None
    }
}

fn dock_link(
    href: &str,
    label: &str,
    icon: DockIcon,
    state: DockState,
    badge: Option<i64>,
) -> Markup {
    html! {
        a class={ "dock-link" (dock_class(state)) } href=(href) {
            span class="dock-icon" aria-hidden="true" { (dock_svg(icon)) }
            span { (label) }
            @if let Some(n) = badge {
                span class="badge" { (n) }
            }
        }
    }
}

fn dock_class(state: DockState) -> &'static str {
    match state {
        DockState::Current => " is-active",
        DockState::Idle => "",
    }
}

fn dock_svg(icon: DockIcon) -> Markup {
    match icon {
        DockIcon::Home => {
            html! { svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true" { path fill="currentColor" d="M12 4.2 3.8 11h2.1v8.2h5.1v-5h2v5h5.1V11h2.1L12 4.2z" {} } }
        }
        DockIcon::Church => {
            html! { svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true" { path fill="currentColor" d="M11.2 2.5h1.6v1.6h1.6v1.6h-1.6v1.3L18 9.4V21h-5.1v-5.2h-1.8V21H6V9.4l5.2-2.4V5.7H9.6V4.1h1.6V2.5z" {} } }
        }
        DockIcon::Body => {
            html! { svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true" { path fill="currentColor" d="M8 6.2a2.3 2.3 0 1 1 0 4.6 2.3 2.3 0 0 1 0-4.6zm8 0a2.3 2.3 0 1 1 0 4.6 2.3 2.3 0 0 1 0-4.6zM12 13.4a2.5 2.5 0 1 1 0 5 2.5 2.5 0 0 1 0-5zm-2.7-3.1 1.4 3.3h2.6l1.4-3.3-1.5-.7-1.2 2.1-1.2-2.1z" {} } }
        }
        DockIcon::Inbox => {
            html! { svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true" { path fill="currentColor" d="M3.8 6.2h16.4v4.2h-4.1l-1.3 2.3H9.2L7.9 10.4H3.8V6.2zm0 5.8h3.3l1.4 2.4h6.8l1.4-2.4h3.3v6.8H3.8V12z" {} } }
        }
        DockIcon::You => {
            html! { svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true" { path fill="currentColor" d="M12 4.6a3.6 3.6 0 1 1 0 7.2 3.6 3.6 0 0 1 0-7.2zM5.2 19.8c.9-3.4 4-5.1 6.8-5.1s5.9 1.7 6.8 5.1v1.1H5.2z" {} } }
        }
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

pub fn first_name(name: &str) -> &str {
    name.split_whitespace().next().unwrap_or(name)
}

pub fn error_page(message: &str) -> Markup {
    page(
        "Sorry",
        None,
        0,
        Nav::None,
        None,
        "",
        html! {
            h1 { "Sorry" }
            p { (message) }
            a class="btn" href="/" { "Go home" }
        },
    )
}
