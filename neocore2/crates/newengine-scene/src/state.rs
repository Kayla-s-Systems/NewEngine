#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::EntityId;

/// Strong scene invariants stored as a resource.
///
/// This prevents the "first found" ambiguity and makes roles explicit.
#[derive(Clone, Copy, Debug)]
pub struct SceneState {
    pub root: Option<EntityId>,
    pub active_camera: Option<EntityId>,
}

impl SceneState {
    #[inline]
    pub fn new(root: Option<EntityId>, active_camera: Option<EntityId>) -> Self {
        Self { root, active_camera }
    }
}
