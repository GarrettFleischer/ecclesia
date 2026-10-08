//! Maud pages. Each file owns one surface; loops live in `cards`.

mod cards;
mod churches;
mod conversation;
mod draft;
mod flash;
mod home;
mod landing;
mod layout;
mod legal;
mod nearby;
mod needs;
mod people;
mod prayers;
mod scripture;
mod words;

pub use churches::{church_new, church_show, churches_index, join_church_page, the_body};
pub use conversation::{
    AvatarFace, AvatarSize, GalleryRemoval, KeptPhoto, LoadedReply, NameLink, Photo, ReplyFace,
    avatar_face, conversation_fragment, person_avatar, photo_fields, photo_gallery, photo_viewer,
    photos_from, reply_article,
};
pub use draft::{
    ChurchDraft, DraftKind, EndorseDraft, GiftDraft, NeedDraft, OfferDraft, PrayerDraft,
    ProfileDraft, RegisterDraft, ReplyIntent,
};
pub use flash::{Flash, flash_for, flash_from, media_flash_code, photo_error_sentence};
pub use home::home;
pub use landing::{
    forgot_password_page, guest_home, landing, magic_link_page, register_page, reset_password,
    sign_in_page,
};
pub use layout::{
    Nav, SorrySeat, csrf_input, error_page, escape_segment, more_churches, more_needs, more_people,
    page, rewrite_row, sorry_page,
};
pub use legal::{give, privacy, terms};
pub use nearby::nearby_page;
pub use needs::NeedMark;
pub use needs::{need_new, need_show};
pub use people::{inbox, me, member_show};
pub use prayers::{
    PrayEmpty, PrayerControls, PrayerCount, pray_page, prayer_new, prayer_show, prayer_tally,
};
pub use scripture::ESV_NOTICE;
