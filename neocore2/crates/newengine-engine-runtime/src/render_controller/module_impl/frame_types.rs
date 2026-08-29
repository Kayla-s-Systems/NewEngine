use crate::scene_bridge::EngineViewGatewayFrame;
use newengine_core::render::RenderFrameDebugSnapshot;
use newengine_ui_api::UiRuntimeDebugOverlayTelemetry;
use newengine_ui_api::{UiInputFrame, UiLayerDomain, UiLayerDrawPacketSet};

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
    /// Complete ordered retained domain packet set carried to RenderFrameEnvelope.
    /// Primary-domain interaction mutates the packet directly; there is no singleton
    /// `UiDrawList` shadow copy in the frame lifecycle.
    pub ui_layers: UiLayerDrawPacketSet,
    pub primary_ui_domain: UiLayerDomain,
    pub input: ViewportInputSnap,
    pub surface_input: Option<UiInputFrame>,
    pub play_mode: GameRunMode,
}

pub(super) struct WorldFrameState {
    pub view_frame: EngineViewGatewayFrame,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RenderFrameError {
    MissingAuthoritativeCamera,
    InvalidAuthoritativeCamera { field: &'static str },
}

impl std::fmt::Display for RenderFrameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingAuthoritativeCamera => formatter.write_str(
                "render frame: MissingAuthoritativeCamera: active engine.camera route is absent or non-authoritative",
            ),
            Self::InvalidAuthoritativeCamera { field } => write!(
                formatter,
                "render frame: invalid authoritative camera field '{field}'",
            ),
        }
    }
}

impl std::error::Error for RenderFrameError {}

#[inline]
fn camera_matrix_is_finite(matrix: newengine_math::Mat4) -> bool {
    matrix
        .to_cols_array()
        .iter()
        .all(|component| component.is_finite())
}

impl WorldFrameState {
    pub(super) fn require_authoritative_camera(
        &self,
    ) -> Result<&EngineViewGatewayFrame, RenderFrameError> {
        if !crate::camera_gateway::camera_gateway_route_is_authoritative_in_current_host_context() {
            return Err(RenderFrameError::MissingAuthoritativeCamera);
        }

        let frame = &self.view_frame;
        let camera = frame.view;
        let invalid_field = if !frame.camera_snapshot.finite {
            Some("snapshot.finite")
        } else if !camera_matrix_is_finite(camera.view) {
            Some("view")
        } else if !camera_matrix_is_finite(camera.projection) {
            Some("projection")
        } else if !camera_matrix_is_finite(camera.view_projection) {
            Some("view_projection")
        } else if !camera_matrix_is_finite(camera.inverse_view) {
            Some("inverse_view")
        } else if !camera.position_ws.is_finite() {
            Some("position_ws")
        } else if !camera.forward_ws.is_finite() {
            Some("forward_ws")
        } else if !camera.position_origin_relative_ws.is_finite() {
            Some("position_origin_relative_ws")
        } else if !camera
            .position_ws_f64
            .iter()
            .all(|component| component.is_finite())
        {
            Some("position_ws_f64")
        } else if !camera
            .world_origin_ws_f64
            .iter()
            .all(|component| component.is_finite())
        {
            Some("world_origin_ws_f64")
        } else if !camera.aspect.is_finite() || camera.aspect <= 0.0 {
            Some("aspect")
        } else {
            None
        };

        if let Some(field) = invalid_field {
            return Err(RenderFrameError::InvalidAuthoritativeCamera { field });
        }

        debug_assert!(camera_matrix_is_finite(camera.view));
        debug_assert!(camera_matrix_is_finite(camera.projection));
        debug_assert!(camera.position_ws.is_finite());
        Ok(frame)
    }
}

pub(super) enum PlayableFrameOutcome {
    Continue {
        frame_debug_snapshot: Option<RenderFrameDebugSnapshot>,
    },
    EndedEarly {
        ui_telemetry: Option<UiRuntimeDebugOverlayTelemetry>,
    },
}

#[cfg(test)]
mod authoritative_camera_tests {
    use super::*;
    use crate::camera_gateway::{CameraGatewayBridge, CameraGatewayFrame, EngineViewFrame};
    use newengine_camera_contracts::CameraFrameSnapshot;
    use newengine_core::host_events::CursorState;
    use newengine_core::render::ViewPostFxFrameParams;
    use newengine_math::{Mat4, Vec3};

    fn finite_world_frame() -> WorldFrameState {
        WorldFrameState {
            view_frame: CameraGatewayFrame {
                frame_index: 7,
                camera_snapshot: CameraFrameSnapshot::default(),
                view: EngineViewFrame {
                    view: Mat4::IDENTITY,
                    projection: Mat4::IDENTITY,
                    view_projection: Mat4::IDENTITY,
                    inverse_view: Mat4::IDENTITY,
                    position_ws: Vec3::ZERO,
                    position_ws_f64: [0.0, 0.0, 0.0],
                    world_origin_ws_f64: [0.0, 0.0, 0.0],
                    position_origin_relative_ws: Vec3::ZERO,
                    forward_ws: Vec3::new(0.0, 0.0, -1.0),
                    viewport_width: 1920,
                    viewport_height: 1080,
                    aspect: 16.0 / 9.0,
                },
                postfx: ViewPostFxFrameParams::default(),
                report: None,
                cursor: CursorState::released(),
                effective_play_mode: GameRunMode::Play,
                world_playable: true,
            }
            .into(),
        }
    }

    #[test]
    fn playable_submit_rejects_missing_authoritative_camera_gateway() {
        let _host = newengine_plugin_host::create_host_context();

        let error = finite_world_frame()
            .require_authoritative_camera()
            .expect_err("playable submit must fail without engine.camera");

        assert_eq!(error, RenderFrameError::MissingAuthoritativeCamera);
    }

    #[test]
    fn playable_submit_rejects_non_finite_authoritative_camera() {
        let _host = newengine_plugin_host::create_host_context();
        let _camera = CameraGatewayBridge::new();
        let mut frame = finite_world_frame();
        frame.view_frame.view.position_ws = Vec3::new(f32::NAN, 0.0, 0.0);

        let error = frame
            .require_authoritative_camera()
            .expect_err("playable submit must reject non-finite camera data");

        assert_eq!(
            error,
            RenderFrameError::InvalidAuthoritativeCamera {
                field: "position_ws"
            }
        );
    }

    #[test]
    fn playable_submit_accepts_published_finite_authoritative_camera() {
        let _host = newengine_plugin_host::create_host_context();
        let _camera = CameraGatewayBridge::new();
        let frame = finite_world_frame();

        let camera = frame
            .require_authoritative_camera()
            .expect("published finite camera must be accepted");

        assert_eq!(camera.frame_index, 7);
    }
}
