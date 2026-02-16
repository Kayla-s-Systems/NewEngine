#![forbid(unsafe_op_in_unsafe_fn)]

mod value;
mod desc;
mod registry;

pub use desc::{MathFnDesc, MathFnFlags, MathFnId};
pub use registry::{MathError, MathRegistry};
pub use value::{MathValue, TypeTag};
