#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{CameraFrame, CameraPostEffects, CameraWorldFrame, Projection};
use newengine_camera_api::{
    CameraFrameSnapshot, CameraPostFxDofIntent, CameraPostFxIntent, CameraPostFxMotionBlurIntent,
    CameraProjectionKind, CameraProjectionSnapshot, CameraViewMode, CameraViewportSnapshot,
    Mat4Cols,
};
use newengine_math::{Mat4, Vec3};

#[inline]
pub fn camera_frame_snapshot(
    frame: CameraFrame,
    effects: CameraPostEffects,
) -> CameraFrameSnapshot {
    camera_frame_snapshot_for_view(frame, effects, CameraViewMode::FirstPerson)
}

#[inline]
pub fn camera_world_frame_snapshot(
    frame: CameraWorldFrame,
    effects: CameraPostEffects,
    view_mode: CameraViewMode,
) -> CameraFrameSnapshot {
    let mut snapshot = camera_frame_snapshot_for_view(frame.frame, effects, view_mode);
    snapshot.position_ws_f64 = [frame.camera_ws.x, frame.camera_ws.y, frame.camera_ws.z];
    snapshot.world_origin_ws_f64 = [
        frame.origin.origin.x,
        frame.origin.origin.y,
        frame.origin.origin.z,
    ];
    snapshot.position_origin_relative_ws = vec3_arr(frame.frame.rig.position);
    snapshot
}

pub fn camera_frame_snapshot_for_view(
    frame: CameraFrame,
    effects: CameraPostEffects,
    view_mode: CameraViewMode,
) -> CameraFrameSnapshot {
    let rig = frame.rig;
    let projection = projection_snapshot(frame.projection);
    let viewport = frame.viewport.sanitized();
    let effects = effects.sanitized();
    CameraFrameSnapshot {
        view_mode,
        view_cols: mat4_cols(frame.matrices.view),
        projection_cols: mat4_cols(frame.matrices.proj),
        view_projection_cols: mat4_cols(frame.matrices.view_proj),
        inverse_view_cols: mat4_cols(frame.matrices.inv_view),
        inverse_projection_cols: mat4_cols(frame.matrices.inv_proj),
        inverse_view_projection_cols: mat4_cols(frame.matrices.inv_view_proj),
        position_ws: vec3_arr(rig.position),
        position_ws_f64: vec3_arr_f64(rig.position),
        world_origin_ws_f64: [0.0, 0.0, 0.0],
        position_origin_relative_ws: vec3_arr(rig.position),
        forward_ws: vec3_arr(rig.forward()),
        right_ws: vec3_arr(rig.right()),
        up_ws: vec3_arr(rig.up()),
        viewport: CameraViewportSnapshot {
            x: viewport.x,
            y: viewport.y,
            width: viewport.width,
            height: viewport.height,
            aspect: viewport.aspect(),
        },
        projection,
        jitter_px: [frame.jitter_px.x, frame.jitter_px.y],
        postfx: CameraPostFxIntent {
            dof: CameraPostFxDofIntent {
                near_start: effects.dof.near_start,
                near_end: effects.dof.near_end,
                far_start: effects.dof.far_start,
                far_end: effects.dof.far_end,
                blend_level: effects.dof.blend_level,
                high_quality: effects.high_quality_dof,
            },
            motion_blur: CameraPostFxMotionBlurIntent {
                strength: effects.motion_blur.strength,
                decay_rate: effects.motion_blur.decay_rate,
            },
            shake_amplitude: effects.shake_amplitude,
            exposure_bias: effects.exposure_bias,
            jitter_px: [effects.jitter_px.x, effects.jitter_px.y],
        },
        finite: frame.diagnostics.finite,
    }
}

#[inline]
fn projection_snapshot(projection: Projection) -> CameraProjectionSnapshot {
    match projection {
        Projection::Perspective(p) => CameraProjectionSnapshot {
            kind: CameraProjectionKind::Perspective,
            fovy: p.fovy,
            aspect: p.aspect,
            half_height: 0.0,
            near: p.near,
            far: p.far,
        },
        Projection::Orthographic(o) => CameraProjectionSnapshot {
            kind: CameraProjectionKind::Orthographic,
            fovy: 0.0,
            aspect: o.aspect,
            half_height: o.half_height,
            near: o.near,
            far: o.far,
        },
    }
}

#[inline]
fn mat4_cols(m: Mat4) -> Mat4Cols {
    m.to_cols_array_2d()
}

#[inline]
fn vec3_arr(v: Vec3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

#[inline]
fn vec3_arr_f64(v: Vec3) -> [f64; 3] {
    [v.x as f64, v.y as f64, v.z as f64]
}
