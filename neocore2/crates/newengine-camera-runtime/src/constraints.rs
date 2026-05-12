#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::EntityId;
use newengine_math::{Quat, Vec3};

/// Declarative spring-arm constraint configuration for third-person cameras.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraSpringArmConfig {
    pub enabled: bool,
    pub probe_radius: f32,
    pub collision_padding: f32,
    pub min_distance: f32,
}

impl Default for CameraSpringArmConfig {
    #[inline]
    fn default() -> Self {
        Self {
            enabled: true,
            probe_radius: 0.18,
            collision_padding: 0.08,
            min_distance: 0.75,
        }
    }
}

impl CameraSpringArmConfig {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            enabled: self.enabled,
            probe_radius: sanitize_non_negative(self.probe_radius, 0.18).min(4.0),
            collision_padding: sanitize_non_negative(self.collision_padding, 0.08).min(2.0),
            min_distance: sanitize_non_negative(self.min_distance, 0.75).min(32.0),
        }
    }
}

/// Engine-neutral spherical collider proxy used by spring-arm constraints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraSpringArmCollider {
    pub entity: EntityId,
    pub center_ws: Vec3,
    pub radius: f32,
}

#[derive(Clone, Debug, Default)]
pub struct CameraSpringArmCollisionWorld {
    pub colliders: Vec<CameraSpringArmCollider>,
}

impl CameraSpringArmCollisionWorld {
    #[inline]
    pub fn clear(&mut self) {
        self.colliders.clear();
    }

    #[inline]
    pub fn push(&mut self, collider: CameraSpringArmCollider) {
        if collider.radius.is_finite() && collider.radius > 0.0 && collider.center_ws.is_finite() {
            self.colliders.push(collider);
        }
    }
}

/// Applies a conservative sphere-proxy spring-arm constraint to a desired local offset.
///
/// The runtime keeps the *authored* offset in profile/config and computes a constrained offset
/// per frame, so collision never permanently deforms the camera rig.
#[inline]
pub fn constrain_spring_arm_offset_ls(
    target_entity: EntityId,
    target_pos: Vec3,
    target_rot: Quat,
    desired_offset_ls: Vec3,
    config: CameraSpringArmConfig,
    collision_world: Option<&CameraSpringArmCollisionWorld>,
) -> Vec3 {
    let config = config.sanitized();
    if !config.enabled {
        return desired_offset_ls;
    }
    let Some(collision_world) = collision_world else {
        return desired_offset_ls;
    };

    let target_rot = target_rot.normalize_or_identity();
    let desired_ws = target_rot * desired_offset_ls;
    let desired_dist = desired_ws.length();
    if desired_dist <= config.min_distance.max(0.001) {
        return desired_offset_ls;
    }

    let dir_ws = desired_ws / desired_dist;
    let max_t = desired_dist;
    let mut constrained_t = max_t;
    let expanded_padding = config.probe_radius + config.collision_padding;

    for collider in collision_world.colliders.iter().copied() {
        if collider.entity == target_entity {
            continue;
        }
        let radius = collider.radius.max(0.001) + expanded_padding;
        let oc = target_pos - collider.center_ws;
        let b = oc.dot(dir_ws);
        let c = oc.dot(oc) - radius * radius;
        let discriminant = b * b - c;
        if discriminant < 0.0 {
            continue;
        }
        let hit_t = -b - discriminant.sqrt();
        if hit_t.is_finite() && hit_t > 0.0 && hit_t < constrained_t && hit_t <= max_t {
            constrained_t = hit_t;
        }
    }

    if constrained_t >= max_t {
        return desired_offset_ls;
    }

    let safe_t = (constrained_t - config.collision_padding).max(config.min_distance.min(max_t));
    let constrained_ws = dir_ws * safe_t;
    target_rot.inverse() * constrained_ws
}

#[inline]
fn sanitize_non_negative(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value >= 0.0 { value } else { fallback }
}
