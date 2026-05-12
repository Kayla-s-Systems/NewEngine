use newengine_camera::{CameraRig, EditorNavController, EditorNavMode};
use newengine_ecs::{EntityId, World};
use newengine_sim::CameraRigComp;

use crate::nav::helpers::persist_camera_pose;
use crate::nav::{BoundsSphere, CameraNavInput, CameraNavState};

#[inline]
pub(crate) fn ensure_mode_without_impulse(
    world: &mut World,
    cam_id: EntityId,
    input: &mut CameraNavInput,
    bounds: BoundsSphere,
    desired_mode: EditorNavMode,
    ctrl: &mut EditorNavController,
    rig: &mut CameraRig,
    state: &mut CameraNavState,
) -> bool {
    if ctrl.mode == desired_mode {
        return false;
    }

    input.clear_motion();
    ctrl.set_mode(desired_mode, rig);

    let _ = world.insert(cam_id, *ctrl);
    let _ = world.insert(cam_id, CameraRigComp(*rig));
    persist_camera_pose(world, cam_id, rig);

    state.last_bounds_center = bounds.center;
    state.last_bounds_radius = bounds.radius;

    true
}