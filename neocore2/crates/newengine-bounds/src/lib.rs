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
