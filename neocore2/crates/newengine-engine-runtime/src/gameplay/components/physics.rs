use super::*;

/// Provider-neutral physics-world parameters consumed by the host physics bridge.
/// Product/gameplay packages may project their authored policy into this resource;
/// the reusable engine runtime must never read product-specific tuning directly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsWorldSettings {
    pub gravity: f32,
    pub contact_skin: f32,
    /// Maximum changed/new static triangle-mesh colliders registered per fixed tick.
    /// Bounding registration prevents large authored worlds from creating a single startup spike.
    pub static_collider_batch_size: usize,
}

impl PhysicsWorldSettings {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            gravity: if self.gravity.is_finite() {
                self.gravity.clamp(0.0, 80.0)
            } else {
                9.81
            },
            contact_skin: if self.contact_skin.is_finite() {
                self.contact_skin.clamp(0.0, 0.50)
            } else {
                0.035
            },
            static_collider_batch_size: self.static_collider_batch_size.clamp(1, 4096),
        }
    }
}

impl Default for PhysicsWorldSettings {
    #[inline]
    fn default() -> Self {
        Self {
            gravity: 9.81,
            contact_skin: 0.035,
            static_collider_batch_size: 128,
        }
    }
}

/// Gameplay-facing classification attached to collidable ECS entities.
/// Physics backends remain material-agnostic; footsteps, impacts and VFX resolve this component
/// from the stable entity key returned by physics queries/contact events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhysicsSurface {
    pub id: String,
    pub footstep_event: String,
    pub landing_event: String,
}

impl Default for PhysicsSurface {
    fn default() -> Self {
        Self {
            id: "surface.default".to_owned(),
            footstep_event: "audio.footstep.default".to_owned(),
            landing_event: "audio.landing.default".to_owned(),
        }
    }
}

/// Runtime-owned static triangle-mesh collider. The component stores provider-neutral
/// arrays and is projected into `PhysicsColliderDto::Mesh` by the physics frame bridge.
/// Vertices are local to the entity transform; authored scale must be baked before attach.
#[derive(Clone, Debug)]
pub struct StaticMeshCollider {
    pub vertices: Arc<[[f32; 3]]>,
    pub triangles: Arc<[[u32; 3]]>,
    pub local_bounds: Aabb,
    /// Stable geometry revision used by the physics bridge to register static
    /// meshes only when their shape or authored transform changes.
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
