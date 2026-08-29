#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_entity_api::EntityHandle;
use newengine_math::{EulerRot, Mat4, Quat, Vec3};

/// Local transform relative to parent.
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    #[inline]
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    /// Builds a transform from yaw/pitch/roll (radians) using engine-default conventions.
    #[inline]
    pub fn from_yaw_pitch_roll(
        position: Vec3,
        yaw: f32,
        pitch: f32,
        roll: f32,
        scale: Vec3,
    ) -> Self {
        let rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
        Self {
            position,
            rotation,
            scale,
        }
    }

    /// Returns (yaw, pitch, roll) in radians using engine-default conventions.
    #[inline]
    pub fn yaw_pitch_roll(self) -> (f32, f32, f32) {
        self.rotation.to_euler(EulerRot::YXZ)
    }

    #[inline]
    pub fn to_mat4(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }
}

/// Marks an entity as the canonical editor transform root for its runtime-generated subtree.
///
/// Picking may hit a render child, but editor selection is promoted to the nearest ancestor with
/// this marker so one gizmo edits the whole logical object.
#[derive(Clone, Copy, Debug, Default)]
pub struct TransformEditRoot;

/// Persistent manual transform calibration layered over a runtime-driven local pose.
///
/// Runtime systems keep ownership of the canonical base pose. Editor tools edit the resolved
/// `Transform`; this component captures the delta and reapplies it when the base pose changes on
/// later frames. Scale deliberately remains runtime-owned: this contract is for placement and
/// orientation calibration.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeTransformEditOverride {
    base: Transform,
    position_offset: Vec3,
    rotation_offset: Quat,
}

impl Default for RuntimeTransformEditOverride {
    fn default() -> Self {
        Self::new(Transform::default())
    }
}

impl RuntimeTransformEditOverride {
    #[inline]
    pub fn new(base: Transform) -> Self {
        Self {
            base: sanitize_edit_base(base),
            position_offset: Vec3::ZERO,
            rotation_offset: Quat::IDENTITY,
        }
    }

    #[inline]
    pub fn base(self) -> Transform {
        self.base
    }

    #[inline]
    pub fn position_offset(self) -> Vec3 {
        self.position_offset
    }

    #[inline]
    pub fn rotation_offset(self) -> Quat {
        self.rotation_offset
    }

    /// Captures a transform authored by editor tools relative to the latest runtime base pose.
    #[inline]
    pub fn capture_edited_transform(&mut self, edited: Transform) {
        if !edited.position.is_finite() || !edited.rotation.is_finite() {
            return;
        }
        let base_rotation = self.base.rotation.normalize_or_identity();
        self.position_offset = edited.position - self.base.position;
        self.rotation_offset = (base_rotation.inverse() * edited.rotation.normalize_or_identity())
            .normalize_or_identity();
    }

    /// Resolves the final local transform for the current runtime base and remembers that base for
    /// subsequent editor capture. Manual position/rotation offsets survive animation and procedural
    /// presentation updates without taking ownership of runtime scale.
    #[inline]
    pub fn resolve_from_base(&mut self, base: Transform) -> Transform {
        let base = sanitize_edit_base(base);
        self.base = base;
        Transform {
            position: base.position + self.position_offset,
            rotation: (base.rotation * self.rotation_offset).normalize_or_identity(),
            scale: base.scale,
        }
    }
}

#[inline]
fn sanitize_edit_base(mut base: Transform) -> Transform {
    if !base.position.is_finite() {
        base.position = Vec3::ZERO;
    }
    base.rotation = base.rotation.normalize_or_identity();
    if !base.scale.is_finite() {
        base.scale = Vec3::ONE;
    }
    base
}

/// World-space transform (derived).
#[derive(Clone, Copy, Debug)]
pub struct GlobalTransform(pub Mat4);

impl Default for GlobalTransform {
    #[inline]
    fn default() -> Self {
        Self(Mat4::IDENTITY)
    }
}

/// Parent link (tree). Pure DTO; ECS storage is owned by runtime crates.
#[derive(Clone, Copy, Debug)]
pub struct Parent(pub EntityHandle);

/// Children list (maintained by editor/gameplay code). Pure DTO.
#[derive(Clone, Debug, Default)]
pub struct Children(pub Vec<EntityHandle>);

/// Marks a node as needing recomputation (optional; currently used as a hint only).
#[derive(Clone, Copy, Debug, Default)]
pub struct TransformDirty;

/// Convenience world-space pose derived from `GlobalTransform`.
///
/// Generated by `propagate_transforms()` together with `GlobalTransform`.
#[derive(Clone, Copy, Debug, Default)]
pub struct WorldPose {
    pub world_pos: Vec3,
    /// Yaw around +Y (radians).
    pub yaw: f32,
    /// Pitch around +X (radians).
    pub pitch: f32,
    /// Roll around +Z (radians).
    pub roll: f32,
    pub world_scale: Vec3,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_transform_edit_override_tracks_moving_base_pose() {
        let base = Transform {
            position: Vec3::new(1.0, 2.0, 3.0),
            rotation: Quat::from_rotation_y(0.35),
            scale: Vec3::ONE,
        };
        let manual_rotation = Quat::from_rotation_x(0.20);
        let edited = Transform {
            position: base.position + Vec3::new(0.10, -0.04, 0.07),
            rotation: base.rotation * manual_rotation,
            scale: Vec3::splat(5.0),
        };
        let mut override_state = RuntimeTransformEditOverride::new(base);
        override_state.capture_edited_transform(edited);

        let next_base = Transform {
            position: Vec3::new(-2.0, 0.5, 4.0),
            rotation: Quat::from_rotation_y(-0.60),
            scale: Vec3::new(1.0, 2.0, 1.0),
        };
        let resolved = override_state.resolve_from_base(next_base);
        assert!(
            (resolved.position - (next_base.position + Vec3::new(0.10, -0.04, 0.07))).length()
                < 1.0e-6
        );
        let expected_rotation = (next_base.rotation * manual_rotation).normalize();
        assert!(resolved.rotation.dot(expected_rotation).abs() > 0.999_999);
        assert_eq!(resolved.scale, next_base.scale);
    }

    #[test]
    fn runtime_transform_edit_override_ignores_non_finite_editor_input() {
        let base = Transform::default();
        let mut override_state = RuntimeTransformEditOverride::new(base);
        override_state.capture_edited_transform(Transform {
            position: Vec3::new(f32::NAN, 0.0, 0.0),
            ..Transform::default()
        });
        let resolved = override_state.resolve_from_base(base);
        assert!(resolved.position.length_squared() < 1.0e-8);
        assert!(resolved.rotation.dot(Quat::IDENTITY).abs() > 0.999_999);
    }
}
