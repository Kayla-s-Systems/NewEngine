#![forbid(unsafe_op_in_unsafe_fn)]

use glam::{Mat4, Quat, Vec3};
use newengine_ecs::{EntityId, World};

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
    #[inline]
    pub fn to_mat4(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }
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

/// Parent link (tree).
#[derive(Clone, Copy, Debug)]
pub struct Parent(pub EntityId);

/// Children list.
#[derive(Clone, Debug, Default)]
pub struct Children(pub Vec<EntityId>);

/// Propagates `Transform` + hierarchy into `GlobalTransform`.
///
/// Assumes:
/// - Entities without `Parent` are treated as roots.
/// - Cycles are not allowed; if present, results are undefined.
#[inline]
pub fn propagate_transforms(world: &mut World) {
    let mut missing: Vec<_> = world
        .iter_with::<Transform>()
        .filter_map(|(id, _)| {
            if world.get::<GlobalTransform>(id).is_none() {
                Some(id)
            } else {
                None
            }
        })
        .collect();

    for id in missing.drain(..) {
        let _ = world.insert(id, GlobalTransform::default());
    }
}
