use newengine_bounds::Bounds;
use newengine_ecs::EntityId;

use crate::{CollisionShapeDesc, PhysicsMaterialDesc};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PhysicsHandle(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicsBodyKind {
    Static,
    Dynamic,
    Kinematic,
}

impl Default for PhysicsBodyKind {
    #[inline]
    fn default() -> Self { Self::Static }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhysicsBodyRuntimeFlags {
    pub is_trigger: bool,
    pub participates_in_queries: bool,
    pub casts_contacts: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsBodyDesc {
    pub shape: CollisionShapeDesc,
    pub kind: PhysicsBodyKind,
    pub flags: PhysicsBodyRuntimeFlags,
    pub material: PhysicsMaterialDesc,
}

impl Default for PhysicsBodyDesc {
    #[inline]
    fn default() -> Self {
        Self {
            shape: CollisionShapeDesc::default(),
            kind: PhysicsBodyKind::Static,
            flags: PhysicsBodyRuntimeFlags {
                is_trigger: false,
                participates_in_queries: true,
                casts_contacts: true,
            },
            material: PhysicsMaterialDesc::default(),
        }
    }
}

impl PhysicsBodyDesc {
    #[inline]
    pub const fn static_solid(shape: CollisionShapeDesc) -> Self {
        Self {
            shape,
            kind: PhysicsBodyKind::Static,
            flags: PhysicsBodyRuntimeFlags { is_trigger: false, participates_in_queries: true, casts_contacts: true },
            material: PhysicsMaterialDesc { friction: 0.75, restitution: 0.05, density: 1.0 },
        }
    }

    #[inline]
    pub const fn dynamic_solid(shape: CollisionShapeDesc) -> Self {
        Self {
            shape,
            kind: PhysicsBodyKind::Dynamic,
            flags: PhysicsBodyRuntimeFlags { is_trigger: false, participates_in_queries: true, casts_contacts: true },
            material: PhysicsMaterialDesc { friction: 0.75, restitution: 0.05, density: 1.0 },
        }
    }

    #[inline]
    pub const fn trigger(shape: CollisionShapeDesc) -> Self {
        Self {
            shape,
            kind: PhysicsBodyKind::Static,
            flags: PhysicsBodyRuntimeFlags { is_trigger: true, participates_in_queries: true, casts_contacts: false },
            material: PhysicsMaterialDesc { friction: 0.75, restitution: 0.05, density: 1.0 },
        }
    }

    #[inline]
    pub const fn dynamic(self) -> bool {
        matches!(self.kind, PhysicsBodyKind::Dynamic)
    }

    #[inline]
    pub const fn is_trigger(self) -> bool { self.flags.is_trigger }

    #[inline]
    pub fn to_bounds(self) -> Bounds { self.shape.to_bounds() }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterControllerDesc {
    pub entity: EntityId,
    pub radius: f32,
    pub half_height: f32,
    pub contact_skin: f32,
    pub max_slope_radians: f32,
}

impl CharacterControllerDesc {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            entity: self.entity,
            radius: self.radius.clamp(0.05, 5.0),
            half_height: self.half_height.clamp(0.05, 8.0),
            contact_skin: self.contact_skin.clamp(0.0, 0.5),
            max_slope_radians: self.max_slope_radians.clamp(0.0, core::f32::consts::FRAC_PI_2),
        }
    }
}
