use serde::{Deserialize, Serialize};

use crate::{PhysicsEntityKey, PhysicsQuat, PhysicsVec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhysicsBodyKindDto {
    Static,
    Dynamic,
    Kinematic,
}

impl Default for PhysicsBodyKindDto {
    #[inline]
    fn default() -> Self {
        Self::Static
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CollisionShapeDto {
    Box { half_extents: PhysicsVec3 },
    Sphere { radius: f32 },
    Capsule { radius: f32, half_height: f32 },
}

impl Default for CollisionShapeDto {
    #[inline]
    fn default() -> Self {
        Self::Box {
            half_extents: [0.5, 0.5, 0.5],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeightfieldColliderDto {
    pub sample_count_x: u32,
    pub sample_count_z: u32,
    pub spacing: [f32; 2],
    pub local_origin: PhysicsVec3,
    #[serde(default)]
    pub heights: Vec<f32>,
    pub min_height: f32,
    pub max_height: f32,
}

impl HeightfieldColliderDto {
    #[inline]
    pub fn sample_count(&self) -> Option<u32> {
        (self.sample_count_x == self.sample_count_z).then_some(self.sample_count_x)
    }

    #[inline]
    pub fn expected_height_len(&self) -> usize {
        self.sample_count_x as usize * self.sample_count_z as usize
    }

    #[inline]
    pub fn is_square_for_native_heightfield(&self) -> bool {
        self.sample_count().is_some() && self.heights.len() == self.expected_height_len()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshColliderDto {
    #[serde(default)]
    pub vertices: Vec<PhysicsVec3>,
    #[serde(default)]
    pub triangles: Vec<[u32; 3]>,
    #[serde(default)]
    pub material_indices: Vec<u32>,
}

impl MeshColliderDto {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() || self.triangles.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PhysicsColliderDto {
    Heightfield(HeightfieldColliderDto),
    Mesh(MeshColliderDto),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsMaterialDto {
    pub friction: f32,
    pub restitution: f32,
    pub density: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsBodyFlagsDto {
    pub is_trigger: bool,
    pub participates_in_queries: bool,
    pub casts_contacts: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysicsFrameColliderSnapshot {
    pub entity: PhysicsEntityKey,
    pub collider: PhysicsColliderDto,
    pub flags: PhysicsBodyFlagsDto,
    pub material: PhysicsMaterialDto,
    pub position: PhysicsVec3,
    pub rotation: PhysicsQuat,
    pub bounds_min: PhysicsVec3,
    pub bounds_max: PhysicsVec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsFrameBodySnapshot {
    pub entity: PhysicsEntityKey,
    pub kind: PhysicsBodyKindDto,
    pub shape: CollisionShapeDto,
    pub flags: PhysicsBodyFlagsDto,
    pub material: PhysicsMaterialDto,
    pub position: PhysicsVec3,
    pub rotation: PhysicsQuat,
    pub linear_velocity: PhysicsVec3,
    pub bounds_min: PhysicsVec3,
    pub bounds_max: PhysicsVec3,
}
