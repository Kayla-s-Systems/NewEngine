#[inline]
pub(super) fn gameplay_target_fov_y(
    active_view: CameraViewMode,
    first_person_aiming: bool,
    config: CameraRuntimeServiceConfig,
) -> f32 {
    match active_view {
        CameraViewMode::FirstPerson if first_person_aiming => config.first_person_ads_fov_y_radians,
        CameraViewMode::FirstPerson => config.first_person_fov_y_radians,
        CameraViewMode::ThirdPersonFollow => config.third_person_follow_fov_y_radians,
        CameraViewMode::ThirdPersonAim => config.third_person_aim_fov_y_radians,
        CameraViewMode::ThirdPersonOrbit => config.third_person_orbit_fov_y_radians,
    }
}

#[inline]
pub(super) fn gameplay_min_near(
    active_view: CameraViewMode,
    config: CameraRuntimeServiceConfig,
) -> f32 {
    if matches!(active_view, CameraViewMode::FirstPerson) {
        config.first_person_near
    } else {
        config.near_clip_third_person_min_distance
    }
}

#[inline]
pub(super) fn apply_gameplay_view_lens(
    frame: CameraFrame,
    active_view: CameraViewMode,
    first_person_aiming: bool,
    config: CameraRuntimeServiceConfig,
    target_near: f32,
) -> CameraFrame {
    let target_fov_y = gameplay_target_fov_y(active_view, first_person_aiming, config);
    let Projection::Perspective(mut perspective) = frame.projection else {
        return frame;
    };
    let target_near = if target_near.is_finite() && target_near > 0.0 {
        target_near
    } else {
        gameplay_min_near(active_view, config)
    };
    if (perspective.fovy - target_fov_y).abs() <= 1.0e-6
        && (perspective.near - target_near).abs() <= 1.0e-6
    {
        return frame;
    }
    perspective.fovy = target_fov_y;
    perspective.near = target_near;
    CameraFrame::build(
        frame.channel,
        frame.rig,
        Projection::Perspective(perspective),
        frame.viewport,
        frame.jitter_px,
    )
}

#[inline]
pub(super) fn view_postfx_from_camera_snapshot(
    snapshot: CameraFrameSnapshot,
) -> ViewPostFxFrameParams {
    let postfx = snapshot.postfx;
    ViewPostFxFrameParams {
        dof: ViewDepthOfFieldFrameParams {
            near_start: postfx.dof.near_start,
            near_end: postfx.dof.near_end,
            far_start: postfx.dof.far_start,
            far_end: postfx.dof.far_end,
            blend_level: postfx.dof.blend_level,
            high_quality: postfx.dof.high_quality,
        },
        motion_blur: ViewMotionBlurFrameParams {
            strength: postfx.motion_blur.strength,
            decay_rate: postfx.motion_blur.decay_rate,
        },
        shake_amplitude: postfx.shake_amplitude,
        exposure_bias: postfx.exposure_bias,
        jitter_px: postfx.jitter_px,
    }
}

#[inline]
pub(super) fn camera_report_snapshot(report: CameraRuntimeReport) -> CameraRuntimeOverlayReport {
    CameraRuntimeOverlayReport {
        active_director: format!("{:?}", report.active_director),
        active_mode: format!("{:?}", report.active_mode),
        active_view_mode: format!("{:?}", report.view_mode),
        target_entity: report.target_entity,
        transition: CameraTransitionOverlayReport {
            phase: match report.transition.phase {
                RuntimeCameraTransitionPhase::Idle => CameraTransitionPhase::Idle,
                RuntimeCameraTransitionPhase::Pending => CameraTransitionPhase::Pending,
                RuntimeCameraTransitionPhase::Blending => CameraTransitionPhase::Blending,
            },
            elapsed_sec: report.transition.elapsed_sec,
        },
        input_context: format!("{:?}", report.input_context),
        gate_blocked: report.gate_blocked,
        frame_blend_active: report.frame_blend_active,
        frame_blend_alpha: report.frame_blend_alpha,
        dominant_director: report.dominant_director.map(|it| format!("{:?}", it)),
        rendered_director_count: report.rendered_director_count,
        director_lock_input: report.director_lock_input,
        pending_event_count: report.pending_event_count,
    }
}

#[inline]
fn mat4_from_cols(cols: [[f32; 4]; 4]) -> Mat4 {
    Mat4::from_cols_array(&[
        cols[0][0], cols[0][1], cols[0][2], cols[0][3], cols[1][0], cols[1][1], cols[1][2],
        cols[1][3], cols[2][0], cols[2][1], cols[2][2], cols[2][3], cols[3][0], cols[3][1],
        cols[3][2], cols[3][3],
    ])
}

#[inline]
fn arr_vec3(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[1], v[2])
}
