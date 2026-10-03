//! Maud pages. Each file owns one surface; loops live in `cards`.

mod cards;
mod churches;
mod draft;
mod flash;
mod home;
mod landing;
mod layout;
mod nearby;
mod needs;
mod people;
mod prayers;
mod words;

pub use churches::{church_new, church_show, churches_index, join_church_page, the_body};
pub use draft::{
    ChurchDraft, DraftKind, EndorseDraft, GiftDraft, NeedDraft, OfferDraft, PrayerDraft, ProfileDraft,
    RegisterDraft,
};
pub use flash::{Flash, flash_for, flash_from};
pub use home::home;
pub use landing::{
    forgot_password_page, guest_home, landing, magic_link_page, register_page, reset_password,
    sign_in_page,
};
pub use layout::{
    Nav, SorrySeat, csrf_input, error_page, escape_segment, more_churches, more_needs, more_people,
    page, rewrite_row, sorry_page,
};
pub use nearby::nearby_page;
pub use needs::{need_new, need_show};
pub use prayers::{PrayEmpty, PrayerControls, pray_page, prayer_new, prayer_show};
pub use people::{inbox, me, member_show};
