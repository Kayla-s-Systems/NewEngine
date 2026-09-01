#![forbid(unsafe_op_in_unsafe_fn)]

//! Engine-generic gameplay world runtime, deliberately outside the engine composition root.

pub mod gameplay;

pub use gameplay::*;
