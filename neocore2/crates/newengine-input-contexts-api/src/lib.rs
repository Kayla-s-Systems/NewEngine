#![forbid(unsafe_op_in_unsafe_fn)]

//! Input context stack DTOs and `engine.input.contexts` service contract.

use serde::{Deserialize, Serialize};

mod capture;
mod context;
mod contracts;

pub use capture::InputCaptureStateV1;
pub use context::{InputCapturePolicy, InputContext, InputContextLifetime, InputContextStack};
pub use contracts::*;

#[cfg(test)]
mod tests;
