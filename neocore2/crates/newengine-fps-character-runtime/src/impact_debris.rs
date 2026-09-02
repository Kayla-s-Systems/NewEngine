use super::*;

use newengine_gameplay_fps_api::{
    PendingImpactDebrisVisual, PersistentImpactDebris, PersistentImpactDebrisKind,
};
use newengine_materials::api::MaterialRegistryApi;
use newengine_materials::{MaterialDescriptor, MaterialFlags};
use newengine_model_domain_api::{MeshRenderOptions, MeshShadowPolicy};

const SHARD_VARIANTS_PER_SURFACE: usize = 3;
const IMPACT_SHARD_PRIMITIVE_IDS: [[PrimitiveId; SHARD_VARIANTS_PER_SURFACE]; 4] = [
    [
        PrimitiveId(fnv1a_64("northstar.impact_debris.concrete.chip_a.v2")),
        PrimitiveId(fnv1a_64("northstar.impact_debris.concrete.chip_b.v2")),
        PrimitiveId(fnv1a_64("northstar.impact_debris.concrete.chunk_a.v2")),
    ],
    [
        PrimitiveId(fnv1a_64("northstar.impact_debris.metal.fragment_a.v2")),
        PrimitiveId(fnv1a_64("northstar.impact_debris.metal.fragment_b.v2")),
        PrimitiveId(fnv1a_64("northstar.impact_debris.metal.fragment_c.v2")),
    ],
    [
        PrimitiveId(fnv1a_64("northstar.impact_debris.wood.splinter_a.v2")),
        PrimitiveId(fnv1a_64("northstar.impact_debris.wood.splinter_b.v2")),
        PrimitiveId(fnv1a_64("northstar.impact_debris.wood.chip_a.v2")),
    ],
    [
        PrimitiveId(fnv1a_64("northstar.impact_debris.glass.shard_a.v2")),
        PrimitiveId(fnv1a_64("northstar.impact_debris.glass.shard_b.v2")),
        PrimitiveId(fnv1a_64("northstar.impact_debris.glass.shard_c.v2")),
    ],
];

#[inline]
fn shard_vertex(position: Vec3, normal: Vec3, uv: [f32; 2]) -> PrimitiveVertex {
    PrimitiveVertex {
        pos: [position.x, position.y, position.z],
        nrm: [normal.x, normal.y, normal.z],
        uv,
    }
}

#[inline]
fn push_triangle(
    vertices: &mut Vec<PrimitiveVertex>,
    indices: &mut Vec<u32>,
    a: Vec3,
    b: Vec3,
    c: Vec3,
) {
    let normal = (b - a).cross(c - a).normalize_or_zero();
    let base = vertices.len() as u32;
    vertices.push(shard_vertex(a, normal, [0.0, 0.0]));
    vertices.push(shard_vertex(b, normal, [1.0, 0.0]));
    vertices.push(shard_vertex(c, normal, [0.5, 1.0]));
    indices.extend_from_slice(&[base, base + 1, base + 2]);
}

/// Closed irregular prism used as the authored low-poly source for impact clutter. GPU triangles are
/// only the tessellation format; the visible object is a closed multi-sided volume with a polygonal
/// silhouette, never a single flat triangular shard.
fn closed_irregular_prism(bottom: &[Vec3], top: &[Vec3]) -> PrimitiveMesh {
    assert_eq!(bottom.len(), top.len());
    assert!(bottom.len() >= 4);

    let mut vertices = Vec::with_capacity((bottom.len() * 4 + (bottom.len() - 2) * 2) * 3);
    let mut indices = Vec::with_capacity(vertices.capacity());

    // Bottom cap faces outward in the opposite winding from the top cap.
    for i in 1..(bottom.len() - 1) {
        push_triangle(
            &mut vertices,
            &mut indices,
            bottom[0],
            bottom[i + 1],
            bottom[i],
        );
        push_triangle(&mut vertices, &mut indices, top[0], top[i], top[i + 1]);
    }

    // Side walls preserve the irregular polygon outline and provide actual 3D thickness.
    for i in 0..bottom.len() {
        let next = (i + 1) % bottom.len();
        push_triangle(
            &mut vertices,
            &mut indices,
            bottom[i],
            bottom[next],
            top[next],
        );
        push_triangle(&mut vertices, &mut indices, bottom[i], top[next], top[i]);
    }

    PrimitiveMesh {
        vertices,
        indices,
        bounds_center: Vec3::ZERO,
        bounds_radius: 1.85,
    }
}

