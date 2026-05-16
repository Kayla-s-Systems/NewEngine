use newengine_camera::{CameraChannel, CameraChannelState, CameraViewport};

use super::BoundsSphere;

#[derive(Clone, Copy, Debug)]
pub struct CameraNavParams {
    pub dt: f32,
    pub viewport: CameraViewport,
    pub channel: CameraChannelState,

    pub bounds: BoundsSphere,
    pub selection_bounds: Option<BoundsSphere>,
}

impl CameraNavParams {
    #[inline]
    pub fn aspect(&self) -> f32 {
        self.viewport.aspect()
    }
}

impl Default for CameraNavParams {
    #[inline]
    fn default() -> Self {
        Self {
            dt: 0.0,
            viewport: CameraViewport::default(),
            channel: CameraChannelState::dominant(CameraChannel::Runtime),
            bounds: BoundsSphere::default(),
            selection_bounds: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CameraNavFrameRequest {
    /// Monotonic sequence id (increments on each request).
    pub seq: u64,
    /// If true, frame the entire scene; otherwise frame selection first.
    pub all: bool,
}
