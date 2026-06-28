use newengine_bounds::{Aabb, Bounds, Sphere};
use newengine_math::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CollisionShapeDesc {
    Box { half_extents: [f32; 3] },
    Sphere { radius: f32 },
    Capsule { radius: f32, half_height: f32 },
}

impl Default for CollisionShapeDesc {
    #[inline]
    fn default() -> Self {
        Self::Box {
            half_extents: [0.5, 0.5, 0.5],
        }
    }
}

impl CollisionShapeDesc {
    #[inline]
    pub fn sanitized(self) -> Self {
        match self {
            Self::Box { half_extents } => Self::Box {
                half_extents: [
                    half_extents[0].abs().clamp(0.001, 10_000.0),
                    half_extents[1].abs().clamp(0.001, 10_000.0),
                    half_extents[2].abs().clamp(0.001, 10_000.0),
                ],
            },
            Self::Sphere { radius } => Self::Sphere {
                radius: radius.abs().clamp(0.001, 10_000.0),
            },
            Self::Capsule {
                radius,
                half_height,
            } => Self::Capsule {
                radius: radius.abs().clamp(0.001, 10_000.0),
                half_height: half_height.abs().clamp(0.0, 10_000.0),
            },
        }
    }

    #[inline]
    pub fn local_aabb(self) -> Aabb {
        match self.sanitized() {
            Self::Box { half_extents } => Aabb::from_center_half_extents(
                Vec3::ZERO,
                Vec3::new(half_extents[0], half_extents[1], half_extents[2]),
            ),
            Self::Sphere { radius } => {
                Aabb::from_center_half_extents(Vec3::ZERO, Vec3::splat(radius))
            }
            Self::Capsule {
                radius,
                half_height,
            } => Aabb::from_center_half_extents(
                Vec3::ZERO,
                Vec3::new(radius, half_height + radius, radius),
            ),
        }
    }

    #[inline]
    pub fn local_sphere(self) -> Sphere {
        match self.sanitized() {
            Self::Box { half_extents } => {
                let he = Vec3::new(half_extents[0], half_extents[1], half_extents[2]);
                Sphere::new(Vec3::ZERO, he.length().max(0.001))
            }
            Self::Sphere { radius } => Sphere::new(Vec3::ZERO, radius),
            Self::Capsule {
                radius,
                half_height,
            } => Sphere::new(Vec3::ZERO, (half_height + radius).max(0.001)),
        }
    }

    #[inline]
    pub fn to_bounds(self) -> Bounds {
        match self {
            Self::Sphere { .. } => Bounds::from_local_sphere(self.local_sphere()),
            _ => Bounds::from_local_aabb(self.local_aabb()),
        }
    }
}
