//! Pure core. No sockets, no SQL, no clocks, no UUIDs.
//!
//! A leaf is a function: values in, `Result<Effect, DomainError>` out.
//! The SDK skin loads those values, then applies the effect to the world.

pub mod model;
pub mod rules;
pub mod stories;
pub mod validate;

pub use model::*;
pub use rules::*;
pub use stories::*;
pub use validate::*;
