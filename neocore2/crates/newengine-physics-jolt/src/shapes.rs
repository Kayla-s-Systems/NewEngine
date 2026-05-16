use std::ptr;

use joltc_sys as sys;
use newengine_physics_api::{
    CollisionShapeDto, HeightfieldColliderDto, MeshColliderDto, PhysicsColliderDto,
    PhysicsFrameBodySnapshot, PhysicsFrameColliderSnapshot,
};

use crate::raw::{float3, sanitize_vec3, vec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BodySignature {
    pub(crate) kind_discriminant: u8,
    pub(crate) shape: ShapeSignature,
    pub(crate) is_trigger: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeSignature {
    pub(crate) kind_discriminant: u8,
    pub(crate) hash: u64,
}

impl BodySignature {
    #[inline]
    pub(crate) fn from_body(snapshot: &PhysicsFrameBodySnapshot) -> Self {
        Self {
            kind_discriminant: match snapshot.kind {
                newengine_physics_api::PhysicsBodyKindDto::Static => 0,
                newengine_physics_api::PhysicsBodyKindDto::Dynamic => 1,
                newengine_physics_api::PhysicsBodyKindDto::Kinematic => 2,
            },
            shape: ShapeSignature::from_body_shape(snapshot.shape),
            is_trigger: snapshot.flags.is_trigger,
        }
    }

    #[inline]
    pub(crate) fn from_collider(snapshot: &PhysicsFrameColliderSnapshot) -> Self {
        Self {
            kind_discriminant: 0,
            shape: ShapeSignature::from_collider(&snapshot.collider),
            is_trigger: snapshot.flags.is_trigger,
        }
    }
}

impl ShapeSignature {
    pub(crate) fn from_body_shape(shape: CollisionShapeDto) -> Self {
        let mut h = Hash64::new(0x7068_7973_626f_6479);
        match shape {
            CollisionShapeDto::Box { half_extents } => {
                h.push_u8(1);
                h.push_vec3(half_extents);
                Self { kind_discriminant: 1, hash: h.finish() }
            }
            CollisionShapeDto::Sphere { radius } => {
                h.push_u8(2);
                h.push_f32(radius);
                Self { kind_discriminant: 2, hash: h.finish() }
            }
            CollisionShapeDto::Capsule { radius, half_height } => {
                h.push_u8(3);
                h.push_f32(radius);
                h.push_f32(half_height);
                Self { kind_discriminant: 3, hash: h.finish() }
            }
        }
    }

    pub(crate) fn from_collider(collider: &PhysicsColliderDto) -> Self {
        let mut h = Hash64::new(0x7068_7973_636f_6c6c);
        match collider {
            PhysicsColliderDto::Heightfield(heightfield) => {
                h.push_u8(10);
                h.push_u32(heightfield.sample_count_x);
                h.push_u32(heightfield.sample_count_z);
                h.push_f32(heightfield.spacing[0]);
                h.push_f32(heightfield.spacing[1]);
                h.push_vec3(heightfield.local_origin);
                for v in &heightfield.heights {
                    h.push_f32(*v);
                }
                Self { kind_discriminant: 10, hash: h.finish() }
            }
            PhysicsColliderDto::Mesh(mesh) => {
                h.push_u8(11);
                h.push_u32(mesh.vertices.len() as u32);
                h.push_u32(mesh.triangles.len() as u32);
                for v in &mesh.vertices {
                    h.push_vec3(*v);
                }
                for tri in &mesh.triangles {
                    h.push_u32(tri[0]);
                    h.push_u32(tri[1]);
                    h.push_u32(tri[2]);
                }
                Self { kind_discriminant: 11, hash: h.finish() }
            }
        }
    }
}

pub(crate) fn create_body_shape(shape: CollisionShapeDto, density: f32) -> Result<*mut sys::JPC_Shape, String> {
    let mut out_shape: *mut sys::JPC_Shape = ptr::null_mut();
    let mut out_error: *mut sys::JPC_String = ptr::null_mut();

    let ok = match shape {
        CollisionShapeDto::Box { half_extents } => {
            let mut settings = sys::JPC_BoxShapeSettings::default();
            settings.HalfExtent = vec3(sanitize_vec3(half_extents, 0.001));
            settings.Density = density;
            unsafe { sys::JPC_BoxShapeSettings_Create(&settings, &mut out_shape, &mut out_error) }
        }
        CollisionShapeDto::Sphere { radius } => {
            let mut settings = sys::JPC_SphereShapeSettings::default();
            settings.Radius = radius.max(0.001);
            settings.Density = density;
            unsafe { sys::JPC_SphereShapeSettings_Create(&settings, &mut out_shape, &mut out_error) }
        }
        CollisionShapeDto::Capsule { radius, half_height } => {
            let mut settings = sys::JPC_CapsuleShapeSettings::default();
            settings.Radius = radius.max(0.001);
            settings.HalfHeightOfCylinder = half_height.max(0.001);
            settings.Density = density;
            unsafe { sys::JPC_CapsuleShapeSettings_Create(&settings, &mut out_shape, &mut out_error) }
        }
    };

    finish_shape(ok, out_shape, out_error, "Jolt rigid-body shape creation failed")
}

pub(crate) fn create_collider_shape(collider: &PhysicsColliderDto) -> Result<*mut sys::JPC_Shape, String> {
    match collider {
        PhysicsColliderDto::Heightfield(heightfield) => create_heightfield_shape(heightfield),
        PhysicsColliderDto::Mesh(mesh) => create_mesh_shape(mesh),
    }
}

fn create_heightfield_shape(heightfield: &HeightfieldColliderDto) -> Result<*mut sys::JPC_Shape, String> {
    if !heightfield.is_square_for_native_heightfield() {
        return create_mesh_shape(&heightfield_to_mesh(heightfield)?);
    }

    let sample_count = heightfield.sample_count().unwrap_or(0).max(2);
    let mut out_shape: *mut sys::JPC_Shape = ptr::null_mut();
    let mut out_error: *mut sys::JPC_String = ptr::null_mut();
    let mut settings = sys::JPC_HeightFieldShapeSettings::default();
    settings.Samples = heightfield.heights.as_ptr();
    settings.SampleCount = sample_count;
    settings.Offset = vec3(heightfield.local_origin);
    settings.Scale = vec3([heightfield.spacing[0].max(0.001), 1.0, heightfield.spacing[1].max(0.001)]);
    settings.MinHeightValue = heightfield.min_height;
    settings.MaxHeightValue = heightfield.max_height;
    settings.BlockSize = 2;
    settings.BitsPerSample = 8;

    let ok = unsafe { sys::JPC_HeightFieldShapeSettings_Create(&settings, &mut out_shape, &mut out_error) };
    finish_shape(ok, out_shape, out_error, "Jolt heightfield shape creation failed")
}

fn create_mesh_shape(mesh: &MeshColliderDto) -> Result<*mut sys::JPC_Shape, String> {
    if mesh.is_empty() {
        return Err("Jolt mesh shape creation failed: empty mesh collider".to_owned());
    }

    let vertices = mesh.vertices.iter().copied().map(float3).collect::<Vec<_>>();
    let triangles = mesh
        .triangles
        .iter()
        .enumerate()
        .map(|(i, tri)| sys::JPC_IndexedTriangle {
            idx: *tri,
            materialIndex: mesh.material_indices.get(i).copied().unwrap_or(0),
        })
        .collect::<Vec<_>>();

    let mut out_shape: *mut sys::JPC_Shape = ptr::null_mut();
    let mut out_error: *mut sys::JPC_String = ptr::null_mut();
    let mut settings = sys::JPC_MeshShapeSettings::default();
    settings.Vertices = vertices.as_ptr();
    settings.VerticesLen = vertices.len();
    settings.Triangles = triangles.as_ptr();
    settings.TrianglesLen = triangles.len();
    settings.MaxTrianglesPerLeaf = 8;

    let ok = unsafe { sys::JPC_MeshShapeSettings_Create(&settings, &mut out_shape, &mut out_error) };
    finish_shape(ok, out_shape, out_error, "Jolt mesh shape creation failed")
}

fn heightfield_to_mesh(heightfield: &HeightfieldColliderDto) -> Result<MeshColliderDto, String> {
    let sx = heightfield.sample_count_x as usize;
    let sz = heightfield.sample_count_z as usize;
    if sx < 2 || sz < 2 || heightfield.heights.len() != sx * sz {
        return Err("invalid heightfield packet for mesh conversion".to_owned());
    }

    let mut vertices = Vec::with_capacity(sx * sz);
    for z in 0..sz {
        for x in 0..sx {
            let h = heightfield.heights[z * sx + x];
            vertices.push([
                heightfield.local_origin[0] + x as f32 * heightfield.spacing[0],
                heightfield.local_origin[1] + h,
                heightfield.local_origin[2] + z as f32 * heightfield.spacing[1],
            ]);
        }
    }

    let mut triangles = Vec::with_capacity((sx - 1) * (sz - 1) * 2);
    for z in 0..(sz - 1) {
        for x in 0..(sx - 1) {
            let a = (z * sx + x) as u32;
            let b = (z * sx + x + 1) as u32;
            let c = ((z + 1) * sx + x) as u32;
            let d = ((z + 1) * sx + x + 1) as u32;
            triangles.push([a, c, b]);
            triangles.push([b, c, d]);
        }
    }

    Ok(MeshColliderDto { vertices, triangles, material_indices: Vec::new() })
}

fn finish_shape(
    ok: bool,
    shape: *mut sys::JPC_Shape,
    error: *mut sys::JPC_String,
    fallback: &str,
) -> Result<*mut sys::JPC_Shape, String> {
    if ok && !shape.is_null() {
        return Ok(shape);
    }

    let message = if error.is_null() {
        fallback.to_owned()
    } else {
        let c_str = unsafe { sys::JPC_String_c_str(error) };
        if c_str.is_null() {
            fallback.to_owned()
        } else {
            unsafe { std::ffi::CStr::from_ptr(c_str) }
                .to_string_lossy()
                .into_owned()
        }
    };
    if !error.is_null() {
        unsafe { sys::JPC_String_delete(error) };
    }
    Err(message)
}

struct Hash64(u64);

impl Hash64 {
    #[inline]
    fn new(seed: u64) -> Self { Self(0xcbf2_9ce4_8422_2325 ^ seed) }

    #[inline]
    fn push_u8(&mut self, v: u8) { self.push_u64(v as u64); }

    #[inline]
    fn push_u32(&mut self, v: u32) { self.push_u64(v as u64); }

    #[inline]
    fn push_f32(&mut self, v: f32) { self.push_u32(v.to_bits()); }

    #[inline]
    fn push_vec3(&mut self, v: [f32; 3]) {
        self.push_f32(v[0]);
        self.push_f32(v[1]);
        self.push_f32(v[2]);
    }

    #[inline]
    fn push_u64(&mut self, v: u64) {
        self.0 ^= v;
        self.0 = self.0.wrapping_mul(0x1000_0000_01b3);
    }

    #[inline]
    fn finish(self) -> u64 { self.0 }
}
