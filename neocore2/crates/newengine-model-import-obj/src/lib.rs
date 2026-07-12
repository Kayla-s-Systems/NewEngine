#![forbid(unsafe_op_in_unsafe_fn)]

//! OBJ/MTL parsing for model construction.
//!
//! This crate is pure import logic: callers provide OBJ text and an MTL loader
//! backed by whichever asset service/provider is active.

mod mesh;
mod mtl;
mod obj;
mod parsing;
mod path;
mod types;

pub use mtl::parse_mtl_text;
pub use obj::decode_obj_with_mtl_loader;
pub use path::{join_logical_path, logical_dir, normalize_logical_path};
pub use types::{ModelMaterialSource, ObjDecodeResult, ObjPart};

#[cfg(test)]
mod tests;
