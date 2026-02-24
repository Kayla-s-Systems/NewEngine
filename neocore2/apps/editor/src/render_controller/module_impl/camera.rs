#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{orbit_frame_sphere, CameraInput, CameraRig};
use newengine_math::{Vec2, Vec3};

use newengine_sim::{
    follow_params_from_pose, step_follow_camera, CameraRigComp, FollowTargetCameraController,
    FollowTargetCameraMotor,
};

use newengine_transform::{read_entity_world_pose, write_entity_local_from_world_pose, Transform};

use crate::editor_camera::{EditorCameraController, EditorCameraMode};

use super::input::ViewportInputSnap;
use super::scene::{BoundsSnap, SelectionBoundsSnap};
use super::EditorRenderController;

#[derive(Clone, Copy, Debug)]
pub(super) struct CameraUpdateParams {
    pub dt: f32,
    pub aspect: f32,
    pub bounds: BoundsSnap,
    pub sel_bounds: Option<SelectionBoundsSnap>,
    pub base_speed: f32,

    pub user_busy: bool,
    pub explicit_frame: bool,
    pub frame_all: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CameraUpdateResult {
    pub rig: CameraRig,
    pub ctrl: EditorCameraController,
}

#[inline]
fn build_camera_input(input: &ViewportInputSnap, ctrl_mode: EditorCameraMode) -> CameraInput {
    let shift = (input.move_mask & newengine_viewport::input::MOVE_SHIFT) != 0;

    let fwd = ((input.move_mask & newengine_viewport::input::MOVE_W) != 0) as i32
        - ((input.move_mask & newengine_viewport::input::MOVE_S) != 0) as i32;
    let right = ((input.move_mask & newengine_viewport::input::MOVE_D) != 0) as i32
        - ((input.move_mask & newengine_viewport::input::MOVE_A) != 0) as i32;
    let up = ((input.move_mask & newengine_viewport::input::MOVE_UP) != 0) as i32
        - ((input.move_mask & newengine_viewport::input::MOVE_DOWN) != 0) as i32;

    let mut move_axis = Vec3::ZERO;
    let speed_mul = if shift { 2.0 } else { 1.0 };

    // Orbit pan uses screen-space pixels; the orbit controller converts that to world-space.
    if input.pan_drag && ctrl_mode == EditorCameraMode::Orbit {
        move_axis.x = -input.dx_px;
        move_axis.y = input.dy_px;
    }

    if ctrl_mode == EditorCameraMode::Fly {
        move_axis.x = right as f32;
        move_axis.y = up as f32;
        move_axis.z = fwd as f32;
    }

    CameraInput {
        look_active: input.look_drag,
        look_delta: Vec2::new(-input.dx_px, -input.dy_px),
        move_axis,
        speed_mul,
        zoom_delta: input.wheel_y,
    }
}

#[inline]
fn ensure_camera_rig(world: &mut newengine_ecs::World, cam_id: newengine_ecs::EntityId) -> CameraRig {
    if let Some(r) = world.get::<CameraRigComp>(cam_id).copied() {
        return r.0;
    }

    let rig = read_entity_world_pose(world, cam_id)
        .map(|(pos, rot)| CameraRig {
            position: pos,
            rotation: rot,
        })
        .unwrap_or_default();

    let _ = world.insert(cam_id, CameraRigComp(rig));
    rig
}

#[inline]
fn persist_camera_pose(world: &mut newengine_ecs::World, cam_id: newengine_ecs::EntityId, rig: &CameraRig) {
    // Transform is derived output: keep hierarchy correct and preserve scale.
    write_entity_local_from_world_pose(world, cam_id, rig.position, rig.rotation);
}

#[inline]
fn retarget_follow_to_rig(
    world: &mut newengine_ecs::World,
    cam_id: newengine_ecs::EntityId,
    mut follow: FollowTargetCameraController,
    rig: &CameraRig,
) -> FollowTargetCameraController {
    if let Some((target_pos, target_rot)) = read_entity_world_pose(world, follow.target) {
        // Editor policy: preserve the authored camera orientation.
        follow.follow_rotation = true;

        let (offset_ls, rot_offset) =
            follow_params_from_pose(target_pos, target_rot, rig.position, rig.rotation);

        follow.offset_ls = offset_ls;
        follow.rot_offset = rot_offset;

        let _ = world.insert(cam_id, follow);
        let _ = world.insert(
            cam_id,
            FollowTargetCameraMotor {
                vel_ws: Vec3::ZERO,
            },
        );
    }
    follow
}

pub(super) fn update_camera_and_persist(
    this: &mut EditorRenderController,
    scene: &mut newengine_scene::Scene,
    input: &mut ViewportInputSnap,
    params: CameraUpdateParams,
) -> CameraUpdateResult {
    let cam_id = scene
        .active_camera()
        .unwrap_or_else(|| scene.root().unwrap_or_default());

    let world = scene.world_mut();

    let mut ctrl = world
        .get::<EditorCameraController>(cam_id)
        .cloned()
        .unwrap_or_default();

    let mut rig = ensure_camera_rig(world, cam_id);

    let follow_ctrl = world.get::<FollowTargetCameraController>(cam_id).copied();

    // Navigation mode (RMB capture => Fly, otherwise Orbit).
    let desired_mode = if input.fly_rmb {
        EditorCameraMode::Fly
    } else {
        EditorCameraMode::Orbit
    };

    // Mode switch must not create impulses.
    if ctrl.mode != desired_mode {
        input.clear_motion();
        ctrl.set_mode(desired_mode, &rig);

        // If the camera has a follow controller, convert the current rig into follow params.
        // This prevents a snap-back when RMB capture ends.
        if desired_mode == EditorCameraMode::Orbit {
            if let Some(follow) = follow_ctrl {
                let _ = retarget_follow_to_rig(world, cam_id, follow, &rig);
            }
        }

        let _ = world.insert(cam_id, ctrl);
        let _ = world.insert(cam_id, CameraRigComp(rig));
        persist_camera_pose(world, cam_id, &rig);
        return CameraUpdateResult { rig, ctrl };
    }

    // Follow mode is an ECS-level composition primitive.
    // Policy: Follow owns camera pose unless RMB Fly is active.
    if desired_mode == EditorCameraMode::Orbit {
        if let Some(mut follow) = follow_ctrl {
            // Ensure follow controller is configured to preserve rotation in the editor.
            if !follow.follow_rotation {
                follow = retarget_follow_to_rig(world, cam_id, follow, &rig);
            }

            // Target must exist and have a transform.
            if world.get::<Transform>(follow.target).is_some() {
                if let Some((target_pos, target_rot)) = read_entity_world_pose(world, follow.target) {
                    let motor = world
                        .get::<FollowTargetCameraMotor>(cam_id)
                        .copied()
                        .unwrap_or_default();

                    if let Some(step) = step_follow_camera(
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
                    ) {
                        rig.position = step.next_pos;
                        rig.rotation = step.next_rot;

                        // Keep orbit state consistent with the externally-authored rig.
                        ctrl.sync_orbit_from_rig(&rig);

                        let _ = world.insert(cam_id, ctrl);
                        let _ = world.insert(cam_id, CameraRigComp(rig));
                        let _ = world.insert(
                            cam_id,
                            FollowTargetCameraMotor {
                                vel_ws: step.next_vel,
                            },
                        );
                        persist_camera_pose(world, cam_id, &rig);
                        return CameraUpdateResult { rig, ctrl };
                    }
                }
            }
            // Target missing/invalid: degrade to Orbit without killing the editor.
        }
    }

    // Per-frame tuning (editor policy).
    if desired_mode == EditorCameraMode::Orbit {
        ctrl.orbit.look_sens = 0.0045;
        ctrl.orbit.dolly_speed = (params.bounds.radius * 0.25).clamp(0.05, 10.0);
        ctrl.orbit.pan_speed = (params.bounds.radius * 0.0025).clamp(0.001, 1.0);
    } else {
        ctrl.fly.look_sens = 0.0045;
        ctrl.fly_speed = (params.bounds.radius * 0.75).clamp(0.5, 200.0);
    }

    let cam_input = build_camera_input(input, desired_mode);
    ctrl.step(&mut rig, cam_input, params.dt);

    // When the camera is attached (FollowTargetCameraController), Fly navigation must not
    // detach the camera from the target. We author the rig freely, then immediately convert
    // the resulting pose into follow parameters.
    if desired_mode == EditorCameraMode::Fly {
        if let Some(follow) = follow_ctrl {
            let _ = retarget_follow_to_rig(world, cam_id, follow, &rig);
        }
    }

    // Framing is Orbit-only.
    if desired_mode == EditorCameraMode::Orbit {
        let do_frame = params.explicit_frame || (!this.framed_once && !params.user_busy);
        if do_frame {
            let (fc, fr) = if params.explicit_frame && !params.frame_all {
                if let Some(sb) = params.sel_bounds {
                    (sb.center, sb.radius)
                } else {
                    (params.bounds.center, params.bounds.radius)
                }
            } else {
                (params.bounds.center, params.bounds.radius)
            };

            let fovy = 60.0f32.to_radians();
            orbit_frame_sphere(&mut ctrl.orbit, fc, fr, fovy, params.aspect, 1.15);

            this.framed_radius = fr;
            this.framed_once = true;

            ctrl.rebuild_orbit_rig(&mut rig);
        }
    }

    let _ = world.insert(cam_id, ctrl);
    let _ = world.insert(cam_id, CameraRigComp(rig));
    persist_camera_pose(world, cam_id, &rig);

    CameraUpdateResult { rig, ctrl }
}
