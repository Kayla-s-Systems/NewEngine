use newengine_camera::{CameraRig, EditorNavController};
use newengine_ecs::{EntityId, World};
use newengine_sim::CameraRigComp;

use crate::nav::helpers::{compute_projection, persist_camera_pose};
use crate::nav::input::cursor_state_for_nav;
use crate::nav::{BoundsSphere, CameraNavInput, CameraNavResult, CameraNavState};

#[inline]
pub(crate) fn commit_and_finish(
    world: &mut World,
    cam_id: EntityId,
    aspect: f32,
    bounds: BoundsSphere,
    ctrl: &EditorNavController,
    rig: &CameraRig,
    state: &mut CameraNavState,
    input: &CameraNavInput,
) -> CameraNavResult {
    let _ = world.insert(cam_id, *ctrl);
    let _ = world.insert(cam_id, CameraRigComp(*rig));
    persist_camera_pose(world, cam_id, rig);

    state.last_bounds_center = bounds.center;
    state.last_bounds_radius = bounds.radius;

    finish_now(input, aspect, bounds, ctrl, rig)
}

#[inline]
pub(crate) fn finish_now(
    input: &CameraNavInput,
    aspect: f32,
    bounds: BoundsSphere,
    ctrl: &EditorNavController,
    rig: &CameraRig,
) -> CameraNavResult {
    let projection = compute_projection(rig, bounds, aspect);
    let cursor = cursor_state_for_nav(input);

    CameraNavResult {
        rig: *rig,
        controller: *ctrl,
        projection,
        cursor,
    }
}