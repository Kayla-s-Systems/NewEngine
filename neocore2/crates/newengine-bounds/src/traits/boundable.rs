use crate::Aabb;

/// Trait for objects that can provide local-space bounds.
pub trait Boundable {
    /// Returns local-space bounding box.
    fn local_bounds(&self) -> Aabb;
}