fn concrete_shard_mesh(variant: usize) -> PrimitiveMesh {
    match variant % SHARD_VARIANTS_PER_SURFACE {
        0 => closed_irregular_prism(
            &[
                Vec3::new(-0.92, -0.52, -0.72),
                Vec3::new(-0.18, -0.82, -0.68),
                Vec3::new(0.78, -0.54, -0.60),
                Vec3::new(0.96, 0.18, -0.48),
                Vec3::new(0.24, 0.86, -0.58),
                Vec3::new(-0.72, 0.62, -0.66),
            ],
            &[
                Vec3::new(-0.72, -0.36, 0.66),
                Vec3::new(-0.06, -0.64, 0.82),
                Vec3::new(0.66, -0.38, 0.56),
                Vec3::new(0.72, 0.26, 0.74),
                Vec3::new(0.14, 0.64, 0.92),
                Vec3::new(-0.62, 0.44, 0.54),
            ],
        ),
        1 => closed_irregular_prism(
            &[
                Vec3::new(-1.00, -0.30, -0.54),
                Vec3::new(-0.42, -0.76, -0.62),
                Vec3::new(0.42, -0.68, -0.48),
                Vec3::new(0.94, -0.08, -0.64),
                Vec3::new(0.54, 0.72, -0.56),
                Vec3::new(-0.30, 0.88, -0.46),
                Vec3::new(-0.88, 0.40, -0.60),
            ],
            &[
                Vec3::new(-0.78, -0.18, 0.62),
                Vec3::new(-0.34, -0.58, 0.78),
                Vec3::new(0.34, -0.54, 0.72),
                Vec3::new(0.76, 0.04, 0.48),
                Vec3::new(0.42, 0.58, 0.68),
                Vec3::new(-0.20, 0.70, 0.86),
                Vec3::new(-0.70, 0.30, 0.56),
            ],
        ),
        _ => closed_irregular_prism(
            &[
                Vec3::new(-0.74, -0.70, -0.78),
                Vec3::new(0.08, -0.92, -0.54),
                Vec3::new(0.88, -0.34, -0.70),
                Vec3::new(0.72, 0.54, -0.52),
                Vec3::new(-0.06, 0.94, -0.66),
                Vec3::new(-0.86, 0.34, -0.50),
            ],
            &[
                Vec3::new(-0.58, -0.52, 0.54),
                Vec3::new(0.12, -0.70, 0.76),
                Vec3::new(0.62, -0.24, 0.58),
                Vec3::new(0.54, 0.44, 0.84),
                Vec3::new(-0.02, 0.72, 0.62),
                Vec3::new(-0.64, 0.28, 0.74),
            ],
        ),
    }
}

fn metal_shard_mesh(variant: usize) -> PrimitiveMesh {
    match variant % SHARD_VARIANTS_PER_SURFACE {
        0 => closed_irregular_prism(
            &[
                Vec3::new(-1.00, -0.48, -0.22),
                Vec3::new(-0.24, -0.70, -0.18),
                Vec3::new(0.94, -0.34, -0.16),
                Vec3::new(0.72, 0.54, -0.20),
                Vec3::new(-0.48, 0.72, -0.14),
            ],
            &[
                Vec3::new(-0.90, -0.42, 0.18),
                Vec3::new(-0.18, -0.62, 0.24),
                Vec3::new(0.82, -0.28, 0.16),
                Vec3::new(0.62, 0.46, 0.22),
                Vec3::new(-0.40, 0.62, 0.14),
            ],
        ),
        1 => closed_irregular_prism(
            &[
                Vec3::new(-0.94, -0.28, -0.18),
                Vec3::new(-0.26, -0.64, -0.24),
                Vec3::new(0.88, -0.46, -0.12),
                Vec3::new(0.96, 0.16, -0.20),
                Vec3::new(0.18, 0.74, -0.14),
                Vec3::new(-0.74, 0.52, -0.22),
            ],
            &[
                Vec3::new(-0.82, -0.22, 0.16),
                Vec3::new(-0.20, -0.54, 0.20),
                Vec3::new(0.76, -0.38, 0.18),
                Vec3::new(0.84, 0.12, 0.14),
                Vec3::new(0.14, 0.62, 0.22),
                Vec3::new(-0.62, 0.42, 0.16),
            ],
        ),
        _ => closed_irregular_prism(
            &[
                Vec3::new(-1.00, -0.18, -0.16),
                Vec3::new(-0.40, -0.56, -0.20),
                Vec3::new(0.56, -0.64, -0.12),
                Vec3::new(0.98, -0.08, -0.18),
                Vec3::new(0.58, 0.54, -0.22),
                Vec3::new(-0.22, 0.70, -0.14),
                Vec3::new(-0.84, 0.34, -0.20),
            ],
            &[
                Vec3::new(-0.88, -0.14, 0.16),
                Vec3::new(-0.34, -0.46, 0.14),
                Vec3::new(0.48, -0.52, 0.20),
                Vec3::new(0.84, -0.04, 0.14),
                Vec3::new(0.50, 0.46, 0.16),
                Vec3::new(-0.18, 0.58, 0.22),
                Vec3::new(-0.72, 0.28, 0.14),
            ],
        ),
    }
}

