#![forbid(unsafe_op_in_unsafe_fn)]

//! Skeleton metadata contracts used by model construction services.
//!
//! This crate is intentionally DTO-only. Format probes and fallback derivation
//! live in provider/runtime crates that consume NEF8 metadata dictionaries.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelSkeletonJointMetadata {
    /// Stable joint index in the source skeleton.
    #[serde(default)]
    pub index: u32,
    /// Source-format bone tag/hash when available (RAGE YFT exposes this directly).
    #[serde(default)]
    pub tag: u32,
    pub name: String,
    pub parent: Option<String>,
    #[serde(default)]
    pub parent_index: Option<u32>,
    /// Local-space bind-pose translation. Kept as `position_ls` for DTO compatibility.
    pub position_ls: [f32; 3],
    /// Local-space bind-pose quaternion `[x, y, z, w]`.
    #[serde(default = "identity_rotation")]
    pub rotation_ls: [f32; 4],
    /// Local-space bind-pose scale.
    #[serde(default = "identity_scale")]
    pub scale_ls: [f32; 3],
    /// Provider/source transform capabilities (RotX/TransX/etc.).
    #[serde(default)]
    pub flags: Vec<String>,
}

#[inline]
fn identity_rotation() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

#[inline]
fn identity_scale() -> [f32; 3] {
    [1.0, 1.0, 1.0]
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
    skeleton_joint_indexed(0, 0, name, parent, None, position_ls)
}

#[inline]
pub fn skeleton_joint_indexed(
    index: u32,
    tag: u32,
    name: impl Into<String>,
    parent: Option<impl Into<String>>,
    parent_index: Option<u32>,
    position_ls: [f32; 3],
) -> ModelSkeletonJointMetadata {
    ModelSkeletonJointMetadata {
        index,
        tag,
        name: name.into(),
        parent: parent.map(Into::into),
        parent_index,
        position_ls,
        rotation_ls: identity_rotation(),
        scale_ls: identity_scale(),
        flags: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_joint_json_defaults_new_bind_pose_fields() {
        let joint: ModelSkeletonJointMetadata =
            serde_json::from_str(r#"{"name":"hips","parent":"root","position_ls":[0.0,0.9,0.0]}"#)
                .expect("legacy skeleton joint JSON");
        assert_eq!(joint.index, 0);
        assert_eq!(joint.tag, 0);
        assert_eq!(joint.parent_index, None);
        assert_eq!(joint.rotation_ls, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(joint.scale_ls, [1.0, 1.0, 1.0]);
        assert!(joint.flags.is_empty());
    }
}
