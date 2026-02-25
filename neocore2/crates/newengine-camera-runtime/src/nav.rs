#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{
    auto_near_far_from_sphere, orbit_frame_sphere, CameraInput, CameraRig, EditorNavController,
    EditorNavMode, Perspective, Projection,
};
use newengine_core::host_events::CursorState;
use newengine_ecs::{EntityId, World};
use newengine_math::{Vec2, Vec3};
use newengine_sim::{
    follow_params_from_pose, step_follow_camera, CameraRigComp, FollowTargetCameraController,
    FollowTargetCameraMotor,
};
use newengine_transform::{read_entity_world_pose, write_entity_local_from_world_pose, Transform};
use newengine_viewport::input::{
    MOVE_A, MOVE_D, MOVE_DOWN, MOVE_S, MOVE_SHIFT, MOVE_UP, MOVE_W,
};

/// Minimal, renderer-agnostic viewport navigation input snapshot.
///
/// Produced by any UI/game layer and consumed by camera navigation systems.
#[derive(Clone, Copy, Debug, Default)]
pub struct CameraNavInput {
    pub dx_px: f32,
    pub dy_px: f32,
    pub wheel_y: f32,

    /// True when the viewport wants input (hovered, focused, or explicitly active).
    pub active: bool,

    /// True while look rotation is active.
    pub look_drag: bool,
    /// True while pan interaction is active.
    pub pan_drag: bool,
    /// True when UI is consuming input (gizmo, text input, etc.).
    pub ui_busy: bool,

    /// Latched free-fly intent (e.g. RMB capture).
    pub fly_rmb: bool,

    /// Movement key bitmask (`newengine_viewport::input::*`).
    pub move_mask: u64,
}