fn wood_shard_mesh(variant: usize) -> PrimitiveMesh {
    match variant % SHARD_VARIANTS_PER_SURFACE {
        0 => closed_irregular_prism(
            &[
                Vec3::new(-0.42, -0.36, -1.00),
                Vec3::new(0.38, -0.30, -0.94),
                Vec3::new(0.54, 0.20, -0.82),
                Vec3::new(0.06, 0.48, -0.90),
                Vec3::new(-0.50, 0.18, -0.86),
            ],
            &[
                Vec3::new(-0.24, -0.20, 0.96),
                Vec3::new(0.24, -0.18, 1.00),
                Vec3::new(0.34, 0.12, 0.88),
                Vec3::new(0.02, 0.30, 0.94),
                Vec3::new(-0.30, 0.10, 0.86),
            ],
        ),
        1 => closed_irregular_prism(
            &[
                Vec3::new(-0.34, -0.28, -0.96),
                Vec3::new(0.46, -0.22, -1.00),
                Vec3::new(0.40, 0.26, -0.86),
                Vec3::new(-0.02, 0.44, -0.90),
                Vec3::new(-0.48, 0.10, -0.82),
            ],
            &[
                Vec3::new(-0.18, -0.18, 0.88),
                Vec3::new(0.26, -0.12, 0.96),
                Vec3::new(0.24, 0.16, 1.00),
                Vec3::new(-0.02, 0.28, 0.90),
                Vec3::new(-0.28, 0.06, 0.94),
            ],
        ),
        _ => closed_irregular_prism(
            &[
                Vec3::new(-0.76, -0.50, -0.56),
                Vec3::new(0.10, -0.72, -0.60),
                Vec3::new(0.78, -0.34, -0.48),
                Vec3::new(0.68, 0.42, -0.54),
                Vec3::new(-0.20, 0.72, -0.44),
                Vec3::new(-0.82, 0.28, -0.50),
            ],
            &[
                Vec3::new(-0.58, -0.38, 0.52),
                Vec3::new(0.10, -0.54, 0.62),
                Vec3::new(0.60, -0.24, 0.46),
                Vec3::new(0.52, 0.34, 0.58),
                Vec3::new(-0.14, 0.56, 0.50),
                Vec3::new(-0.64, 0.20, 0.60),
            ],
        ),
    }
}

fn glass_shard_mesh(variant: usize) -> PrimitiveMesh {
    match variant % SHARD_VARIANTS_PER_SURFACE {
        0 => closed_irregular_prism(
            &[
                Vec3::new(-0.92, -0.24, -0.10),
                Vec3::new(-0.26, -0.76, -0.08),
                Vec3::new(0.72, -0.52, -0.10),
                Vec3::new(0.94, 0.08, -0.08),
                Vec3::new(0.22, 0.78, -0.10),
                Vec3::new(-0.70, 0.52, -0.08),
            ],
            &[
                Vec3::new(-0.88, -0.22, 0.10),
                Vec3::new(-0.24, -0.72, 0.08),
                Vec3::new(0.68, -0.48, 0.10),
                Vec3::new(0.90, 0.08, 0.08),
                Vec3::new(0.20, 0.74, 0.10),
                Vec3::new(-0.66, 0.48, 0.08),
            ],
        ),
        1 => closed_irregular_prism(
            &[
                Vec3::new(-0.80, -0.50, -0.09),
                Vec3::new(0.04, -0.84, -0.08),
                Vec3::new(0.82, -0.34, -0.09),
                Vec3::new(0.66, 0.58, -0.08),
                Vec3::new(-0.18, 0.84, -0.09),
                Vec3::new(-0.88, 0.24, -0.08),
            ],
            &[
                Vec3::new(-0.76, -0.46, 0.09),
                Vec3::new(0.04, -0.80, 0.08),
                Vec3::new(0.78, -0.32, 0.09),
                Vec3::new(0.62, 0.54, 0.08),
                Vec3::new(-0.16, 0.80, 0.09),
                Vec3::new(-0.84, 0.22, 0.08),
            ],
        ),
        _ => closed_irregular_prism(
            &[
                Vec3::new(-0.96, -0.18, -0.08),
                Vec3::new(-0.34, -0.70, -0.10),
                Vec3::new(0.46, -0.76, -0.08),
                Vec3::new(0.92, -0.12, -0.10),
                Vec3::new(0.58, 0.58, -0.08),
                Vec3::new(-0.10, 0.82, -0.10),
                Vec3::new(-0.76, 0.42, -0.08),
            ],
            &[
                Vec3::new(-0.92, -0.16, 0.08),
                Vec3::new(-0.32, -0.66, 0.10),
                Vec3::new(0.44, -0.72, 0.08),
                Vec3::new(0.88, -0.10, 0.10),
                Vec3::new(0.56, 0.54, 0.08),
                Vec3::new(-0.10, 0.78, 0.10),
                Vec3::new(-0.72, 0.40, 0.08),
            ],
        ),
    }
}

