use crate::{Aabb, Sphere};

/// Kind of bounds stored in [`Bounds`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundsKind {
    Aabb,
    Sphere,
}

/// World-space bounds component.
///
/// The component stores both local-space source data and derived world-space data.
/// Systems are expected to keep `world_*` fields updated.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub kind: BoundsKind,

    pub local_aabb: Aabb,
    pub world_aabb: Aabb,

    pub local_sphere: Sphere,
    pub world_sphere: Sphere,
}

impl Bounds {
    #[inline]
    pub fn from_local_aabb(aabb: Aabb) -> Self {
        let sphere = crate::aabb_to_sphere(aabb);
        Self {
            kind: BoundsKind::Aabb,
            local_aabb: aabb,
            world_aabb: aabb,
            local_sphere: sphere,
            world_sphere: sphere,
        }
    }

    #[inline]
    pub fn from_local_sphere(sphere: Sphere) -> Self {
        let aabb = crate::sphere_to_aabb(sphere);
        Self {
            kind: BoundsKind::Sphere,
            local_aabb: aabb,
            world_aabb: aabb,
            local_sphere: sphere,
            world_sphere: sphere,
        }
    }
}
