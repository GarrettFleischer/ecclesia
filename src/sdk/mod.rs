//! Skins over externals. Leaves never live here.
//!
//! - `clock` / ids come from the machine
//! - `session` signs cookies and checks CSRF
//! - `memory` applies effects in process (tests)
//! Persistence (`src/db`) and HTTP (`src/http`) are sibling skins, not
//! modules of this crate. They apply effects after a leaf returns.

pub mod clock;
pub mod memory;
pub mod push;
pub mod session;
pub mod web_push;
