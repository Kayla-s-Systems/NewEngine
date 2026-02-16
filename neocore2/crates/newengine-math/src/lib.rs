#![forbid(unsafe_op_in_unsafe_fn)]

pub mod types;

#[cfg(feature = "kernel")]
pub mod kernel;

pub mod prelude {
    #[cfg(feature = "kernel")]
    pub use crate::kernel::{MathError, MathFnDesc, MathFnId, MathRegistry, MathValue, TypeTag};
    pub use crate::types::*;
}
