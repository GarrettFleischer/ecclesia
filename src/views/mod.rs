//! Maud pages. Each file owns one surface; loops live in `cards`.

mod cards;
mod churches;
mod draft;
mod flash;
mod home;
mod landing;
mod layout;
mod needs;
mod people;
mod words;

pub use churches::{church_new, church_show, churches_index, the_body};
pub use draft::{
    ChurchDraft, DraftKind, EndorseDraft, GiftDraft, NeedDraft, OfferDraft, ProfileDraft,
    RegisterDraft,
};
pub use flash::{Flash, flash_from};
pub use home::home;
pub use landing::{guest_home, landing};
pub use layout::{Nav, SorrySeat, csrf_input, error_page, page, rewrite_row, sorry_page};
pub use needs::{need_new, need_show};
pub use people::{inbox, me, member_show};
