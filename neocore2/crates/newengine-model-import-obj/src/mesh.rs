use newengine_math::Vec3;
use newengine_primitives::{PrimitiveMesh, PrimitiveVertex};

use crate::types::{ObjCorner, ObjPartBuilder};
use crate::ObjPart;

#[inline]
fn vec3(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}

#[inline]
fn array(value: Vec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}

#[inline]
fn vertex(
    corner: ObjCorner,
    positions: &[[f32; 3]],
    uvs: &[[f32; 2]],
    normals: &[[f32; 3]],
    fallback_normal: [f32; 3],
) -> PrimitiveVertex {
    PrimitiveVertex {
        pos: positions[corner.pos],
        nrm: corner
            .nrm
            .and_then(|index| normals.get(index).copied())
            .unwrap_or(fallback_normal),
        uv: corner
            .uv
            .and_then(|index| uvs.get(index).copied())
            .unwrap_or([0.0, 0.0]),
    }
}

pub(crate) fn push_triangle(
    part: &mut ObjPartBuilder,
    triangle: [ObjCorner; 3],
    positions: &[[f32; 3]],
    uvs: &[[f32; 2]],
    normals: &[[f32; 3]],
) {
    let [a, b, c] = triangle.map(|corner| vec3(positions[corner.pos]));
    let normal = (b - a).cross(c - a).normalize_or_zero();
    let fallback = if normal.length_squared() > 0.0 {
        array(normal)
    } else {
        [0.0, 1.0, 0.0]
    };

    for corner in triangle {
        let index = part.vertices.len() as u32;
        part.vertices
            .push(vertex(corner, positions, uvs, normals, fallback));
        part.indices.push(index);
    }
}

pub(crate) fn mesh_from_builder(mut builder: ObjPartBuilder) -> Option<PrimitiveMesh> {
    if builder.vertices.is_empty() || builder.indices.is_empty() {
        return None;
    }

    let (min, max) = vertex_bounds(&builder.vertices)?;
    let center = (min + max) * 0.5;
    let radius = bounds_radius(&builder.vertices, center);
    let max_index = builder.vertices.len().saturating_sub(1) as u32;
    for index in &mut builder.indices {
        *index = (*index).min(max_index);
    }

    Some(PrimitiveMesh {
        vertices: builder.vertices,
        indices: builder.indices,
        bounds_center: center,
        bounds_radius: radius.max(0.001),
    })
}

pub(crate) fn normalize_parts(parts: &mut [ObjPart], target_height: f32) {
    let Some((min, max)) = part_bounds(parts) else {
        return;
    };
    let height = (max.y - min.y).abs();
    if !height.is_finite() || height <= 0.0001 {
        return;
    }

    let scale = target_height.clamp(0.25, 3.0) / height;
    let origin = Vec3::new((min.x + max.x) * 0.5, min.y, (min.z + max.z) * 0.5);

    for part in parts {
        for vertex in &mut part.mesh.vertices {
            let position = (vec3(vertex.pos) - origin) * scale;
            vertex.pos = array(position);
        }
        refresh_bounds(&mut part.mesh);
    }
}

fn part_bounds(parts: &[ObjPart]) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found = false;
    for part in parts {
        for vertex in &part.mesh.vertices {
            let position = vec3(vertex.pos);
            min = min.min(position);
            max = max.max(position);
            found = true;
        }
    }
    found.then_some((min, max))
}

fn vertex_bounds(vertices: &[PrimitiveVertex]) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found = false;
    for vertex in vertices {
        let position = vec3(vertex.pos);
        min = min.min(position);
        max = max.max(position);
        found = true;
    }
    found.then_some((min, max))
}

fn bounds_radius(vertices: &[PrimitiveVertex], center: Vec3) -> f32 {
    vertices.iter().fold(0.0, |radius, vertex| {
        radius.max((vec3(vertex.pos) - center).length())
    })
}

fn refresh_bounds(mesh: &mut PrimitiveMesh) {
    let Some((min, max)) = vertex_bounds(&mesh.vertices) else {
        return;
    };
    let center = (min + max) * 0.5;
    mesh.bounds_center = center;
    mesh.bounds_radius = bounds_radius(&mesh.vertices, center).max(0.001);
}