#[inline]
const fn debris_kind_index(kind: PersistentImpactDebrisKind) -> usize {
    match kind {
        PersistentImpactDebrisKind::Concrete => 0,
        PersistentImpactDebrisKind::Metal => 1,
        PersistentImpactDebrisKind::Wood => 2,
        PersistentImpactDebrisKind::Glass => 3,
    }
}

fn shard_mesh(kind: PersistentImpactDebrisKind, variant: usize) -> PrimitiveMesh {
    match kind {
        PersistentImpactDebrisKind::Concrete => concrete_shard_mesh(variant),
        PersistentImpactDebrisKind::Metal => metal_shard_mesh(variant),
        PersistentImpactDebrisKind::Wood => wood_shard_mesh(variant),
        PersistentImpactDebrisKind::Glass => glass_shard_mesh(variant),
    }
}

fn ensure_shard_primitives(primitives: &mut PrimitiveRegistry) {
    for kind in [
        PersistentImpactDebrisKind::Concrete,
        PersistentImpactDebrisKind::Metal,
        PersistentImpactDebrisKind::Wood,
        PersistentImpactDebrisKind::Glass,
    ] {
        let kind_index = debris_kind_index(kind);
        for variant in 0..SHARD_VARIANTS_PER_SURFACE {
            let primitive_id = IMPACT_SHARD_PRIMITIVE_IDS[kind_index][variant];
            if !primitives.is_registered(primitive_id) {
                primitives.register_mesh(
                    primitive_id,
                    format!("ImpactDebris/{}/{variant}", kind.label()),
                    shard_mesh(kind, variant),
                );
            }
        }
    }
}

#[inline]
fn inherited_debris_material(
    materials: &MaterialRegistry,
    source_material_id: u64,
) -> Option<MaterialId> {
    let material = MaterialId(source_material_id);
    (material.is_valid() && materials.get(material).is_some()).then_some(material)
}

fn debris_material(materials: &MaterialRegistry, kind: PersistentImpactDebrisKind) -> MaterialId {
    let (name, base_color, roughness, metallic) = match kind {
        PersistentImpactDebrisKind::Concrete => {
            ("ImpactDebris/Concrete", [0.39, 0.37, 0.34, 1.0], 0.90, 0.0)
        }
        PersistentImpactDebrisKind::Metal => {
            ("ImpactDebris/Metal", [0.26, 0.28, 0.30, 1.0], 0.48, 0.82)
        }
        PersistentImpactDebrisKind::Wood => {
            ("ImpactDebris/Wood", [0.36, 0.19, 0.075, 1.0], 0.80, 0.0)
        }
        PersistentImpactDebrisKind::Glass => {
            ("ImpactDebris/Glass", [0.50, 0.66, 0.72, 1.0], 0.18, 0.08)
        }
    };
    materials.upsert_named(
        name,
        MaterialDescriptor {
            base_color,
            roughness,
            metallic,
            flags: MaterialFlags::RECEIVE_SHADOWS,
            ..MaterialDescriptor::default()
        },
    )
}

#[inline]
fn debris_render_options() -> MeshRenderOptions {
    let mut options = MeshRenderOptions::world_opaque();
    // Persistent frozen fragments receive world lighting/shadows but never become shadow-map
    // casters, keeping large firefight clutter sets cheap on GTX-class hardware.
    options.shadow_policy = MeshShadowPolicy::ReceiveOnly;
    options
}

