#![forbid(unsafe_op_in_unsafe_fn)]

mod bindings;
mod diagnostics;
mod module;
mod protocol;
mod transport;
mod wire;

pub use bindings::*;
pub use diagnostics::*;
pub use module::*;
pub use protocol::*;
pub use transport::*;
pub use wire::*;

#[cfg(test)]
mod tests;
