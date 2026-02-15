#![forbid(unsafe_op_in_unsafe_fn)]

pub mod builtins;

mod component;
mod id;
mod mesh;
mod registry;
mod vertex;

pub use component::Primitive;
pub use id::{fnv1a_64, PrimitiveId};
pub use mesh::PrimitiveMesh;
pub use registry::{PrimitiveBuildError, PrimitiveBuildFn, PrimitiveRegistry};
pub use vertex::PrimitiveVertex;
