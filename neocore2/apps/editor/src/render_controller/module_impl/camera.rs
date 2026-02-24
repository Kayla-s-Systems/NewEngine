#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{orbit_frame_sphere, CameraInput};
use newengine_math::{Vec2, Vec3};

use newengine_sim::CameraRigComp;
use newengine_sim::{AngularVelocity, CameraInputComp, OrbitCameraMotor, Velocity};
use newengine_transform::Transform;

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
    pub rig: newengine_camera::CameraRig,
    pub ctrl: EditorCameraController,
}

#[inline]
fn build_camera_input(input: &ViewportInputSnap, ctrl_mode: EditorCameraMode, base_speed: f32) -> CameraInput {
    const MOVE_W: u64 = 1 << 0;
    const MOVE_A: u64 = 1 << 1;
    const MOVE_S: u64 = 1 << 2;
    const MOVE_D: u64 = 1 << 3;
    const MOVE_UP: u64 = 1 << 4;
    const MOVE_DOWN: u64 = 1 << 5;
    const MOVE_SHIFT: u64 = 1 << 6;

    let shift = (input.move_mask & MOVE_SHIFT) != 0;

    let fwd = ((input.move_mask & MOVE_W) != 0) as i32 - ((input.move_mask & MOVE_S) != 0) as i32;
    let right = ((input.move_mask & MOVE_D) != 0) as i32 - ((input.move_mask & MOVE_A) != 0) as i32;
    let up = ((input.move_mask & MOVE_UP) != 0) as i32 - ((input.move_mask & MOVE_DOWN) != 0) as i32;

    let mut move_axis = Vec3::ZERO;
    let speed_mul = if shift { 2.0 } else { 1.0 };

    if input.pan_drag && ctrl_mode == EditorCameraMode::Orbit {
        move_axis.x = -input.dx_px;
        move_axis.y = input.dy_px;
    }

    if ctrl_mode == EditorCameraMode::Fly {
        move_axis.x = right as f32;
        move_axis.y = up as f32;
        move_axis.z = fwd as f32;
    } else {
        move_axis.x += (right as f32) * base_speed;
        move_axis.z += (fwd as f32) * base_speed;
    }

    CameraInput {
        look_active: input.look_drag,
        look_delta: Vec2::new(-input.dx_px, -input.dy_px),
        move_axis,
        speed_mul,
        zoom_delta: input.wheel_y,
    }
}

pub(super) fn update_camera_and_persist(
    this: &mut EditorRenderController,
    scene: &mut newengine_scene::Scene,
    input: &mut ViewportInputSnap,
    params: CameraUpdateParams,
) -> CameraUpdateResult {
    let cam_id = scene.active_camera().unwrap_or_else(|| scene.root().unwrap_or_default());

    let world_mut = scene.world_mut();

    // Editor camera must be driven by the editor controller only.
    // If simulation motors/components remain on the active camera entity, they can overwrite
    // `CameraRigComp` (or integrate velocities) and cause visible snaps/drift when RMB toggles.
    // We aggressively strip those components for deterministic 1:1 pose preservation.
    let _ = world_mut.remove::<OrbitCameraMotor>(cam_id);
    let _ = world_mut.remove::<CameraInputComp>(cam_id);
    let _ = world_mut.remove::<Velocity>(cam_id);
    let _ = world_mut.remove::<AngularVelocity>(cam_id);

    let mut ctrl = world_mut
        .get::<EditorCameraController>(cam_id)
        .copied()
        .unwrap_or_default();

    let mut rig = world_mut
        .get::<CameraRigComp>(cam_id)
        .copied()
        .map(|c| c.0)
        .unwrap_or_default();

    // In the editor, the camera pose visible to the user is ultimately the entity `Transform`.
    // `CameraRigComp` can lag behind due to external edits or system ordering.
    // If we toggle RMB while the rig is stale, we overwrite the visible transform with an old pose,
    // which looks like a positional "kick" on press and a small drift on release.
    //
    // To guarantee 1:1 pose preservation across RMB transitions, always seed the rig
    // from the current `Transform` when it exists.
    if let Some(t) = world_mut.get::<Transform>(cam_id).copied() {
        rig.position = t.position;
        rig.rotation = t.rotation.normalize_or_identity();
    }

    let prev_mode = ctrl.mode;
    let desired_mode = if input.fly_rmb {
        EditorCameraMode::Fly
    } else {
        EditorCameraMode::Orbit
    };

    if prev_mode != desired_mode {
        input.clear_motion();

        match (prev_mode, desired_mode) {
            (EditorCameraMode::Fly, EditorCameraMode::Orbit) => {
                ctrl.sync_orbit_from_rig(&rig);
            }
            (EditorCameraMode::Orbit, EditorCameraMode::Fly) => {
                ctrl.sync_fly_from_rig(&rig);
            }
            _ => {}
        }
    }

    ctrl.mode = desired_mode;

    let cam_input = build_camera_input(input, ctrl.mode, params.base_speed);

    if ctrl.mode == EditorCameraMode::Orbit {
        ctrl.orbit.look_sens = 0.0045;
        ctrl.orbit.dolly_speed = (params.bounds.radius * 0.25).clamp(0.05, 10.0);
        ctrl.orbit.pan_speed = (params.bounds.radius * 0.0025).clamp(0.001, 1.0);

        EditorRenderController::enforce_orbit_basic(&mut ctrl.orbit);
        ctrl.apply(&mut rig, cam_input, params.dt);
        EditorRenderController::sync_rig_with_floor_lift(&mut ctrl.orbit, &mut rig);
    } else {
        ctrl.fly.look_sens = 0.0045;
        ctrl.fly.move_speed = (params.bounds.radius * 0.75).clamp(0.5, 200.0);
        ctrl.apply(&mut rig, cam_input, params.dt);
    }

    if params.explicit_frame || (!this.framed_once && !params.user_busy) {
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

        EditorRenderController::sync_rig_with_floor_lift(&mut ctrl.orbit, &mut rig);
    }

    let _ = world_mut.insert(cam_id, ctrl);
    let _ = world_mut.insert(cam_id, CameraRigComp(rig));

    // Persist pose immediately this frame. Do not rely solely on a later sim system to copy
    // `CameraRigComp` to `Transform`, since other systems may also touch `Transform`.
    if let Some(mut t) = world_mut.get::<Transform>(cam_id).copied() {
        t.position = rig.position;
        t.rotation = rig.rotation;
        let _ = world_mut.insert(cam_id, t);
    }

    CameraUpdateResult { rig, ctrl }
}