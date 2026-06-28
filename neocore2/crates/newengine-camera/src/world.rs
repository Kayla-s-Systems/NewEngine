#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Quat, Vec3};

use crate::{CameraChannelState, CameraFrame, CameraRig, CameraViewport, Projection};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Double-precision world position used by camera/domain code before lowering to
/// renderer-local `f32` coordinates.
///
/// The render path should receive positions relative to a nearby origin. This keeps
/// matrices and shader inputs stable when authored coordinates drift away
/// from `(0, 0, 0)`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraWorldPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl CameraWorldPoint {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    #[inline]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub const fn from_array(v: [f64; 3]) -> Self {
        Self::new(v[0], v[1], v[2])
    }

    #[inline]
    pub const fn to_array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }

    #[inline]
    pub fn translated(self, delta: Vec3) -> Self {
        if delta.is_finite() {
            Self::new(
                self.x + delta.x as f64,
                self.y + delta.y as f64,
                self.z + delta.z as f64,
            )
        } else {
            self
        }
    }

    #[inline]
    pub fn from_vec3(v: Vec3) -> Self {
        Self::new(v.x as f64, v.y as f64, v.z as f64)
    }

    /// Lossy conversion intended only for small/local coordinates.
    #[inline]
    pub fn to_vec3_lossy(self) -> Vec3 {
        Vec3::new(self.x as f32, self.y as f32, self.z as f32)
    }

    #[inline]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    #[inline]
    pub fn relative_to(self, origin: CameraWorldPoint) -> Vec3 {
        Vec3::new(
            (self.x - origin.x) as f32,
            (self.y - origin.y) as f32,
            (self.z - origin.z) as f32,
        )
    }

    #[inline]
    pub fn distance(self, rhs: Self) -> f64 {
        self.distance_squared(rhs).sqrt()
    }

    #[inline]
    pub fn distance_squared(self, rhs: Self) -> f64 {
        let dx = self.x - rhs.x;
        let dy = self.y - rhs.y;
        let dz = self.z - rhs.z;
        dx * dx + dy * dy + dz * dz
    }
}

/// Quantized camera-local origin for precision-safe rendering.
///
/// The origin is intentionally explicit data, not a hidden singleton. Streaming,
/// render-prep and debug tooling can inspect it and decide which packets need to be
/// rebuilt after an origin change.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraWorldOrigin {
    pub origin: CameraWorldPoint,
    /// Quantization grid in world units. A 1024-unit cell keeps camera-relative coordinates small while avoiding constant rebases.
    pub cell_size: f64,
}

impl Default for CameraWorldOrigin {
    #[inline]
    fn default() -> Self {
        Self::new(CameraWorldPoint::ZERO, 1024.0)
    }
}

impl CameraWorldOrigin {
    #[inline]
    pub const fn new(origin: CameraWorldPoint, cell_size: f64) -> Self {
        Self { origin, cell_size }
    }

    #[inline]
    pub fn for_camera(camera_position: CameraWorldPoint, cell_size: f64) -> Self {
        let cell = sanitize_cell_size(cell_size);
        Self::new(quantize_point(camera_position, cell), cell)
    }

    #[inline]
    pub fn relative_point(self, point: CameraWorldPoint) -> Vec3 {
        point.relative_to(self.origin)
    }

    #[inline]
    pub fn camera_relative(self, camera_position: CameraWorldPoint) -> Vec3 {
        self.relative_point(camera_position)
    }

    #[inline]
    pub fn local_to_world(self, local: Vec3) -> CameraWorldPoint {
        self.origin.translated(local)
    }

    #[inline]
    pub fn should_rebase(self, camera_position: CameraWorldPoint) -> bool {
        let half = sanitize_cell_size(self.cell_size) * 0.5;
        let dx = (camera_position.x - self.origin.x).abs();
        let dy = (camera_position.y - self.origin.y).abs();
        let dz = (camera_position.z - self.origin.z).abs();
        dx > half || dy > half || dz > half
    }

    #[inline]
    pub fn rebased_for_camera(self, camera_position: CameraWorldPoint) -> Self {
        if self.should_rebase(camera_position) {
            Self::for_camera(camera_position, self.cell_size)
        } else {
            self
        }
    }
}

