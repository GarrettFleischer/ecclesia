//! Skins over externals. Leaves never live here.
//!
//! - `clock` / ids come from the machine
//! - `session` signs cookies and checks CSRF
//! - `memory` applies effects in process (tests)
//! - `db` (sqlite) applies effects to disk
//! - `http` maps requests onto those skins

pub mod clock;
pub mod memory;
pub mod push;
pub mod session;
pub mod web_push;
