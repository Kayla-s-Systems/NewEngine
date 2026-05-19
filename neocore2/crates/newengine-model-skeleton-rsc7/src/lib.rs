#![forbid(unsafe_op_in_unsafe_fn)]

//! Opaque RSC7/YMT skeleton probing.
//!
//! This does not decode proprietary skeleton payloads. It validates the RSC7
//! container, records metadata and derives humanoid anchors for runtime camera,
//! attachment and collision use.

use newengine_model_skeleton_api::{
    ModelSkeletonAnchors, ModelSkeletonJointMetadata, ModelSkeletonMetadata,
};

pub fn probe_rsc7_ymt_skeleton_metadata(
    source: &str,
    bytes: &[u8],
    target_height: f32,
    eye_height_ratio: f32,
) -> Result<ModelSkeletonMetadata, String> {
    if bytes.len() < 16 {
        return Err(format!("model skeleton ymt is too small source='{source}' bytes={}", bytes.len()));
    }

    let magic = std::str::from_utf8(&bytes[0..4]).unwrap_or("????").to_owned();
    if magic != "RSC7" {
        return Err(format!("unsupported model skeleton container source='{source}' magic='{magic}'"));
    }

    let target_height = target_height.clamp(0.25, 3.0);
    let eye_height = (target_height * eye_height_ratio.clamp(0.55, 0.98)).clamp(0.05, target_height);
    let hash = blake3::hash(bytes).to_hex().to_string();

    Ok(ModelSkeletonMetadata {
        source: source.to_owned(),
        source_format: "rockstar.ymt/rsc7".to_owned(),
        container_magic: magic,
        byte_len: bytes.len(),
        content_hash: format!("blake3:{hash}"),
        decode_status: "rsc7-container-detected; native skeleton payload kept opaque; humanoid anchors derived for runtime camera/attachment/collision use".to_owned(),
        joints: humanoid_anchor_skeleton(target_height, eye_height),
        anchors: ModelSkeletonAnchors {
            root: "root".to_owned(),
            hips: "pelvis".to_owned(),
            head: "head".to_owned(),
            left_hand: "hand_l".to_owned(),
            right_hand: "hand_r".to_owned(),
            left_foot: "foot_l".to_owned(),
            right_foot: "foot_r".to_owned(),
            eye: "eye_center".to_owned(),
            eye_height,
        },
    })
}

fn joint(name: &str, parent: Option<&str>, x: f32, y: f32, z: f32) -> ModelSkeletonJointMetadata {
    ModelSkeletonJointMetadata { name: name.to_owned(), parent: parent.map(str::to_owned), position_ls: [x, y, z] }
}

pub fn humanoid_anchor_skeleton(height: f32, eye_height: f32) -> Vec<ModelSkeletonJointMetadata> {
    let h = height.max(0.25);
    vec![
        joint("root", None, 0.0, 0.0, 0.0),
        joint("pelvis", Some("root"), 0.0, h * 0.52, 0.0),
        joint("spine_01", Some("pelvis"), 0.0, h * 0.62, 0.0),
        joint("spine_02", Some("spine_01"), 0.0, h * 0.72, 0.0),
        joint("neck", Some("spine_02"), 0.0, h * 0.84, 0.0),
        joint("head", Some("neck"), 0.0, h * 0.92, 0.0),
        joint("eye_center", Some("head"), 0.0, eye_height, 0.06),
        joint("clavicle_l", Some("spine_02"), -0.12 * h, h * 0.78, 0.0),
        joint("upperarm_l", Some("clavicle_l"), -0.22 * h, h * 0.74, 0.0),
        joint("lowerarm_l", Some("upperarm_l"), -0.32 * h, h * 0.62, 0.0),
        joint("hand_l", Some("lowerarm_l"), -0.38 * h, h * 0.52, 0.0),
        joint("clavicle_r", Some("spine_02"), 0.12 * h, h * 0.78, 0.0),
        joint("upperarm_r", Some("clavicle_r"), 0.22 * h, h * 0.74, 0.0),
        joint("lowerarm_r", Some("upperarm_r"), 0.32 * h, h * 0.62, 0.0),
        joint("hand_r", Some("lowerarm_r"), 0.38 * h, h * 0.52, 0.0),
        joint("thigh_l", Some("pelvis"), -0.09 * h, h * 0.42, 0.0),
        joint("calf_l", Some("thigh_l"), -0.09 * h, h * 0.22, 0.0),
        joint("foot_l", Some("calf_l"), -0.09 * h, h * 0.03, 0.10 * h),
        joint("thigh_r", Some("pelvis"), 0.09 * h, h * 0.42, 0.0),
        joint("calf_r", Some("thigh_r"), 0.09 * h, h * 0.22, 0.0),
        joint("foot_r", Some("calf_r"), 0.09 * h, h * 0.03, 0.10 * h),
    ]
}
