use glam::Vec3;

/// Bounding sphere in 3D space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f32,
}

impl Sphere {
    #[inline]
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }

    #[inline]
    pub fn contains_point(&self, p: Vec3) -> bool {
        self.center.distance_squared(p) <= self.radius * self.radius
    }
}
