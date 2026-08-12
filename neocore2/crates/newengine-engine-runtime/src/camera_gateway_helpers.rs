use super::*;

impl EngineViewFrame {
    #[inline]
    pub(super) fn from_camera_snapshot(snapshot: CameraFrameSnapshot) -> Self {
        Self {
            view: mat4_from_cols(snapshot.view_cols),
            projection: mat4_from_cols(snapshot.projection_cols),
            view_projection: mat4_from_cols(snapshot.view_projection_cols),
            inverse_view: mat4_from_cols(snapshot.inverse_view_cols),
            position_ws: arr_vec3(snapshot.position_ws),
            position_ws_f64: snapshot.position_ws_f64,
            world_origin_ws_f64: snapshot.world_origin_ws_f64,
            position_origin_relative_ws: arr_vec3(snapshot.position_origin_relative_ws),
            forward_ws: arr_vec3(snapshot.forward_ws),
            viewport_width: snapshot.viewport.width,
            viewport_height: snapshot.viewport.height,
            aspect: snapshot.viewport.aspect,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CameraRuntimeOverlayReport {
    pub active_director: String,
    pub active_mode: String,
    pub active_view_mode: String,
    pub target_entity: Option<EntityId>,
    pub transition: CameraTransitionOverlayReport,
    pub input_context: String,
    pub gate_blocked: bool,
    pub frame_blend_active: bool,
    pub frame_blend_alpha: f32,
    pub dominant_director: Option<String>,
    pub rendered_director_count: usize,
    pub director_lock_input: bool,
    pub pending_event_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraTransitionPhase {
    Idle,
    Pending,
    Blending,
}

#[derive(Clone, Debug)]
pub struct CameraTransitionOverlayReport {
    pub phase: CameraTransitionPhase,
    pub elapsed_sec: f32,
}

#[inline]
pub fn apply_view_postfx(
    mut params: PostFxFrameParams,
    view: ViewPostFxFrameParams,
) -> PostFxFrameParams {
    params.display.exposure *= 2.0f32.powf(view.exposure_bias);
    params.view = view;
    params
}

#[inline]
pub(super) fn sanitize_camera_dt(dt: f32) -> f32 {
    if !dt.is_finite() || dt <= 0.0 {
        return 0.0;
    }
    // Camera navigation must not integrate a whole stall in one frame: render/asset
    // hitch recovery should not teleport the view or explode springs.
    dt.min(1.0 / 20.0)
}

#[inline]
fn finite_or_zero(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

#[inline]
fn finite_or_one(v: f32) -> f32 {
    if v.is_finite() && v > 0.0 {
        v
    } else {
        1.0
    }
}

#[inline]
pub(super) fn camera_runtime_service_config(
    world: &World,
    active_view: CameraViewMode,
) -> CameraRuntimeServiceConfig {
    let mut config = CameraRuntimeServiceConfig::default();
    if let Some(player) = first_player(world) {
        if let Some(body) = world.get::<CharacterBody>(player) {
            config.first_person_eye_height = world
                .get::<PlayerStanceState>(player)
                .map(|state| state.current_eye_height)
                .unwrap_or(body.standing_eye_height);
        }
        if let Some(motion) = world.get::<CharacterMotionTuning>(player) {
            config.sprint_multiplier = motion.sanitized().sprint_multiplier;
        }
    }
    config.runner = match active_view {
        CameraViewMode::FirstPerson => {
            newengine_camera_runtime::GameplayCameraRunnerKind::FirstPerson
        }
        CameraViewMode::ThirdPersonFollow => {
            newengine_camera_runtime::GameplayCameraRunnerKind::ThirdPersonFollow
        }
        CameraViewMode::ThirdPersonAim => {
            newengine_camera_runtime::GameplayCameraRunnerKind::ThirdPersonAim
        }
    };
    config
}

pub(super) fn apply_runtime_input(
    world: &mut World,
    input: CameraGatewayInput,
    effective_play_mode: GameRunMode,
    service_config: CameraRuntimeServiceConfig,
    frame_index: u64,
) {
    let Some(player) = first_player(world) else {
        return;
    };
    let controller_active = effective_play_mode.wants_direct_player_control()
        && is_player_controller_enabled(world, player);
    let gameplay_capture = crate::gameplay::gameplay_input_capture(world);
    let movement_blocked = input.gameplay_movement_gated || gameplay_capture.block_player_movement;
    let direct_control = controller_active && !movement_blocked;
    let command_actions = if controller_active {
        input.gameplay_actions
    } else {
        ActionCommandFrame::default()
    };
    apply_player_command_frame(world, player, frame_index, command_actions);

    if movement_blocked {
        CameraRuntimeService::clear_player_input(world, player);
    } else if direct_control {
        CameraRuntimeService::apply_player_input(
            world,
            player,
            input.move_mask,
            Vec2::new(-input.dx_px, -input.dy_px),
            input.active,
            service_config.sprint_multiplier,
        );
        emit_player_event(
            world,
            player,
            PlayerEventKind::InputApplied,
            "local input sampled",
        );
    } else {
        CameraRuntimeService::clear_player_input(world, player);
    }
}

#[inline]
pub(super) fn camera_nav_input(
    input: CameraGatewayInput,
    play_mode: GameRunMode,
) -> CameraNavInput {
    let mut nav_input = CameraNavInput {
        dx_px: finite_or_zero(input.dx_px).clamp(-240.0, 240.0),
        dy_px: finite_or_zero(input.dy_px).clamp(-240.0, 240.0),
        wheel_y: finite_or_zero(input.wheel_y).clamp(-12.0, 12.0),
        active: input.active,
        look_drag: input.look_drag,
        pan_drag: input.pan_drag,
        ui_busy: input.ui_busy,
        fly_rmb: input.fly_rmb,
        navigation_gated: input.camera_navigation_gated,
        move_mask: input.move_mask,
        speed_scalar: finite_or_one(input.speed_scalar).clamp(0.05, 20.0),
    };
    if play_mode.wants_direct_player_control() {
        nav_input.wheel_y = 0.0;
        nav_input.pan_drag = false;
    }
    if nav_input.navigation_gated {
        nav_input.gate_navigation();
    }
    nav_input
}

#[inline]
pub(super) fn apply_gameplay_view_lens(
    frame: CameraFrame,
    active_view: CameraViewMode,
) -> CameraFrame {
    let target_fov_y = match active_view {
        // 68 degrees vertical is approximately 100 degrees horizontal at 16:9.
        // It increases peripheral motion cues without becoming a distorted ultra-wide view.
        CameraViewMode::FirstPerson => 68.0_f32.to_radians(),
        CameraViewMode::ThirdPersonFollow => 64.0_f32.to_radians(),
        CameraViewMode::ThirdPersonAim => 54.0_f32.to_radians(),
    };
    let Projection::Perspective(mut perspective) = frame.projection else {
        return frame;
    };
    if (perspective.fovy - target_fov_y).abs() <= 1.0e-6 {
        return frame;
    }
    perspective.fovy = target_fov_y;
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
