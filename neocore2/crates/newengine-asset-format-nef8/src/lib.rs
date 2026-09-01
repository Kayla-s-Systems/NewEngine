#![forbid(unsafe_op_in_unsafe_fn)]
//! Generic NEF8 domain-body codecs.
//!
//! Asset type identity, extension routing, semantic gateways and schema policy are
//! owned exclusively by StarVault format modules under `PluginsSrc/formats`. This
//! crate intentionally contains no asset-format registry.

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
