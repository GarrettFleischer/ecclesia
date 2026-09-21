//! Pure core. No sockets, no SQL, no clocks, no UUIDs.
//!
//! A leaf is a function: values in, `Result<Effect, DomainError>` out.
//! The SDK skin loads those values, then applies the effect to the world.

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

#[cfg(test)]
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