/// Double-precision camera pose plus explicit render origin.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraWorldRig {
    pub position: CameraWorldPoint,
    pub rotation: Quat,
    pub origin: CameraWorldOrigin,
}

impl Default for CameraWorldRig {
    #[inline]
    fn default() -> Self {
        Self::new(CameraWorldPoint::ZERO, Quat::IDENTITY)
    }
}

impl CameraWorldRig {
    #[inline]
    pub fn new(position: CameraWorldPoint, rotation: Quat) -> Self {
        let origin = CameraWorldOrigin::for_camera(position, 1024.0);
        Self {
            position,
            rotation: rotation.normalize_or_identity(),
            origin,
        }
    }

    #[inline]
    pub fn with_origin(
        position: CameraWorldPoint,
        rotation: Quat,
        origin: CameraWorldOrigin,
    ) -> Self {
        let origin = origin.rebased_for_camera(position);
        Self {
            position,
            rotation: rotation.normalize_or_identity(),
            origin,
        }
    }

    #[inline]
    pub fn with_cell_size(mut self, cell_size: f64) -> Self {
        self.origin = CameraWorldOrigin::for_camera(self.position, cell_size);
        self
    }

    #[inline]
    pub fn rebase_if_needed(&mut self) {
        self.origin = self.origin.rebased_for_camera(self.position);
    }

    #[inline]
    pub fn local_position(self) -> Vec3 {
        self.origin.camera_relative(self.position)
    }

    #[inline]
    pub fn to_local_rig(self) -> CameraRig {
        CameraRig::new(self.local_position(), self.rotation)
    }
}

/// Fully resolved world-space camera frame.
///
/// `frame` is renderer-ready and uses camera-origin-relative `f32` coordinates.
/// `camera_ws` and `origin` preserve precise world-space authority for streaming, diagnostics, culling and render packets.
#[derive(Clone, Copy, Debug)]
pub struct CameraWorldFrame {
    pub frame: CameraFrame,
    pub camera_ws: CameraWorldPoint,
    pub origin: CameraWorldOrigin,
}

impl CameraWorldFrame {
    #[inline]
    pub fn build(
        channel: CameraChannelState,
        mut rig: CameraWorldRig,
        projection: Projection,
        viewport: CameraViewport,
        jitter_px: newengine_math::Vec2,
    ) -> Self {
        rig.rebase_if_needed();
        let frame =
            CameraFrame::build(channel, rig.to_local_rig(), projection, viewport, jitter_px);
        Self {
            frame,
            camera_ws: rig.position,
            origin: rig.origin,
        }
    }

    #[inline]
    pub fn relative_point(&self, point: CameraWorldPoint) -> Vec3 {
        self.origin.relative_point(point)
    }
}

#[inline]
fn sanitize_cell_size(cell_size: f64) -> f64 {
    if cell_size.is_finite() && cell_size >= 1.0 {
        cell_size
    } else {
        1024.0
    }
}

#[inline]
fn quantize_axis(v: f64, cell: f64) -> f64 {
    (v / cell).floor() * cell
}

#[inline]
fn quantize_point(p: CameraWorldPoint, cell: f64) -> CameraWorldPoint {
    if !p.is_finite() {
        return CameraWorldPoint::ZERO;
    }
    CameraWorldPoint::new(
        quantize_axis(p.x, cell),
        quantize_axis(p.y, cell),
        quantize_axis(p.z, cell),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_keeps_camera_local_position_small() {
        let p = CameraWorldPoint::new(1_000_123.0, 42.0, -2_000_900.0);
        let origin = CameraWorldOrigin::for_camera(p, 1024.0);
        let local = origin.camera_relative(p);
        assert!(local.x.abs() < 1024.0);
        assert!(local.y.abs() < 1024.0);
        assert!(local.z.abs() < 1024.0);
    }

    #[test]
    fn origin_rebases_after_crossing_half_cell() {
        let p0 = CameraWorldPoint::new(0.0, 0.0, 0.0);
        let o0 = CameraWorldOrigin::for_camera(p0, 100.0);
        assert!(!o0.should_rebase(CameraWorldPoint::new(49.0, 0.0, 0.0)));
        assert!(o0.should_rebase(CameraWorldPoint::new(51.0, 0.0, 0.0)));
    }
}
