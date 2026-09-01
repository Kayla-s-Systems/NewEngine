#![forbid(unsafe_op_in_unsafe_fn)]

//! Bounds and volumes for derived scene state.
//!
//! Design goals:
//! - deterministic, allocation-free math
//! - minimal coupling to higher-level systems
//! - composable primitives for culling, physics broad-phase, and editor selection

pub mod aabb;
pub mod components;
pub mod sphere;
pub mod systems;
pub mod traits;

mod convert;

pub use aabb::Aabb;
pub use components::{Bounds, BoundsKind};
pub use sphere::Sphere;
pub use traits::Boundable;

pub use convert::{aabb_to_sphere, sphere_to_aabb};

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
