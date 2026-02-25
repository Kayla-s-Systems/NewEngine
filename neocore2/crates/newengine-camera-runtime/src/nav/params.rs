use super::BoundsSphere;

#[derive(Clone, Copy, Debug)]
pub struct CameraNavParams {
    pub dt: f32,
    pub aspect: f32,

    pub bounds: BoundsSphere,
    pub selection_bounds: Option<BoundsSphere>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CameraNavFrameRequest {
    /// Monotonic sequence id (increments on each request).
    pub seq: u64,
    /// If true, frame the entire scene; otherwise frame selection first.
    pub all: bool,
}