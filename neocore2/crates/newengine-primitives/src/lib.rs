#![forbid(unsafe_op_in_unsafe_fn)]

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

/// Built-in primitive kinds.
///
/// This is a minimal, deterministic set meant for editor bootstrap and tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrimitiveKind {
    Cube,
    Plane,
}

impl Default for PrimitiveKind {
    #[inline]
    fn default() -> Self {
        Self::Cube
    }
}

/// Renderable primitive component.
///
/// Concrete renderers are expected to map `kind` into GPU meshes.
#[derive(Clone, Copy, Debug)]
pub struct Primitive {
    pub kind: PrimitiveKind,
    pub color: [f32; 4],
}

impl Default for Primitive {
    #[inline]
    fn default() -> Self {
        Self {
            kind: PrimitiveKind::Cube,
            color: [0.85, 0.85, 0.9, 1.0],
        }
    }
}

/// Standard vertex format for primitives.
///
/// Layout:
/// - location 0: position (vec3)
/// - location 1: normal (vec3)
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Zeroable, Pod)]
pub struct PrimitiveVertex {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
}

/// Deterministic CPU mesh.
#[derive(Clone, Debug)]
pub struct PrimitiveMesh {
    pub vertices: Vec<PrimitiveVertex>,
    pub indices: Vec<u32>,
    pub bounds_center: Vec3,
    pub bounds_radius: f32,
}

/// Builds a deterministic primitive mesh.
#[inline]
pub fn build_mesh(kind: PrimitiveKind) -> PrimitiveMesh {
    match kind {
        PrimitiveKind::Cube => cube_mesh(),
        PrimitiveKind::Plane => plane_mesh(),
    }
}

#[inline]
fn cube_mesh() -> PrimitiveMesh {
    // Unit cube centered at origin. 24 vertices (4 per face) for hard normals.
    let p = |x: f32,
             y: f32,
             z: f32,
             nx: f32,
             ny: f32,
             nz: f32,
    | PrimitiveVertex {
        pos: [x, y, z],
        nrm: [nx, ny, nz],
    };

    let h = 0.5f32;

    let mut v: Vec<PrimitiveVertex> = Vec::with_capacity(24);

    // +Z
    v.extend_from_slice(&[
        p(-h, -h, h, 0.0, 0.0, 1.0),
        p(h, -h, h, 0.0, 0.0, 1.0),
        p(h, h, h, 0.0, 0.0, 1.0),
        p(-h, h, h, 0.0, 0.0, 1.0),
    ]);
    // -Z
    v.extend_from_slice(&[
        p(h, -h, -h, 0.0, 0.0, -1.0),
        p(-h, -h, -h, 0.0, 0.0, -1.0),
        p(-h, h, -h, 0.0, 0.0, -1.0),
        p(h, h, -h, 0.0, 0.0, -1.0),
    ]);
    // +X
    v.extend_from_slice(&[
        p(h, -h, h, 1.0, 0.0, 0.0),
        p(h, -h, -h, 1.0, 0.0, 0.0),
        p(h, h, -h, 1.0, 0.0, 0.0),
        p(h, h, h, 1.0, 0.0, 0.0),
    ]);
    // -X
    v.extend_from_slice(&[
        p(-h, -h, -h, -1.0, 0.0, 0.0),
        p(-h, -h, h, -1.0, 0.0, 0.0),
        p(-h, h, h, -1.0, 0.0, 0.0),
        p(-h, h, -h, -1.0, 0.0, 0.0),
    ]);
    // +Y
    v.extend_from_slice(&[
        p(-h, h, h, 0.0, 1.0, 0.0),
        p(h, h, h, 0.0, 1.0, 0.0),
        p(h, h, -h, 0.0, 1.0, 0.0),
        p(-h, h, -h, 0.0, 1.0, 0.0),
    ]);
    // -Y
    v.extend_from_slice(&[
        p(-h, -h, -h, 0.0, -1.0, 0.0),
        p(h, -h, -h, 0.0, -1.0, 0.0),
        p(h, -h, h, 0.0, -1.0, 0.0),
        p(-h, -h, h, 0.0, -1.0, 0.0),
    ]);

    let mut i: Vec<u32> = Vec::with_capacity(36);
    for f in 0..6u32 {
        let base = f * 4;
        i.extend_from_slice(&[
            base + 0,
            base + 1,
            base + 2,
            base + 0,
            base + 2,
            base + 3,
        ]);
    }

    PrimitiveMesh {
        vertices: v,
        indices: i,
        bounds_center: Vec3::ZERO,
        bounds_radius: Vec3::splat(h).length(),
    }
}

#[inline]
fn plane_mesh() -> PrimitiveMesh {
    // Unit plane on XZ, centered at origin, normal +Y.
    let h = 0.5f32;
    let v = vec![
        PrimitiveVertex {
            pos: [-h, 0.0, -h],
            nrm: [0.0, 1.0, 0.0],
        },
        PrimitiveVertex {
            pos: [h, 0.0, -h],
            nrm: [0.0, 1.0, 0.0],
        },
        PrimitiveVertex {
            pos: [h, 0.0, h],
            nrm: [0.0, 1.0, 0.0],
        },
        PrimitiveVertex {
            pos: [-h, 0.0, h],
            nrm: [0.0, 1.0, 0.0],
        },
    ];

    let i = vec![0u32, 1, 2, 0, 2, 3];

    PrimitiveMesh {
        vertices: v,
        indices: i,
        bounds_center: Vec3::ZERO,
        bounds_radius: Vec3::new(h, 0.0, h).length(),
    }
}
