#![forbid(unsafe_op_in_unsafe_fn)]
//! Generic NEF8 domain-body codecs.
//!
//! Asset type identity, extension routing, semantic gateways and schema policy are
//! owned exclusively by StarVault format modules under `PluginsSrc/formats`. This
//! crate intentionally contains no asset-format registry.

pub mod audio_clip_binary;
pub mod fxd_binary;
pub mod ydd_binary;
pub mod ysncd_binary;
pub mod ysncd_sound_graph;

pub use audio_clip_binary::{
    decode_audio_clip_binary_body, decode_audio_clip_nef8, encode_audio_clip_binary_body,
    AudioClipBinary, AudioClipLoopRegion, AUDIO_CLIP_BINARY_MAGIC,
    AUDIO_CLIP_BINARY_SCHEMA_VERSION, AUDIO_CLIP_ENCODING_PCM_F32_LE,
};
pub use fxd_binary::{decode_fxd_nef8, encode_fxd_nef8};
pub use ydd_binary::{
    encode_ydd_binary_body, YDD_BINARY_CONTRACT_SPEC, YDD_BINARY_ENCODING,
    YDD_BINARY_SCHEMA_VERSION,
};
pub use ysncd_binary::{
    decode_ysncd_binary_body, decode_ysncd_nef8, encode_ysncd_binary_body, YsncdAttenuation,
    YsncdClip, YsncdCue, YsncdCueDescriptor, YsncdDictionary, YSNCD_BINARY_MAGIC,
    YSNCD_BINARY_SCHEMA_VERSION,
};
pub use ysncd_sound_graph::{
    YsncdBlendPoint, YsncdLayerNodeRef, YsncdSoundGraph, YsncdSoundGraphNode,
    YsncdSoundGraphValueKind, YsncdWeightedNodeRef,
};