pub(crate) fn tick_persistent_impact_debris_visuals(
    world: &mut newengine_ecs::World,
    primitives: &mut PrimitiveRegistry,
    materials: &MaterialRegistry,
) {
    let pending = world
        .query::<PendingImpactDebrisVisual>()
        .filter_map(|(entity, _)| {
            world
                .get::<PersistentImpactDebris>(entity)
                .copied()
                .map(|debris| (entity, debris))
        })
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return;
    }

    ensure_shard_primitives(primitives);
    let mut material_ids: [Option<MaterialId>; 4] = [None, None, None, None];
    for (entity, debris) in pending {
        let material_index = debris_kind_index(debris.kind);
        let material_id = inherited_debris_material(materials, debris.source_material_id)
            .unwrap_or_else(|| {
                *material_ids[material_index]
                    .get_or_insert_with(|| debris_material(materials, debris.kind))
            });
        let primitive_id = IMPACT_SHARD_PRIMITIVE_IDS[material_index]
            [debris.variant as usize % SHARD_VARIANTS_PER_SURFACE];
        let half_extents = Vec3::new(
            debris.half_extents[0].abs().max(0.001),
            debris.half_extents[1].abs().max(0.001),
            debris.half_extents[2].abs().max(0.001),
        );
        let child = crate::materials_terrain::spawn_game_primitive(
            world,
            &*primitives,
            materials,
            crate::materials_terrain::PrimitiveSpawnSpec {
                parent: entity,
                primitive_id,
                material_id,
                name: &format!(
                    "WeaponFx/ImpactDebris/Visual/{}/{:016x}",
                    debris.kind.label(),
                    entity.stable_u64()
                ),
                position: Vec3::ZERO,
                scale: half_extents,
                color: [1.0, 1.0, 1.0, 1.0],
                render_options: debris_render_options(),
            },
        );
        // Deterministic local twist makes repeated variants expose different irregular faces while
        // the physics root remains the sole owner of world-space motion.
        if let Some(transform) = world.get_mut_tracked::<Transform>(child) {
            transform.rotation = Quat::from_euler(
                EulerRot::YXZ,
                debris.variant as f32 * 0.71,
                debris.variant as f32 * 0.43,
                debris.variant as f32 * 0.29,
            );
        }
        let _ = world.remove::<PendingImpactDebrisVisual>(entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shard_meshes_are_closed_volumetric_and_surface_specific() {
        for kind in [
            PersistentImpactDebrisKind::Concrete,
            PersistentImpactDebrisKind::Metal,
            PersistentImpactDebrisKind::Wood,
            PersistentImpactDebrisKind::Glass,
        ] {
            for variant in 0..SHARD_VARIANTS_PER_SURFACE {
                let mesh = shard_mesh(kind, variant);
                assert!(
                    mesh.vertices.len() >= 30,
                    "{kind:?}/{variant} is too primitive"
                );
                assert!(mesh.indices.len() >= 30);
                assert!(mesh.indices.len().is_multiple_of(3));
                let mut min = [f32::INFINITY; 3];
                let mut max = [f32::NEG_INFINITY; 3];
                for vertex in &mesh.vertices {
                    for axis in 0..3 {
                        min[axis] = min[axis].min(vertex.pos[axis]);
                        max[axis] = max[axis].max(vertex.pos[axis]);
                    }
                }
                for axis in 0..3 {
                    assert!(max[axis] - min[axis] > 0.05, "{kind:?}/{variant} is flat");
                }
            }
        }

        // Silhouettes are intentionally different classes rather than one tetrahedron scaled into
        // four materials.
        assert_ne!(
            concrete_shard_mesh(0).vertices.len(),
            metal_shard_mesh(0).vertices.len()
        );
        assert_ne!(
            wood_shard_mesh(0).vertices.len(),
            glass_shard_mesh(2).vertices.len()
        );
    }

    #[test]
    fn debris_inherits_exact_registered_wall_material_before_surface_fallback() {
        let materials = MaterialRegistry::new();
        let wall = materials.register_named(
            "Tests/Wall/RedPaintedConcrete",
            MaterialDescriptor {
                base_color: [0.72, 0.08, 0.05, 1.0],
                roughness: 0.41,
                metallic: 0.0,
                ..MaterialDescriptor::default()
            },
        );
        assert_eq!(
            inherited_debris_material(&materials, wall.raw()),
            Some(wall)
        );
    }

    #[test]
    fn debris_material_inheritance_fails_closed_for_unknown_material_id() {
        let materials = MaterialRegistry::new();
        assert_eq!(inherited_debris_material(&materials, 0), None);
        assert_eq!(inherited_debris_material(&materials, 0x1234_5678), None);
    }
}
