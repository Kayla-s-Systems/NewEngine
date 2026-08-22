use crate::scene_bridge::EngineViewGatewayFrame;
use newengine_core::render::RenderFrameDebugSnapshot;
use newengine_ui_api::UiRuntimeDebugOverlayTelemetry;
use newengine_ui_api::{UiDrawList, UiInputFrame};

use super::input::ViewportInputSnap;
use crate::gameplay::GameRunMode;

#[derive(Clone, Copy, Debug)]
pub(super) struct RenderFrameScope {
    pub w: u32,
    pub h: u32,
    pub vp_w: u32,
    pub vp_h: u32,
    pub direct_surface_viewport: bool,
    pub ui_enabled: bool,
    pub trace_frame: bool,
    pub dt: f32,
    pub fixed_dt: f32,
    pub fixed_alpha: f32,
    pub fixed_step_count: u32,
    pub fixed_tick: u64,
}

impl RenderFrameScope {
    #[inline]
    pub fn aspect(&self) -> f32 {
        (self.vp_w as f32 / self.vp_h as f32).max(1e-6)
    }
}

pub(super) struct ViewportFrameInput {
    pub ui: Option<UiDrawList>,
    pub input: ViewportInputSnap,
    pub surface_input: Option<UiInputFrame>,
    pub play_mode: GameRunMode,
}

pub(super) struct WorldFrameState {
    pub view_frame: EngineViewGatewayFrame,
}

pub(super) enum PlayableFrameOutcome {
    Continue {
        frame_debug_snapshot: Option<RenderFrameDebugSnapshot>,
    },
    EndedEarly {
        ui_telemetry: Option<UiRuntimeDebugOverlayTelemetry>,
    },
}
