#![forbid(unsafe_op_in_unsafe_fn)]

//! Strict binary runtime contract for NEF8 `.ydd` drawable bodies.
//!
//! JSON geometry is intentionally rejected. Legacy assets must be migrated by
//! the offline packer before they enter a runtime asset catalog.

mod decode;
mod types;

pub use decode::decode_ydd_binary_body;
pub use types::{
    YddBinaryDocument, YddBinaryEntry, YddBinaryMesh, YddBinaryVertex, YDD_BINARY_CONTRACT_SPEC,
    YDD_BINARY_ENCODING, YDD_BINARY_SCHEMA_VERSION,
};

#[cfg(test)]
mod tests;
