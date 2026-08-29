//! Axis-aligned bounding box utilities.

#[allow(clippy::module_inception)]
mod aabb;
mod ops;
mod transform;

pub use aabb::Aabb;
