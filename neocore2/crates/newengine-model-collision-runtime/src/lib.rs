#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime collision derivation for constructed models.

use newengine_model_domain_api::{ModelCollisionKind, ModelCollisionRef};
use newengine_model_skeleton_api::ModelSkeletonMetadata;

pub fn default_collisions_for_model(skeleton: Option<&ModelSkeletonMetadata>, target_height: f32) -> Vec<ModelCollisionRef> {
    let height = target_height.clamp(0.25, 3.0);
    let eye = skeleton.map(|it| it.anchors.eye_height).unwrap_or(height * 0.91);
    let half_height = (eye * 0.48).clamp(0.28, height * 0.48);
    let radius = (height * 0.18).clamp(0.14, 0.42);
    vec![ModelCollisionRef {
        name: "humanoid.body".to_owned(),
        kind: ModelCollisionKind::Capsule,
        anchor: Some("hips".to_owned()),
        radius,
        half_height,
        half_extents: [radius, half_height, radius],
        mesh: None,
    }]
}
