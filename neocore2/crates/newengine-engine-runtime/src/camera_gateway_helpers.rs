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
pub fn apply_view_postfx(mut params: PostFxFrameParams, view: ViewPostFxFrameParams) -> PostFxFrameParams {
    params.display.exposure *= 2.0f32.powf(view.exposure_bias);
    params.view = view;
    params
}

#[inline]
pub(super) fn camera_runtime_service_config(world: &World, active_view: CameraViewMode) -> CameraRuntimeServiceConfig {
    let mut config = CameraRuntimeServiceConfig::default();
    if let Some(rules) = world.resource::<FpsDemoRules>() {
        config.first_person_eye_height = rules.player.camera_eye_height;
        config.sprint_multiplier = rules.player.sprint_multiplier;
    }
    config.runner = match active_view {
        CameraViewMode::FirstPerson => newengine_camera_runtime::GameplayCameraRunnerKind::FirstPerson,
        CameraViewMode::ThirdPersonFollow => newengine_camera_runtime::GameplayCameraRunnerKind::ThirdPersonFollow,
        CameraViewMode::ThirdPersonAim => newengine_camera_runtime::GameplayCameraRunnerKind::ThirdPersonAim,
    };
    config
}

pub(super) fn apply_runtime_input(
    world: &mut World,
    input: CameraGatewayInput,
    effective_play_mode: GameRunMode,
    service_config: CameraRuntimeServiceConfig,
) {
    let Some(player) = first_player(world) else {
        return;
    };
    if effective_play_mode.wants_direct_player_control() && is_player_controller_enabled(world, player) {
        CameraRuntimeService::apply_player_input(
            world,
            player,
            input.move_mask,
            Vec2::new(-input.dx_px, -input.dy_px),
            input.active,
            service_config.sprint_multiplier,
        );
        emit_player_event(world, player, PlayerEventKind::InputApplied, "local input sampled");
    } else {
        CameraRuntimeService::clear_player_input(world, player);
    }
}

#[inline]
pub(super) fn camera_nav_input(input: CameraGatewayInput, play_mode: GameRunMode) -> CameraNavInput {
    let mut nav_input = CameraNavInput {
        dx_px: input.dx_px,
        dy_px: input.dy_px,
        wheel_y: input.wheel_y,
        active: input.active,
        look_drag: input.look_drag,
        pan_drag: input.pan_drag,
        ui_busy: input.ui_busy,
        fly_rmb: input.fly_rmb,
        move_mask: input.move_mask,
        speed_scalar: input.speed_scalar,
    };
    if play_mode.wants_direct_player_control() {
        nav_input.wheel_y = 0.0;
        nav_input.pan_drag = false;
    }
    nav_input
}

#[inline]
pub(super) fn view_postfx_from_camera_snapshot(snapshot: CameraFrameSnapshot) -> ViewPostFxFrameParams {
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
        cols[0][0], cols[0][1], cols[0][2], cols[0][3],
        cols[1][0], cols[1][1], cols[1][2], cols[1][3],
        cols[2][0], cols[2][1], cols[2][2], cols[2][3],
        cols[3][0], cols[3][1], cols[3][2], cols[3][3],
    ])
}

#[inline]
fn arr_vec3(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[1], v[2])
}
