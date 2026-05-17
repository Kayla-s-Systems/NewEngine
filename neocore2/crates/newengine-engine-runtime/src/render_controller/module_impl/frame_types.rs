use newengine_core::render::RenderFrameDebugSnapshot;
use crate::camera_gateway::CameraGatewayFrame;
use newengine_ui::draw::UiDrawList;

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
    pub play_mode: GameRunMode,
}

pub(super) struct WorldFrameState {
    pub view_frame: CameraGatewayFrame,
}

pub(super) enum PlayableFrameOutcome {
    Continue {
        frame_debug_snapshot: Option<RenderFrameDebugSnapshot>,
    },
    EndedEarly,
}
