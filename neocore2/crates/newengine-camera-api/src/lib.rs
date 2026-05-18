#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

/// Engine-facing camera service gateway id. Runtime consumers call this facade;
/// the host resolves it to the active camera provider by descriptor metadata.
pub const ENGINE_CAMERA_SERVICE_ID: &str = "engine.camera";

/// Default/first-party provider service id for camera runtime backends.
pub const CAMERA_SERVICE_ID: &str = "camera.api";
pub const CAMERA_BACKEND_CAPABILITY_ID: &str = "camera.backend";
pub const CAMERA_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const CAMERA_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const CAMERA_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const CAMERA_SERVICE_METHOD_SNAPSHOT_JSON_V1: &str = "snapshot_json_v1";

/// Generic backend-family declaration for camera providers.
pub const CAMERA_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "camera",
        ENGINE_CAMERA_SERVICE_ID,
        CAMERA_SERVICE_ID,
        CAMERA_BACKEND_CAPABILITY_ID,
    );

/// Startup validation contract for the engine-facing camera gateway.
pub const CAMERA_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_CAMERA_SERVICE_ID,
        "newengine.camera-api >= 0.1.x",
        newengine_service_api::JSON_CONTROL_SERVICE_METHODS_V1,
    );

/// Declarative startup requirement for camera. Missing camera degrades unless
/// the explicit env switch is enabled by a strict test/runtime profile.
pub const CAMERA_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        CAMERA_RUNTIME_CONTRACT_SPEC,
        Some(CAMERA_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_CAMERA_BACKEND"),
    );

pub type Mat4Cols = [[f32; 4]; 4];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraProjectionKind {
    Perspective,
    Orthographic,
    Custom,
}

