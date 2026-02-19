#![forbid(unsafe_code)]

mod compile;
mod pass;
mod resource;

pub use compile::{CompiledGraph, GraphCompileError};
pub use pass::{PassId, PassNode, PassRead, PassWrite};
pub use resource::{GraphBufferDesc, GraphImageDesc, ResourceId, ResourceKind};
