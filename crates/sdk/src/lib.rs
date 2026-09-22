//! Skins over the machine. Domain never lives here.
//!
//! The App crate talks to this crate only. Store, session, clock, word gate,
//! and story functions sit here.

pub mod cache;
pub mod chat;
pub mod clock;
pub mod db;
pub mod host;
pub mod identity;
pub mod judge;
pub mod limit;
pub mod memory;
pub mod outbox;
pub mod password;
pub mod prelude;
pub mod prompts;
pub mod push;
pub mod refine;
pub mod session;
pub mod story;
pub mod web_push;

pub use cache::Cache;
pub use db::Db;
pub use story::{Sdk, StoryOk};
