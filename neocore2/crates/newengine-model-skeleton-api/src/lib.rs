#![forbid(unsafe_op_in_unsafe_fn)]

//! Skeleton metadata contracts used by model construction services.
//!
//! This crate is intentionally DTO-only. Format probes and fallback derivation
//! live in provider/runtime crates that consume NEF8 metadata dictionaries.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelSkeletonJointMetadata {
    pub name: String,
    pub parent: Option<String>,
    pub position_ls: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelSkeletonAnchors {
    pub root: String,
    pub hips: String,
    pub head: String,
    pub left_hand: String,
    pub right_hand: String,
    pub left_foot: String,
    pub right_foot: String,
    pub eye: String,
    pub eye_height: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelSkeletonMetadata {
    pub source: String,
    pub source_format: String,
    pub container_magic: String,
    pub byte_len: usize,
    pub content_hash: String,
    pub decode_status: String,
    pub joints: Vec<ModelSkeletonJointMetadata>,
    pub anchors: ModelSkeletonAnchors,
}

#[inline]
pub fn skeleton_joint(
    name: impl Into<String>,
    parent: Option<impl Into<String>>,
    position_ls: [f32; 3],
) -> ModelSkeletonJointMetadata {
    ModelSkeletonJointMetadata {
        name: name.into(),
        parent: parent.map(Into::into),
        position_ls,
    }
}
