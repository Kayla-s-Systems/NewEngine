#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera_contracts::CameraFrameSnapshot;
use newengine_core::host_events::CursorState;
use newengine_core::render::{
    PostFxFrameParams, ViewDepthOfFieldFrameParams, ViewMotionBlurFrameParams,
    ViewPostFxFrameParams,
};
use newengine_ecs::World;
use newengine_input_actions_api::{ActionCommandFrame, CameraViewRequest};
use newengine_math::{Mat4, Vec3};

use crate::camera_gateway::{CameraGatewayFrame, CameraGatewayInput, CameraTransitionPhase};
use crate::engine_bounds::EngineBoundsSnap;
use crate::gameplay::GameRunMode;
use crate::viewport_bridge::ViewportBridge;

use super::SceneBridge;

/// Engine-neutral input consumed by the active `engine.camera` gateway.
///
/// Render code may assemble this from viewport/input state, but it must not know
/// whether the active camera backend is runtime-nav, cinematic, replay, scripted,
/// or a future provider plugin.
#[derive(Clone, Debug, Default)]
pub(crate) struct EngineViewInput {
    pub dx_px: f32,
    pub dy_px: f32,
    pub wheel_y: f32,
    pub active: bool,
    pub look_drag: bool,
    pub pan_drag: bool,
    pub ui_busy: bool,
    pub fly_rmb: bool,
    pub sampling_alive: bool,
    pub camera_navigation_gated: bool,
    pub gameplay_movement_gated: bool,
    pub move_mask: u64,
    pub speed_scalar: f32,
    pub camera_view: CameraViewRequest,
    pub gameplay_actions: ActionCommandFrame,
}

