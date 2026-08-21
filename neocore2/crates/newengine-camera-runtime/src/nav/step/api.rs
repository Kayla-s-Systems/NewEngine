use newengine_camera::{RuntimeNavController, RuntimeNavMode};
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

use crate::nav::{
    CameraNavFrameRequest, CameraNavInput, CameraNavParams, CameraNavResult, CameraNavState,
};

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
        RuntimeNavMode::Fly
    } else {
        RuntimeNavMode::Orbit
    };

    let mut ctrl = world
        .get::<RuntimeNavController>(cam_id)
        .cloned()
        .unwrap_or_default();

    let mut rig = ensure_camera_rig(world, cam_id);

    let follow_ctrl = world.get::<FollowTargetCameraController>(cam_id).copied();

    // A gated possessed camera is owned by the gameplay camera service. Runtime navigation
    // must not retarget or integrate its FollowTargetCameraController in the same frame; doing
    // so creates two camera writers and visible third-person jitter.
    if input.navigation_gated && follow_ctrl.is_some() {
        return finish_now(input, params, bounds, &ctrl, &rig);
    }

    if let Some(res) = handle_capture_edge(
        state,
        world,
        cam_id,
        input,
        params,
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
        bounds,
        desired_mode,
        &mut ctrl,
        &mut rig,
        state,
    ) {
        return finish_now(input, params, bounds, &ctrl, &rig);
    }

    if desired_mode == RuntimeNavMode::Orbit {
        if let Some(res) = try_step_follow_orbit(
            world,
            cam_id,
            input,
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

    if desired_mode == RuntimeNavMode::Orbit {
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

    commit_and_finish(world, cam_id, params, bounds, &ctrl, &rig, state, input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_math::{Quat, Vec3};

    #[test]
    fn gated_possessed_camera_keeps_gameplay_follow_controller_owned() {
        let mut world = World::new();
        let player = world.spawn();
        let camera = world.spawn();
        let follow = FollowTargetCameraController {
            target: player,
            offset_ls: Vec3::new(0.0, 1.3, 4.8),
            rot_offset: Quat::IDENTITY,
            focus_offset_ls: Vec3::new(0.0, 0.95, 0.0),
            follow_rotation: false,
            render_cadence_only: true,
            smooth_time: 0.06,
            max_speed: 0.0,
        };
        let _ = world.insert(camera, follow);

        let mut state = CameraNavState::default();
        let mut input = CameraNavInput::default();
        input.navigation_gated = true;
        let params = CameraNavParams {
            dt: 1.0 / 60.0,
            viewport: newengine_camera::CameraViewport::from_size(1280, 720),
            channel: newengine_camera::CameraChannelState::dominant(
                newengine_camera::CameraChannel::Gameplay,
            ),
            bounds: crate::nav::BoundsSphere {
                center: Vec3::ZERO,
                radius: 10.0,
            },
            selection_bounds: None,
        };

        let _ = step_camera_nav(
            &mut state,
            &mut world,
            camera,
            &mut input,
            params,
            CameraNavFrameRequest::default(),
        );

        let after = world
            .get::<FollowTargetCameraController>(camera)
            .copied()
            .expect("gameplay follow controller must remain attached");
        assert!(!after.follow_rotation);
        assert_eq!(after.offset_ls, follow.offset_ls);
        assert_eq!(after.focus_offset_ls, follow.focus_offset_ls);
    }
}
