use newengine_camera::{EditorNavController, EditorNavMode};
use newengine_ecs::{EntityId, World};
use newengine_sim::FollowTargetCameraController;

use super::commit::{commit_and_finish, finish_now};
use super::edges::handle_capture_edge;
use super::follow::try_step_follow_orbit;
use super::frame::maybe_frame_orbit;
use super::integrate::integrate_nav;
use super::resync::ensure_mode_without_impulse;
use super::tune::tune_controller;

use crate::nav::helpers::{compute_user_busy, ensure_camera_rig};
use crate::nav::input::cursor_state_for_nav;

use crate::nav::{
    BoundsSphere, CameraNavFrameRequest, CameraNavInput, CameraNavParams, CameraNavResult,
    CameraNavState,
};

/// Step camera navigation deterministically.
///
/// Contract:
/// - app is a thin adapter: feeds `CameraNavInput` and bounds, consumes `CameraNavResult`
/// - mode switches (Orbit/Fly) never teleport the rig
/// - cursor capture policy is derived from input (`active && fly_rmb`)
pub fn step_camera_nav(
    state: &mut CameraNavState,
    world: &mut World,
    cam_id: EntityId,
    input: &mut CameraNavInput,
    params: CameraNavParams,
    frame_req: CameraNavFrameRequest,
) -> CameraNavResult {
    let mut bounds = params.bounds;
    bounds.radius = bounds.radius.max(0.001);

    let desired_mode = if input.fly_rmb {
        EditorNavMode::Fly
    } else {
        EditorNavMode::Orbit
    };

    let mut ctrl = world
        .get::<EditorNavController>(cam_id)
        .cloned()
        .unwrap_or_default();

    let mut rig = ensure_camera_rig(world, cam_id);

    let follow_ctrl = world.get::<FollowTargetCameraController>(cam_id).copied();

    if let Some(res) = handle_capture_edge(
        state,
        world,
        cam_id,
        input,
        params.aspect,
        bounds,
        &mut ctrl,
        &mut rig,
        follow_ctrl,
    ) {
        return res;
    }

    let explicit_frame = frame_req.seq != state.last_frame_seq;
    if explicit_frame {
        state.last_frame_seq = frame_req.seq;
    }

    let user_busy = compute_user_busy(
        state.last_bounds_center,
        state.last_bounds_radius,
        input.look_drag,
        input.pan_drag,
        input.move_mask,
        input.ui_busy,
        bounds,
    );

    if ensure_mode_without_impulse(
        world,
        cam_id,
        input,
        params.aspect,
        bounds,
        desired_mode,
        &mut ctrl,
        &mut rig,
        state,
    ) {
        return finish_now(input, params.aspect, bounds, &ctrl, &rig);
    }

    if desired_mode == EditorNavMode::Orbit {
        if let Some(res) = try_step_follow_orbit(
            world,
            cam_id,
            params,
            bounds,
            &mut ctrl,
            &mut rig,
            follow_ctrl,
            state,
        ) {
            return res;
        }
    }

    tune_controller(&mut ctrl, desired_mode, bounds);

    integrate_nav(
        world,
        cam_id,
        input,
        params.dt,
        desired_mode,
        &mut ctrl,
        &mut rig,
        follow_ctrl,
    );

    if desired_mode == EditorNavMode::Orbit {
        maybe_frame_orbit(
            state,
            params,
            bounds,
            frame_req,
            explicit_frame,
            user_busy,
            &mut ctrl,
            &mut rig,
        );
    }

    commit_and_finish(world, cam_id, params.aspect, bounds, &ctrl, &rig, state, input)
}