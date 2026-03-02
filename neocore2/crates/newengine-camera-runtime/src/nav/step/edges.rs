use newengine_camera::{CameraRig, EditorNavController, EditorNavMode};
use newengine_ecs::{EntityId, World};
use newengine_sim::FollowTargetCameraController;
use newengine_transform_api::Transform;

use crate::nav::helpers::{compute_projection, persist_camera_pose, retarget_follow_to_rig};
use crate::nav::input::cursor_state_for_nav;
use crate::nav::{BoundsSphere, CameraNavInput, CameraNavResult, CameraNavState};
use newengine_sim::CameraRigComp;

#[inline]
pub(crate) fn handle_capture_edge(
    state: &mut CameraNavState,
    world: &mut World,
    cam_id: EntityId,
    input: &mut CameraNavInput,
    aspect: f32,
    bounds: BoundsSphere,
    ctrl: &mut EditorNavController,
    rig: &mut CameraRig,
    follow_ctrl: Option<FollowTargetCameraController>,
) -> Option<CameraNavResult> {
    if input.fly_rmb == state.last_fly_rmb {
        return None;
    }

    let capture_begin = input.fly_rmb;
    state.last_fly_rmb = input.fly_rmb;

    input.clear_motion();

    let pre_local = world.get::<Transform>(cam_id).copied();

    if capture_begin {
        if ctrl.mode != EditorNavMode::Fly {
            ctrl.set_mode(EditorNavMode::Fly, rig);
        } else {
            ctrl.sync_fly_from_rig(rig);
        }
    } else {
        if ctrl.mode != EditorNavMode::Orbit {
            ctrl.set_mode(EditorNavMode::Orbit, rig);
        } else {
            ctrl.sync_orbit_from_rig(rig);
        }

        if let Some(follow) = follow_ctrl {
            let _ = retarget_follow_to_rig(world, cam_id, follow, rig);
        }
    }

    let _ = world.insert(cam_id, *ctrl);
    let _ = world.insert(cam_id, CameraRigComp(*rig));
    persist_camera_pose(world, cam_id, rig);

    let post_local = world.get::<Transform>(cam_id).copied();

    log::debug!(
        "camera_nav: capture {} cam={:?} mode={:?} rig_pos={:?} rig_rot={:?} orbit(yaw={:.5} pitch={:.5} dist={:.4} tgt={:?}) fly(yaw={:.5} pitch={:.5}) input(active={} look={} pan={} ui_busy={} move_mask=0x{:X}) local_pre={:?} local_post={:?}",
        if capture_begin { "BEGIN" } else { "END" },
        cam_id,
        ctrl.mode,
        rig.position,
        rig.rotation,
        ctrl.orbit.yaw,
        ctrl.orbit.pitch,
        ctrl.orbit.distance,
        ctrl.orbit.target,
        ctrl.fly.yaw,
        ctrl.fly.pitch,
        input.active,
        input.look_drag,
        input.pan_drag,
        input.ui_busy,
        input.move_mask,
        pre_local,
        post_local,
    );

    state.last_bounds_center = bounds.center;
    state.last_bounds_radius = bounds.radius;

    let projection = compute_projection(rig, bounds, aspect);
    let cursor = cursor_state_for_nav(input);

    Some(CameraNavResult {
        rig: *rig,
        controller: *ctrl,
        projection,
        cursor,
    })
}