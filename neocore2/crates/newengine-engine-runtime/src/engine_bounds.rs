#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::Vec3;

/// Engine-neutral spherical bounds snapshot used by gateway/bridge layers.
///
/// This type deliberately lives outside render-control and camera-runtime modules so
/// engine.camera, engine.render and future gateway bridges do not depend on
/// each other's implementation modules.
#[derive(Clone, Copy, Debug)]
pub struct EngineBoundsSnap {
    pub center: Vec3,
    pub radius: f32,
}

impl EngineBoundsSnap {
    #[inline]
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }
}