impl From<EngineViewInput> for CameraGatewayInput {
    #[inline]
    fn from(input: EngineViewInput) -> Self {
        Self {
            dx_px: input.dx_px,
            dy_px: input.dy_px,
            wheel_y: input.wheel_y,
            active: input.active,
            look_drag: input.look_drag,
            pan_drag: input.pan_drag,
            ui_busy: input.ui_busy,
            fly_rmb: input.fly_rmb,
            sampling_alive: input.sampling_alive,
            camera_navigation_gated: input.camera_navigation_gated,
            gameplay_movement_gated: input.gameplay_movement_gated,
            move_mask: input.move_mask,
            speed_scalar: input.speed_scalar,
            camera_view: input.camera_view,
            gameplay_actions: input.gameplay_actions,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) struct EngineViewGatewayFrame {
    pub frame_index: u64,
    pub camera_snapshot: CameraFrameSnapshot,
    pub view: EngineViewFrame,
    pub postfx: ViewPostFxFrameParams,
    pub diagnostics: Option<EngineViewDiagnostics>,
    pub cursor: CursorState,
    pub effective_play_mode: GameRunMode,
    pub world_playable: bool,
}

impl From<CameraGatewayFrame> for EngineViewGatewayFrame {
    #[inline]
    fn from(frame: CameraGatewayFrame) -> Self {
        Self {
            frame_index: frame.frame_index,
            camera_snapshot: frame.camera_snapshot,
            view: EngineViewFrame {
                view: frame.view.view,
                projection: frame.view.projection,
                view_projection: frame.view.view_projection,
                inverse_view: frame.view.inverse_view,
                position_ws: frame.view.position_ws,
                position_ws_f64: frame.view.position_ws_f64,
                world_origin_ws_f64: frame.view.world_origin_ws_f64,
                position_origin_relative_ws: frame.view.position_origin_relative_ws,
                forward_ws: frame.view.forward_ws,
                viewport_width: frame.view.viewport_width,
                viewport_height: frame.view.viewport_height,
                aspect: frame.view.aspect,
            },
            postfx: frame.postfx,
            diagnostics: frame.report.map(EngineViewDiagnostics::from),
            cursor: frame.cursor,
            effective_play_mode: frame.effective_play_mode,
            world_playable: frame.world_playable,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct EngineViewFrame {
    pub view: Mat4,
    pub projection: Mat4,
    pub view_projection: Mat4,
    pub inverse_view: Mat4,
    pub position_ws: Vec3,
    pub position_ws_f64: [f64; 3],
    pub world_origin_ws_f64: [f64; 3],
    pub position_origin_relative_ws: Vec3,
    pub forward_ws: Vec3,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub aspect: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct EngineViewDiagnostics {
    pub active_director: String,
    pub active_mode: String,
    pub active_view_mode: String,
    pub target_entity: Option<newengine_ecs::EntityId>,
    pub transition: EngineViewTransitionDiagnostics,
    pub input_context: String,
    pub gate_blocked: bool,
    pub frame_blend_active: bool,
    pub frame_blend_alpha: f32,
    pub dominant_director: Option<String>,
    pub rendered_director_count: usize,
    pub director_lock_input: bool,
    pub pending_event_count: usize,
}

impl From<crate::camera_gateway::CameraRuntimeOverlayReport> for EngineViewDiagnostics {
    #[inline]
    fn from(report: crate::camera_gateway::CameraRuntimeOverlayReport) -> Self {
        Self {
            active_director: report.active_director,
            active_mode: report.active_mode,
            active_view_mode: report.active_view_mode,
            target_entity: report.target_entity,
            transition: EngineViewTransitionDiagnostics::from(report.transition),
            input_context: report.input_context,
            gate_blocked: report.gate_blocked,
            frame_blend_active: report.frame_blend_active,
            frame_blend_alpha: report.frame_blend_alpha,
            dominant_director: report.dominant_director,
            rendered_director_count: report.rendered_director_count,
            director_lock_input: report.director_lock_input,
            pending_event_count: report.pending_event_count,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EngineViewTransitionPhase {
    Idle,
    Pending,
    Blending,
}

#[derive(Clone, Debug)]
pub(crate) struct EngineViewTransitionDiagnostics {
    pub phase: EngineViewTransitionPhase,
    pub elapsed_sec: f32,
}

impl From<crate::camera_gateway::CameraTransitionOverlayReport>
    for EngineViewTransitionDiagnostics
{
    #[inline]
    fn from(report: crate::camera_gateway::CameraTransitionOverlayReport) -> Self {
        Self {
            phase: match report.phase {
                CameraTransitionPhase::Idle => EngineViewTransitionPhase::Idle,
                CameraTransitionPhase::Pending => EngineViewTransitionPhase::Pending,
                CameraTransitionPhase::Blending => EngineViewTransitionPhase::Blending,
            },
            elapsed_sec: report.elapsed_sec,
        }
    }
}

#[inline]
pub(crate) fn apply_engine_view_postfx(
    mut params: PostFxFrameParams,
    view: ViewPostFxFrameParams,
) -> PostFxFrameParams {
    let launch_graphics = newengine_core::startup_launch_settings().graphics;
    params.display.exposure *= 2.0f32.powf(view.exposure_bias);
    params.view = ViewPostFxFrameParams {
        dof: if launch_graphics.depth_of_field_enabled {
            ViewDepthOfFieldFrameParams {
                near_start: view.dof.near_start,
                near_end: view.dof.near_end,
                far_start: view.dof.far_start,
                far_end: view.dof.far_end,
                blend_level: view.dof.blend_level,
                high_quality: view.dof.high_quality,
            }
        } else {
            ViewDepthOfFieldFrameParams::default()
        },
        motion_blur: if launch_graphics.motion_blur_enabled {
            ViewMotionBlurFrameParams {
                strength: view.motion_blur.strength,
                decay_rate: view.motion_blur.decay_rate,
            }
        } else {
            ViewMotionBlurFrameParams::default()
        },
        shake_amplitude: view.shake_amplitude,
        exposure_bias: view.exposure_bias,
        jitter_px: view.jitter_px,
    };
    params
}

impl SceneBridge {
    /// Apply direct-player input before simulation and consume camera-view requests.
    pub(crate) fn prepare_engine_runtime_input(
        &self,
        world: &mut World,
        input: EngineViewInput,
        effective_play_mode: GameRunMode,
        frame_index: u64,
    ) {
        self.camera_gateway.prepare_world_input(
            world,
            CameraGatewayInput::from(input),
            effective_play_mode,
            frame_index,
        );
    }

    /// Resolve the engine view for this frame through `engine.camera`.
    ///
    /// The render controller calls this neutral bridge and receives only a
    /// resolved view frame. It does not own or import camera director/nav state.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve_engine_view_frame(
        &self,
        world: &mut World,
        viewport: &ViewportBridge,
        input: EngineViewInput,
        play_mode: GameRunMode,
        effective_play_mode: GameRunMode,
        world_playable: bool,
        frame_index: u64,
        dt: f32,
        vp_w: u32,
        vp_h: u32,
        bounds: EngineBoundsSnap,
        selection_bounds: Option<EngineBoundsSnap>,
    ) -> EngineViewGatewayFrame {
        self.camera_gateway
            .tick_world_frame(
                world,
                viewport,
                CameraGatewayInput::from(input),
                play_mode,
                effective_play_mode,
                world_playable,
                frame_index,
                dt,
                vp_w,
                vp_h,
                bounds,
                selection_bounds,
            )
            .into()
    }
}
