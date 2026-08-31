#![forbid(unsafe_op_in_unsafe_fn)]
//! Unified first-party NEF8/ListFile and package format registry.
//!
//! The crate is split into descriptor policy, format identity data, registry
//! lookup and binary payload codecs. Public paths remain source-compatible.

mod descriptor;
mod formats;
mod registry;

pub mod fxd_binary;

pub mod ydd_binary;
pub mod yscd_binary;
pub mod yscd_sound_graph;

pub use fxd_binary::{decode_fxd_nef8, encode_fxd_nef8};

pub use ydd_binary::{
    encode_ydd_binary_body, YDD_BINARY_CONTRACT_SPEC, YDD_BINARY_ENCODING,
    YDD_BINARY_SCHEMA_VERSION,
};

pub use yscd_binary::{
    decode_yscd_binary_body, decode_yscd_nef8, encode_yscd_binary_body, YscdAttenuation, YscdClip,
    YscdCue, YscdCueDescriptor, YscdDictionary, YSCD_BINARY_MAGIC, YSCD_BINARY_SCHEMA_VERSION,
};
pub use yscd_sound_graph::{
    YscdBlendPoint, YscdLayerNodeRef, YscdSoundGraph, YscdSoundGraphNode, YscdSoundGraphValueKind,
    YscdWeightedNodeRef,
};

pub use descriptor::{Nef8FormatSpec, ASSET_BLOB_OUTPUT, DOMAIN_MANIFEST_OUTPUT, NEF8_MAGIC_HEX};
pub use registry::{
    default_entry_route_for_content_kind, descriptor_for_extension, descriptors,
    spec_for_content_kind, specs,
};

pub use formats::fxd;
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
pub use formats::yft;
pub use formats::yld;
pub use formats::ymap;
pub use formats::ymf;
pub use formats::ymt;
pub use formats::ypdb;
pub use formats::ysc;
pub use formats::yscd;
pub use formats::ytd;
pub use formats::ytf;
pub use formats::ytyd;
pub use formats::ytyp;
pub use formats::yvr;
pub use formats::ywr;

#[cfg(test)]
mod tests;
