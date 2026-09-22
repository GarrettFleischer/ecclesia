//! Church rules. No sockets, no SQL, no clocks, no UUIDs.
//!
//! A domain function takes values and returns `Result<Effect, DomainError>`.
//! The SDK loads those values, then applies the effect to the world.

pub mod auth;
pub mod directory;
pub mod effect;
pub mod flags;
pub mod gifts;
pub mod household;
pub mod membership;
pub mod model;
pub mod need;
pub mod needs;
pub mod notice;
pub mod person;
pub mod rules;
pub mod validate;

pub mod sample;

pub use auth::*;
pub use directory::*;
pub use flags::*;
pub use gifts::*;
pub use membership::*;
pub use model::*;
pub use needs::*;
pub use notice::*;
pub use rules::*;
pub use validate::*;

#[cfg(test)]
mod style;
