#![forbid(unsafe_op_in_unsafe_fn)]

//! ECS-facing physics world contracts.
//!
//! This crate is the narrow boundary between world/domain runtimes and the provider-neutral
//! physics transport. It deliberately does not depend on the engine composition root.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use newengine_bounds::Aabb;
use newengine_ecs::{EntityId, World};
use newengine_math::Vec3;
use newengine_physics_api::{PhysicsQueryDto, PhysicsQueryHitDto};
use newengine_transform::Transform;

/// Capability-owned contributor of provider-neutral physics queries.
///
/// Implementations own query semantics and result projection. The engine physics bridge owns only
/// batching, transport and deterministic scheduling. Keeping this contract outside the composition
/// root allows audio, gameplay, camera and future capabilities to contribute queries without
/// depending on `newengine-engine-runtime`.
pub trait GameplayPhysicsQueryProvider: Send + Sync {
    fn id(&self) -> &'static str;

    fn collect_queries(&self, world: &World) -> Vec<PhysicsQueryDto>;

    fn resolve_query_hits(
        &self,
        world: &mut World,
        fixed_tick: u64,
        hits: &[PhysicsQueryHitDto],
        key_to_entity: &BTreeMap<u64, EntityId>,
    ) -> BTreeSet<u64>;
}

/// Gameplay-facing project-authored surface semantics attached to collidable ECS entities.
/// Physics backends remain material-agnostic; domain capabilities resolve the opaque surface id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhysicsSurface {
    pub id: String,
    pub event_bindings: BTreeMap<String, String>,
}

impl Default for PhysicsSurface {
    fn default() -> Self {
        Self {
            id: String::new(),
            event_bindings: BTreeMap::new(),
        }
    }
}

impl PhysicsSurface {
    #[inline]
    pub fn event_for(&self, signal: &str) -> Option<&str> {
        let signal = signal.trim();
        if signal.is_empty() {
            return None;
        }
        self.event_bindings
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(signal))
            .map(|(_, value)| value.trim())
            .filter(|value| !value.is_empty())
    }

    pub fn with_event(mut self, signal: impl Into<String>, event_id: impl Into<String>) -> Self {
        let signal = signal.into();
        let event_id = event_id.into();
        if !signal.trim().is_empty() && !event_id.trim().is_empty() {
            self.event_bindings.insert(signal, event_id);
        }
        self
    }
}

/// Provider-neutral static triangle-mesh collider component.
/// Vertices are local to the entity transform; authored scale must be baked before attachment.
#[derive(Clone, Debug)]
pub struct StaticMeshCollider {
    pub vertices: Arc<[[f32; 3]]>,
    pub triangles: Arc<[[u32; 3]]>,
    pub local_bounds: Aabb,
    pub revision: u64,
    pub friction: f32,
    pub restitution: f32,
}

impl StaticMeshCollider {
    pub fn new(vertices: Vec<[f32; 3]>, triangles: Vec<[u32; 3]>) -> Result<Self, String> {
        if vertices.is_empty() || triangles.is_empty() {
            return Err("static mesh collider requires vertices and triangles".to_owned());
        }
        for (triangle_index, triangle) in triangles.iter().enumerate() {
            if triangle
                .iter()
                .any(|index| *index as usize >= vertices.len())
            {
                return Err(format!(
                    "static mesh collider triangle out of bounds triangle={} vertices={}",
                    triangle_index,
                    vertices.len()
                ));
            }
        }
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for vertex in &vertices {
            let point = Vec3::new(vertex[0], vertex[1], vertex[2]);
            if !point.is_finite() {
                return Err("static mesh collider contains non-finite vertex".to_owned());
            }
            min = min.min(point);
            max = max.max(point);
        }
        let revision = static_mesh_revision(&vertices, &triangles);
        Ok(Self {
            vertices: Arc::from(vertices.into_boxed_slice()),
            triangles: Arc::from(triangles.into_boxed_slice()),
            local_bounds: Aabb::new(min, max),
            revision,
            friction: 0.92,
            restitution: 0.0,
        })
    }

    #[inline]
    pub fn with_material(mut self, friction: f32, restitution: f32) -> Self {
        self.friction = friction.clamp(0.0, 10.0);
        self.restitution = restitution.clamp(0.0, 1.0);
        self
    }

    pub fn runtime_revision(&self, transform: Transform) -> u64 {
        let mut hash = StableFnv64::from_seed(self.revision);
        for bits in [
            transform.position.x.to_bits(),
            transform.position.y.to_bits(),
            transform.position.z.to_bits(),
            transform.rotation.x.to_bits(),
            transform.rotation.y.to_bits(),
            transform.rotation.z.to_bits(),
            transform.rotation.w.to_bits(),
            self.friction.to_bits(),
            self.restitution.to_bits(),
        ] {
            hash.push_u32(bits);
        }
        hash.finish()
    }
}

struct StableFnv64(u64);

impl StableFnv64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x100_0000_01b3;

    #[inline]
    const fn from_seed(seed: u64) -> Self {
        Self(seed)
    }

    #[inline]
    fn push_u32(&mut self, value: u32) {
        self.0 ^= u64::from(value);
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }

    #[inline]
    const fn finish(self) -> u64 {
        self.0
    }
}

fn static_mesh_revision(vertices: &[[f32; 3]], triangles: &[[u32; 3]]) -> u64 {
    let mut hash = StableFnv64::from_seed(StableFnv64::OFFSET_BASIS);
    for vertex in vertices {
        for component in vertex {
            hash.push_u32(component.to_bits());
        }
    }
    for triangle in triangles {
        for index in triangle {
            hash.push_u32(*index);
        }
    }
    hash.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_mesh_rejects_invalid_indices() {
        assert!(StaticMeshCollider::new(vec![[0.0, 0.0, 0.0]], vec![[0, 1, 0]]).is_err());
    }

    #[test]
    fn surface_event_lookup_is_case_insensitive() {
        let surface = PhysicsSurface::default().with_event("Contact", "vfx.contact");
        assert_eq!(surface.event_for("contact"), Some("vfx.contact"));
    }
}