impl Default for CameraProjectionKind {
    #[inline]
    fn default() -> Self {
        Self::Perspective
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraProjectionSnapshot {
    #[serde(default)]
    pub kind: CameraProjectionKind,
    #[serde(default)]
    pub fovy: f32,
    #[serde(default = "default_aspect")]
    pub aspect: f32,
    #[serde(default)]
    pub half_height: f32,
    #[serde(default = "default_near")]
    pub near: f32,
    #[serde(default = "default_far")]
    pub far: f32,
}

impl Default for CameraProjectionSnapshot {
    #[inline]
    fn default() -> Self {
        Self {
            kind: CameraProjectionKind::Perspective,
            fovy: 60.0f32.to_radians(),
            aspect: default_aspect(),
            half_height: 1.0,
            near: default_near(),
            far: default_far(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraViewportSnapshot {
    #[serde(default)]
    pub x: u32,
    #[serde(default)]
    pub y: u32,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default = "default_aspect")]
    pub aspect: f32,
}

impl Default for CameraViewportSnapshot {
    #[inline]
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: default_width(),
            height: default_height(),
            aspect: default_aspect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraPostFxDofIntent {
    #[serde(default)]
    pub near_start: f32,
    #[serde(default)]
    pub near_end: f32,
    #[serde(default = "default_far")]
    pub far_start: f32,
    #[serde(default = "default_far")]
    pub far_end: f32,
    #[serde(default)]
    pub blend_level: f32,
    #[serde(default)]
    pub high_quality: bool,
}

impl Default for CameraPostFxDofIntent {
    #[inline]
    fn default() -> Self {
        Self {
            near_start: 0.0,
            near_end: 0.0,
            far_start: default_far(),
            far_end: default_far(),
            blend_level: 0.0,
            high_quality: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraPostFxMotionBlurIntent {
    #[serde(default)]
    pub strength: f32,
    #[serde(default = "default_motion_blur_decay_rate")]
    pub decay_rate: f32,
}

impl Default for CameraPostFxMotionBlurIntent {
    #[inline]
    fn default() -> Self {
        Self { strength: 0.0, decay_rate: default_motion_blur_decay_rate() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraPostFxIntent {
    #[serde(default)]
    pub dof: CameraPostFxDofIntent,
    #[serde(default)]
    pub motion_blur: CameraPostFxMotionBlurIntent,
    #[serde(default)]
    pub shake_amplitude: f32,
    #[serde(default)]
    pub exposure_bias: f32,
    #[serde(default)]
    pub jitter_px: [f32; 2],
}

impl Default for CameraPostFxIntent {
    #[inline]
    fn default() -> Self {
        Self {
            dof: CameraPostFxDofIntent::default(),
            motion_blur: CameraPostFxMotionBlurIntent::default(),
            shake_amplitude: 0.0,
            exposure_bias: 0.0,
            jitter_px: [0.0, 0.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraFrameSnapshot {
    #[serde(default)]
    pub view_cols: Mat4Cols,
    #[serde(default)]
    pub projection_cols: Mat4Cols,
    #[serde(default)]
    pub view_projection_cols: Mat4Cols,
    #[serde(default)]
    pub inverse_view_cols: Mat4Cols,
    #[serde(default)]
    pub inverse_projection_cols: Mat4Cols,
    #[serde(default)]
    pub inverse_view_projection_cols: Mat4Cols,
    #[serde(default)]
    pub position_ws: [f32; 3],
    #[serde(default)]
    pub forward_ws: [f32; 3],
    #[serde(default)]
    pub right_ws: [f32; 3],
    #[serde(default)]
    pub up_ws: [f32; 3],
    #[serde(default)]
    pub viewport: CameraViewportSnapshot,
    #[serde(default)]
    pub projection: CameraProjectionSnapshot,
    #[serde(default)]
    pub jitter_px: [f32; 2],
    #[serde(default)]
    pub postfx: CameraPostFxIntent,
    #[serde(default = "default_true")]
    pub finite: bool,
}

impl Default for CameraFrameSnapshot {
    #[inline]
    fn default() -> Self {
        Self {
            view_cols: identity_cols(),
            projection_cols: identity_cols(),
            view_projection_cols: identity_cols(),
            inverse_view_cols: identity_cols(),
            inverse_projection_cols: identity_cols(),
            inverse_view_projection_cols: identity_cols(),
            position_ws: [0.0, 0.0, 0.0],
            forward_ws: [0.0, 0.0, -1.0],
            right_ws: [1.0, 0.0, 0.0],
            up_ws: [0.0, 1.0, 0.0],
            viewport: CameraViewportSnapshot::default(),
            projection: CameraProjectionSnapshot::default(),
            jitter_px: [0.0, 0.0],
            postfx: CameraPostFxIntent::default(),
            finite: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
}

impl Default for CameraServiceInfo {
    #[inline]
    fn default() -> Self {
        Self {
            protocol: "newengine.camera-api/v1".to_owned(),
            features: Vec::new(),
            methods: {
                let mut methods = newengine_service_api::JSON_CONTROL_SERVICE_METHODS_V1
                    .iter()
                    .map(|it| (*it).to_owned())
                    .collect::<Vec<_>>();
                methods.push(CAMERA_SERVICE_METHOD_SNAPSHOT_JSON_V1.to_owned());
                methods
            },
        }
    }
}

#[inline]
pub const fn identity_cols() -> Mat4Cols {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

#[inline]
fn default_true() -> bool { true }
#[inline]
fn default_width() -> u32 { 1920 }
#[inline]
fn default_height() -> u32 { 1080 }
#[inline]
fn default_aspect() -> f32 { 16.0 / 9.0 }
#[inline]
fn default_near() -> f32 { 0.01 }
#[inline]
fn default_far() -> f32 { 10_000.0 }
#[inline]
fn default_motion_blur_decay_rate() -> f32 { 0.5 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_frame_snapshot_accepts_minimal_payload() {
        let decoded: CameraFrameSnapshot = serde_json::from_str("{}").expect("defaults must decode");
        assert!(decoded.finite);
        assert_eq!(decoded.forward_ws, [0.0, 0.0, -1.0]);
    }

    #[test]
    fn camera_service_ids_are_engine_gateway_first() {
        assert_eq!(ENGINE_CAMERA_SERVICE_ID, "engine.camera");
        assert_eq!(CAMERA_BACKEND_SERVICE_SPEC.engine_gateway_id, ENGINE_CAMERA_SERVICE_ID);
        assert_eq!(CAMERA_BACKEND_SERVICE_SPEC.provider_service_id, CAMERA_SERVICE_ID);
    }
}
