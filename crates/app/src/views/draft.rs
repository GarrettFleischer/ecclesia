//! Filled-in writing forms after a rewrite, before publish.

use maud::{Markup, html};

use ecclesia_sdk::prelude::VoicePass;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DraftKind {
    Blank,
    Review,
}

impl DraftKind {
    pub fn submit_label(self, fresh: &'static str) -> &'static str {
        match self {
            Self::Blank => fresh,
            Self::Review => "Publish",
        }
    }

    pub fn pass(self) -> VoicePass {
        match self {
            Self::Blank => VoicePass::Review,
            Self::Review => VoicePass::Publish,
        }
    }
}

pub fn voice_pass_input(kind: DraftKind) -> Markup {
    html! { input type="hidden" name="pass" value=(kind.pass().as_str()); }
}

pub fn review_banner(kind: DraftKind) -> Markup {
    match kind {
        DraftKind::Review => html! {
            p class="review-banner" { "Read this through. Edit anything you want. Then publish." }
        },
        DraftKind::Blank => html! {},
    }
}

pub struct RegisterDraft<'a> {
    pub name: &'a str,
    pub email: &'a str,
    pub city: &'a str,
    pub region: &'a str,
    pub bio: &'a str,
    pub kind: DraftKind,
}

impl RegisterDraft<'static> {
    pub fn blank() -> Self {
        Self {
            name: "",
            email: "",
            city: "",
            region: "",
            bio: "",
            kind: DraftKind::Blank,
        }
    }
}

pub struct ChurchDraft<'a> {
    pub name: &'a str,
    pub city: &'a str,
    pub region: &'a str,
    pub gathering: &'a str,
    pub description: &'a str,
    pub kind: DraftKind,
}

impl<'a> ChurchDraft<'a> {
    pub fn blank(city: &'a str, region: &'a str) -> Self {
        Self {
            name: "",
            city,
            region,
            gathering: "",
            description: "",
            kind: DraftKind::Blank,
        }
    }
}

pub struct NeedDraft<'a> {
    pub church_id: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    pub gift_id: &'a str,
    pub scope: &'a str,
    pub kind: DraftKind,
}

impl NeedDraft<'static> {
    pub fn blank(church_id: &str) -> NeedDraft<'_> {
        NeedDraft {
            church_id,
            title: "",
            body: "",
            gift_id: "",
            scope: "church",
            kind: DraftKind::Blank,
        }
    }
}

pub struct OfferDraft<'a> {
    pub message: &'a str,
    pub kind: DraftKind,
}

impl OfferDraft<'static> {
    pub fn blank() -> Self {
        Self {
            message: "",
            kind: DraftKind::Blank,
        }
    }
}

pub struct EndorseDraft<'a> {
    pub skill: &'a str,
    pub note: &'a str,
    pub kind: DraftKind,
}

impl EndorseDraft<'static> {
    pub fn blank() -> Self {
        Self {
            skill: "",
            note: "",
            kind: DraftKind::Blank,
        }
    }
}

pub struct ProfileDraft<'a> {
    pub name: &'a str,
    pub city: &'a str,
    pub region: &'a str,
    pub bio: &'a str,
    pub kind: DraftKind,
}

pub struct GiftDraft<'a> {
    pub gift_id: &'a str,
    pub note: &'a str,
    pub kind: DraftKind,
}

impl GiftDraft<'static> {
    pub fn blank() -> Self {
        Self {
            gift_id: "",
            note: "",
            kind: DraftKind::Blank,
        }
    }
}
