#![forbid(unsafe_op_in_unsafe_fn)]
//! Unified first-party NEF8/ListFile and package format registry.
//!
//! The crate is split into descriptor policy, format identity data, registry
//! lookup and binary payload codecs. Public paths remain source-compatible.

mod descriptor;
mod formats;
mod registry;

pub mod ydd_binary;

pub use descriptor::{Nef8FormatSpec, ASSET_BLOB_OUTPUT, DOMAIN_MANIFEST_OUTPUT, NEF8_MAGIC_HEX};
pub use registry::{descriptor_for_extension, descriptors, specs};

pub use formats::neftd;
pub use formats::neitems;
pub use formats::nemat;
pub use formats::nepak;
pub use formats::neui;
pub use formats::ybd;
pub use formats::ybn;
pub use formats::ycd;
pub use formats::ydd;
pub use formats::ydr;
pub use formats::yed;
pub use formats::yfd;
pub use formats::yld;
pub use formats::ymap;
pub use formats::ymf;
pub use formats::ymt;
pub use formats::ypdb;
pub use formats::ysc;
pub use formats::ytd;
pub use formats::ytf;
pub use formats::ytyd;
pub use formats::ytyp;
pub use formats::yvr;
pub use formats::ywr;

#[cfg(test)]
mod tests;
