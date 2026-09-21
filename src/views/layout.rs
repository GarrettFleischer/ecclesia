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

pub fn page(
    title: &str,
    user: Option<&User>,
    unread: i64,
    nav: Nav,
    flash: Option<Flash>,
    main: Markup,
) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover";
                meta name="theme-color" content="#2a3729";
                meta name="apple-mobile-web-app-capable" content="yes";
                meta name="apple-mobile-web-app-status-bar-style" content="black-translucent";
                meta name="apple-mobile-web-app-title" content="Ecclesia";
                link rel="manifest" href="/static/manifest.webmanifest";
                link rel="icon" href="/static/favicon.svg" type="image/svg+xml";
                title { (title) " · Ecclesia" }
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin;
                link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Fraunces:opsz,wght@9..144,520;9..144,640&family=Source+Sans+3:ital,wght@0,400;0,600;1,400&display=swap";
                link rel="stylesheet" href="/static/app.css";
            }
            body class=(site_class(nav)) {
                (atmosphere())
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
            }
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
            (dock_link("/the-body", "The body", DockIcon::Body, DockState::for_nav(nav, Nav::Body), None))
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
            html! { svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" { path d="M4 11.5 12 5l8 6.5V20H4z"; path d="M10 20v-6h4v6"; } }
        }
        DockIcon::Church => {
            html! { svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" { path d="M12 3v4M10 5h4"; path d="M5 21V10l7-5 7 5v11"; path d="M9 21v-6h6v6"; } }
        }
        DockIcon::Body => {
            html! { svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" { circle cx="8" cy="8" r="2.2"; circle cx="16" cy="8" r="2.2"; circle cx="12" cy="16" r="2.2"; path d="M9.7 9.7 11 14.2M14.3 9.7 13 14.2"; } }
        }
        DockIcon::Inbox => {
            html! { svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" { path d="M4 6h16v12H4z"; path d="M4 12h4l2 3h4l2-3h4"; } }
        }
        DockIcon::You => {
            html! { svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" { circle cx="12" cy="8" r="3"; path d="M5 20c1.5-4 12.5-4 14 0"; } }
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
        "Something gave way",
        None,
        0,
        Nav::None,
        None,
        html! {
            h1 { "Something gave way" }
            p { (message) }
            a class="btn" href="/" { "Return" }
        },
    )
}
