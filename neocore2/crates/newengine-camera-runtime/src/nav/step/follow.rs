use newengine_camera::{CameraFrame, CameraRig, EditorNavController};
use newengine_ecs::{EntityId, World};
use newengine_sim::{step_follow_camera, FollowTargetCameraController, FollowTargetCameraMotor};
use newengine_transform_api::read_entity_world_pose_local_chain;

use crate::nav::helpers::{
    compute_projection, persist_camera_pose, retarget_follow_to_rig, target_has_transform,
};
use crate::nav::input::cursor_state_for_nav;
use crate::nav::{BoundsSphere, CameraNavInput, CameraNavParams, CameraNavResult, CameraNavState};
use newengine_sim::CameraRigComp;

#[inline]
pub(crate) fn try_step_follow_orbit(
    world: &mut World,
    cam_id: EntityId,
    input: &CameraNavInput,
    params: CameraNavParams,
    bounds: BoundsSphere,
    ctrl: &mut EditorNavController,
    rig: &mut CameraRig,
    follow_ctrl: Option<FollowTargetCameraController>,
    state: &mut CameraNavState,
) -> Option<CameraNavResult> {
    let Some(mut follow) = follow_ctrl else { return None };

    if !follow.follow_rotation {
        follow = retarget_follow_to_rig(world, cam_id, follow, rig);
    }

    if !target_has_transform(world, follow.target) {
        return None;
    }

    let Some((target_pos, target_rot)) = read_entity_world_pose_local_chain(world, follow.target) else {
        return None;
    };

    let motor = world
        .get::<FollowTargetCameraMotor>(cam_id)
        .copied()
        .unwrap_or_default();

    let Some(step) = step_follow_camera(
        rig.position,
        rig.rotation,
        target_pos,
        target_rot,
        follow.offset_ls,
        follow.rot_offset,
        follow.follow_rotation,
        motor.vel_ws,
        follow.smooth_time,
        follow.max_speed,
        params.dt,
    ) else {
        return None;
    };

    rig.position = step.next_pos;
    rig.rotation = step.next_rot;

    ctrl.sync_orbit_from_rig(rig);

    let _ = world.insert(cam_id, *ctrl);
    let _ = world.insert(cam_id, CameraRigComp(*rig));
    let _ = world.insert(
        cam_id,
        FollowTargetCameraMotor {
            vel_ws: step.next_vel,
        },
    );
    persist_camera_pose(world, cam_id, rig);

    state.last_bounds_center = bounds.center;
    state.last_bounds_radius = bounds.radius;

    let projection = compute_projection(rig, bounds, params.aspect());
    let frame = CameraFrame::build(params.channel, *rig, projection, params.viewport, newengine_math::Vec2::ZERO);
    let cursor = cursor_state_for_nav(input);

    Some(CameraNavResult { frame, controller: *ctrl, cursor })
}