impl CameraNavInput {
    #[inline]
    pub fn clear_motion(&mut self) {
        self.dx_px = 0.0;
        self.dy_px = 0.0;
        self.wheel_y = 0.0;
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BoundsSphere {
    pub center: Vec3,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct CameraNavParams {
    pub dt: f32,
    pub aspect: f32,

    pub bounds: BoundsSphere,
    pub selection_bounds: Option<BoundsSphere>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CameraNavFrameRequest {
    /// Monotonic sequence id (increments on each request).
    pub seq: u64,
    /// If true, frame the entire scene; otherwise frame selection first.
    pub all: bool,
}

/// Persistent navigation state that must outlive a single frame.
///
/// This state is engine-side; apps store it as an opaque field.
#[derive(Clone, Copy, Debug)]
pub struct CameraNavState {
    framed_once: bool,
    framed_radius: f32,

    last_frame_seq: u64,

    last_bounds_center: Vec3,
    last_bounds_radius: f32,
}

impl Default for CameraNavState {
    #[inline]
    fn default() -> Self {
        Self {
            framed_once: false,
            framed_radius: 0.0,
            last_frame_seq: 0,
            last_bounds_center: Vec3::ZERO,
            last_bounds_radius: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CameraNavResult {
    pub rig: CameraRig,
    pub controller: EditorNavController,
    pub projection: Projection,
    pub cursor: CursorState,
}

#[inline]
pub fn cursor_state_for_nav(input: &CameraNavInput) -> CursorState {
    if input.active && input.fly_rmb {
        CursorState::captured_locked()
    } else {
        CursorState::released()
    }
}

#[inline]
fn build_camera_input(input: &CameraNavInput, mode: EditorNavMode) -> CameraInput {
    let shift = (input.move_mask & MOVE_SHIFT) != 0;

    let fwd = ((input.move_mask & MOVE_W) != 0) as i32 - ((input.move_mask & MOVE_S) != 0) as i32;
    let right =
        ((input.move_mask & MOVE_D) != 0) as i32 - ((input.move_mask & MOVE_A) != 0) as i32;
    let up = ((input.move_mask & MOVE_UP) != 0) as i32 - ((input.move_mask & MOVE_DOWN) != 0) as i32;

    let mut move_axis = Vec3::ZERO;
    let speed_mul = if shift { 2.0 } else { 1.0 };

    // Orbit pan uses screen-space pixels; the controller converts that to world-space.
    if input.pan_drag && mode == EditorNavMode::Orbit {
        move_axis.x = -input.dx_px;
        move_axis.y = input.dy_px;
    }

    if mode == EditorNavMode::Fly {
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
fn ensure_camera_rig(world: &mut World, cam_id: EntityId) -> CameraRig {
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
fn persist_camera_pose(world: &mut World, cam_id: EntityId, rig: &CameraRig) {
    write_entity_local_from_world_pose(world, cam_id, rig.position, rig.rotation);
}

#[inline]
fn retarget_follow_to_rig(
    world: &mut World,
    cam_id: EntityId,
    mut follow: FollowTargetCameraController,
    rig: &CameraRig,
) -> FollowTargetCameraController {
    if let Some((target_pos, target_rot)) = read_entity_world_pose(world, follow.target) {
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

#[inline]
fn compute_user_busy(state: &CameraNavState, input: &CameraNavInput, bounds: BoundsSphere) -> bool {
    let bounds_center_delta = (bounds.center - state.last_bounds_center).length();
    let bounds_radius_delta = (bounds.radius - state.last_bounds_radius).abs();
    let eps = bounds.radius.max(0.001) * 0.0005;
    let bounds_changed = bounds_center_delta > eps || bounds_radius_delta > eps;

    input.look_drag || input.pan_drag || input.move_mask != 0 || input.ui_busy || bounds_changed
}

#[inline]
fn compute_projection(rig: &CameraRig, bounds: BoundsSphere, aspect: f32) -> Projection {
    let fovy = 60.0f32.to_radians();
    let cam_dist = (rig.position - bounds.center).length().max(0.01);
    let (near, far) = auto_near_far_from_sphere(cam_dist, bounds.radius);
    Projection::Perspective(Perspective::new(fovy, aspect, near, far))
}

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

    let explicit_frame = frame_req.seq != state.last_frame_seq;
    if explicit_frame {
        state.last_frame_seq = frame_req.seq;
    }

    let user_busy = compute_user_busy(state, input, bounds);

    // Mode switch must not create impulses.
    if ctrl.mode != desired_mode {
        input.clear_motion();
        ctrl.set_mode(desired_mode, &rig);

        // If the camera has a follow controller, convert the current rig into follow params.
        // This prevents a snap-back when Fly capture ends.
        if desired_mode == EditorNavMode::Orbit {
            if let Some(follow) = follow_ctrl {
                let _ = retarget_follow_to_rig(world, cam_id, follow, &rig);
            }
        }

        let _ = world.insert(cam_id, ctrl);
        let _ = world.insert(cam_id, CameraRigComp(rig));
        persist_camera_pose(world, cam_id, &rig);

        // Update bounds tracking on every frame, even on early exits.
        state.last_bounds_center = bounds.center;
        state.last_bounds_radius = bounds.radius;

        let projection = compute_projection(&rig, bounds, params.aspect);
        let cursor = cursor_state_for_nav(input);
        return CameraNavResult {
            rig,
            controller: ctrl,
            projection,
            cursor,
        };
    }

    // Follow mode is an ECS-level composition primitive.
    // Policy: Follow owns camera pose unless Fly is active.
    if desired_mode == EditorNavMode::Orbit {
        if let Some(mut follow) = follow_ctrl {
            // Ensure follow controller is configured to preserve rotation.
            if !follow.follow_rotation {
                follow = retarget_follow_to_rig(world, cam_id, follow, &rig);
            }

            // Target must exist and have a transform.
            if world.get::<Transform>(follow.target).is_some() {
                if let Some((target_pos, target_rot)) = read_entity_world_pose(world, follow.target)
                {
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

                        state.last_bounds_center = bounds.center;
                        state.last_bounds_radius = bounds.radius;

                        let projection = compute_projection(&rig, bounds, params.aspect);
                        let cursor = cursor_state_for_nav(input);
                        return CameraNavResult {
                            rig,
                            controller: ctrl,
                            projection,
                            cursor,
                        };
                    }
                }
            }
            // Target missing/invalid: degrade to Orbit without killing the app.
        }
    }

    // Per-frame tuning.
    if desired_mode == EditorNavMode::Orbit {
        ctrl.orbit.look_sens = 0.0045;
        ctrl.orbit.dolly_speed = (bounds.radius * 0.25).clamp(0.05, 10.0);
        ctrl.orbit.pan_speed = (bounds.radius * 0.0025).clamp(0.001, 1.0);
    } else {
        ctrl.fly.look_sens = 0.0045;
        ctrl.fly_speed = (bounds.radius * 0.75).clamp(0.5, 200.0);
    }

    let cam_input = build_camera_input(input, desired_mode);
    ctrl.step(&mut rig, cam_input, params.dt);

    // When the camera is attached (FollowTargetCameraController), Fly navigation must not
    // detach the camera from the target. Convert the resulting pose into follow parameters.
    if desired_mode == EditorNavMode::Fly {
        if let Some(follow) = follow_ctrl {
            let _ = retarget_follow_to_rig(world, cam_id, follow, &rig);
        }
    }

    // Framing is Orbit-only.
    if desired_mode == EditorNavMode::Orbit {
        let grew = bounds.radius > state.framed_radius * 1.05;
        let do_frame = explicit_frame || (!state.framed_once && !user_busy) || (state.framed_once && !user_busy && grew);

        if do_frame {
            let (center, radius) = if explicit_frame && !frame_req.all {
                if let Some(sb) = params.selection_bounds {
                    (sb.center, sb.radius)
                } else {
                    (bounds.center, bounds.radius)
                }
            } else {
                (bounds.center, bounds.radius)
            };

            let fovy = 60.0f32.to_radians();
            orbit_frame_sphere(&mut ctrl.orbit, center, radius, fovy, params.aspect, 1.15);

            state.framed_radius = radius;
            state.framed_once = true;

            ctrl.rebuild_orbit_rig(&mut rig);
        }
    }

    let _ = world.insert(cam_id, ctrl);
    let _ = world.insert(cam_id, CameraRigComp(rig));
    persist_camera_pose(world, cam_id, &rig);

    state.last_bounds_center = bounds.center;
    state.last_bounds_radius = bounds.radius;

    let projection = compute_projection(&rig, bounds, params.aspect);
    let cursor = cursor_state_for_nav(input);

    CameraNavResult {
        rig,
        controller: ctrl,
        projection,
        cursor,
    }
}
