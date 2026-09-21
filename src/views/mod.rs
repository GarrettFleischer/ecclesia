//! Maud pages. Each file owns one surface; loops live in `cards`.

mod cards;
mod churches;
mod flash;
mod home;
mod landing;
mod layout;
mod needs;
mod people;

pub use churches::{church_new, church_show, churches_index, the_body};
pub use flash::{flash_from, Flash};
pub use home::home;
pub use landing::landing;
pub use layout::{csrf_input, error_page, page, Nav};
pub use needs::{need_new, need_show};
pub use people::{inbox, me, member_show};
