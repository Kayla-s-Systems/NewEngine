use super::*;

use super::super::world_model::GroundPlacementSurface;

const FOLIAGE_GROUND_MIN_UP_DOT: f32 = 0.28;

#[inline]
fn ray_triangle_t(origin: Vec3, dir: Vec3, max_t: f32, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let edge1 = b - a;
    let edge2 = c - a;
    let p = dir.cross(edge2);
    let det = edge1.dot(p);
    if det.abs() <= 1.0e-7 {
        return None;
    }
    let inv_det = det.recip();
    let tvec = origin - a;
    let u = tvec.dot(p) * inv_det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = tvec.cross(edge1);
    let v = dir.dot(q) * inv_det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = edge2.dot(q) * inv_det;
    (t >= 0.0 && t <= max_t).then_some(t)
}

fn static_ground_height(world: &newengine_ecs::World, x: f32, z: f32) -> Option<f32> {
    let mut highest: Option<f32> = None;
    for (entity, _) in world.query::<GroundPlacementSurface>() {
        let Some(collider) =
            world.get::<newengine_engine_runtime::gameplay::StaticMeshCollider>(entity)
        else {
            continue;
        };
        let Some((position, rotation)) =
            newengine_transform::read_entity_world_pose_local_chain(world, entity)
        else {
            continue;
        };
        let rotation = rotation.normalize_or_identity();
        let world_from_local =
            newengine_math::Mat4::from_scale_rotation_translation(Vec3::ONE, rotation, position);
        let bounds = collider.local_bounds.transformed(world_from_local);
        if x < bounds.min.x - 0.05
            || x > bounds.max.x + 0.05
            || z < bounds.min.z - 0.05
            || z > bounds.max.z + 0.05
        {
            continue;
        }
        let origin_ws = Vec3::new(x, bounds.max.y + 2.0, z);
        let dir_ws = Vec3::new(0.0, -1.0, 0.0);
        let inv_rotation = rotation.conjugate();
        let origin_ls = inv_rotation * (origin_ws - position);
        let dir_ls = inv_rotation * dir_ws;
        let max_t = (bounds.max.y - bounds.min.y).abs().max(0.1) + 4.0;
        for triangle in collider.triangles.iter() {
            let [ax, ay, az] = collider.vertices[triangle[0] as usize];
            let [bx, by, bz] = collider.vertices[triangle[1] as usize];
            let [cx, cy, cz] = collider.vertices[triangle[2] as usize];
            let a = Vec3::new(ax, ay, az);
            let b = Vec3::new(bx, by, bz);
            let c = Vec3::new(cx, cy, cz);
            let normal_ls = (b - a).cross(c - a);
            let normal_len2 = normal_ls.length_squared();
            if normal_len2 <= 1.0e-10 {
                continue;
            }
            let normal_ws = rotation * (normal_ls / normal_len2.sqrt());
            if normal_ws.y.abs() < FOLIAGE_GROUND_MIN_UP_DOT {
                continue;
            }
            let Some(t) = ray_triangle_t(origin_ls, dir_ls, max_t, a, b, c) else {
                continue;
            };
            let hit_y = origin_ws.y - t;
            if hit_y.is_finite() {
                highest = Some(highest.map_or(hit_y, |current| current.max(hit_y)));
            }
        }
    }
    highest
}

#[inline]
fn has_static_ground_surfaces(world: &newengine_ecs::World) -> bool {
    world.query::<GroundPlacementSurface>().next().is_some()
}

pub(crate) fn terrain_height(
    world: &newengine_ecs::World,
    terrain: EntityId,
    x: f32,
    z: f32,
) -> f32 {
    let origin = world
        .get::<Transform>(terrain)
        .map(|t| t.position)
        .unwrap_or(Vec3::ZERO);
    world
        .get::<ProceduralTerrain>(terrain)
        .map(|t| {
            t.heightfield
                .sample_height_local(x - origin.x, z - origin.z)
                + origin.y
        })
        // A terrain entity without procedural sampling data may still be a valid
        // authored/static terrain anchor. Falling back to its world Y is safer
        // than silently forcing foliage to global Y=0.
        .unwrap_or(origin.y)
}

#[inline]
pub(super) fn hash_cell(seed: u64, x: i32, z: i32, salt: u64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64 ^ seed ^ salt;
    h = h.wrapping_mul(0x100_0000_01b3) ^ (x as i64 as u64);
    h = h.wrapping_mul(0x100_0000_01b3) ^ (z as i64 as u64);
    h ^ (h >> 32)
}

#[inline]
pub(super) fn unit_from_hash(h: u64) -> f32 {
    ((h >> 40) as u32 as f32) / ((1u32 << 24) as f32)
}

#[inline]
pub(super) fn choose_foliage_prefab<'a>(
    prefabs: &'a [GameReadyPrefabSpec],
    id: &str,
) -> Option<&'a GameReadyPrefabSpec> {
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    prefabs.iter().find(|p| p.enabled && p.id == id)
}

pub(super) fn effective_foliage_spec(spec: &GameReadyFoliageSpec) -> GameReadyFoliageSpec {
    let mut effective = spec.clone();
    let stress = crate::env_config::var("NEWENGINE_FOLIAGE_STRESS")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);
    if stress && effective.enabled {
        // Diagnostic-only density override. It keeps the authored terrain/prefab/material chain
        // intact while generating enough placements to exercise the real instance-buffer path.
        effective.grid_min = -64;
        effective.grid_max = 64;
        effective.spacing = 0.65;
        effective.jitter = 0.15;
        effective.gate_threshold = 1.0;
        effective.max_count = 4096;
        effective.min_player_distance = 0.0;
        effective.edge_margin = effective.edge_margin.min(0.5);
    }
    effective
}

pub(super) fn collect_tree_placements(
    world: &newengine_ecs::World,
    terrain: EntityId,
    terrain_surface: Option<&TerrainSurfaceSampler>,
    spec: &GameReadyFoliageSpec,
    player_start: Vec3,
) -> Vec<TreePlacement> {
    if !spec.enabled || spec.max_count == 0 {
        return Vec::new();
    }

    let terrain_data = world.get::<ProceduralTerrain>(terrain);
    let origin = terrain_surface
        .map(|surface| surface.origin)
        .or_else(|| {
            world
                .get::<Transform>(terrain)
                .map(|transform| transform.position)
        })
        .unwrap_or(Vec3::ZERO);

    // The direct sampler is the authoritative bootstrap seam. ECS components are
    // retained as a compatibility fallback for monolithic/static builds.
    let (terrain_half_x, terrain_half_z, domain_source) = if let Some(surface) = terrain_surface {
        let (half_x, half_z) = surface.half_extents_xz();
        (half_x, half_z, "bootstrap_sampler")
    } else if let Some(data) = terrain_data {
        let settings = data.heightfield.settings();
        (
            settings.size_x * 0.5,
            settings.size_z * 0.5,
            "heightfield_ecs",
        )
    } else if let Some(bounds) = world.get::<Bounds>(terrain) {
        let extents = bounds.local_aabb.half_extents();
        (extents.x.abs(), extents.z.abs(), "bounds_ecs")
    } else {
        newengine_ulog_api::ulog::warn!(
            "game-ready foliage placement rejected: terrain={:?} has no bootstrap sampler and no ECS terrain surface",
            terrain
        );
        return Vec::new();
    };

    let half_x = terrain_half_x - spec.edge_margin;
    let half_z = terrain_half_z - spec.edge_margin;
    if !half_x.is_finite() || !half_z.is_finite() || half_x <= 0.5 || half_z <= 0.5 {
        newengine_ulog_api::ulog::warn!(
            "game-ready foliage placement rejected: terrain={:?} domain={} half=({:.3},{:.3}) edge_margin={:.3} origin=({:.2},{:.2},{:.2})",
            terrain,
            domain_source,
            terrain_half_x,
            terrain_half_z,
            spec.edge_margin,
            origin.x,
            origin.y,
            origin.z
        );
        return Vec::new();
    }

    let min_player_dist2 = spec.min_player_distance * spec.min_player_distance;
    let density_gate = (spec.gate_threshold * spec.settings.density).clamp(0.0, 1.0);
    let mut placements = Vec::with_capacity(spec.max_count.min(512) as usize);
    let mut gate_rejected = 0u32;
    let mut edge_rejected = 0u32;
    let mut player_rejected = 0u32;
    let mut ground_rejected = 0u32;
    let static_ground = has_static_ground_surfaces(world);

    for gz in spec.grid_min..=spec.grid_max {
        for gx in spec.grid_min..=spec.grid_max {
            if placements.len() as u32 >= spec.max_count {
                newengine_ulog_api::ulog::info!(
                    "game-ready foliage placement domain: terrain={:?} domain={} origin=({:.2},{:.2},{:.2}) half=({:.2},{:.2}) placed={} gate_rejected={} edge_rejected={} player_rejected={} density_gate={:.3}",
                    terrain,
                    domain_source,
                    origin.x,
                    origin.y,
                    origin.z,
                    half_x,
                    half_z,
                    placements.len(),
                    gate_rejected,
                    edge_rejected,
                    player_rejected,
                    density_gate
                );
                return placements;
            }

            let gate = unit_from_hash(hash_cell(spec.seed, gx, gz, 0xa11c_e101));
            if gate > density_gate {
                gate_rejected = gate_rejected.saturating_add(1);
                continue;
            }

            let jx = (unit_from_hash(hash_cell(spec.seed, gx, gz, 0x41f0_0001)) * 2.0 - 1.0)
                * spec.spacing
                * spec.jitter;
            let jz = (unit_from_hash(hash_cell(spec.seed, gx, gz, 0x41f0_0002)) * 2.0 - 1.0)
                * spec.spacing
                * spec.jitter;
            let local_x = gx as f32 * spec.spacing + jx;
            let local_z = gz as f32 * spec.spacing + jz;
            if local_x.abs() > half_x || local_z.abs() > half_z {
                edge_rejected = edge_rejected.saturating_add(1);
                continue;
            }

            // Grid coordinates are terrain-local. Translate them into world space
            // before player exclusion and sampling. This matters as soon as the
            // active terrain chunk is centered anywhere other than world origin.
            let x = origin.x + local_x;
            let z = origin.z + local_z;
            let dx = x - player_start.x;
            let dz = z - player_start.z;
            if dx * dx + dz * dz < min_player_dist2 {
                player_rejected = player_rejected.saturating_add(1);
                continue;
            }

            let ground_y = if static_ground {
                static_ground_height(world, x, z)
            } else {
                terrain_surface
                    .map(|surface| surface.sample_world_height(x, z))
                    .or_else(|| {
                        terrain_data.map(|data| {
                            data.heightfield.sample_height_local(local_x, local_z) + origin.y
                        })
                    })
            };
            let Some(ground_y) = ground_y else {
                ground_rejected = ground_rejected.saturating_add(1);
                continue;
            };
            let y = ground_y + spec.surface_offset;
            let scale_t = unit_from_hash(hash_cell(spec.seed, gx, gz, 0x51ca_1e00));
            let scale = spec.min_scale + (spec.max_scale - spec.min_scale) * scale_t;
            let yaw =
                unit_from_hash(hash_cell(spec.seed, gx, gz, 0x7a77_0001)) * core::f32::consts::TAU;

            placements.push(TreePlacement {
                index: placements.len() as u32,
                position: Vec3::new(x, y, z),
                yaw,
                scale,
            });
        }
    }

    newengine_ulog_api::ulog::info!(
        "game-ready foliage placement domain: terrain={:?} domain={} origin=({:.2},{:.2},{:.2}) half=({:.2},{:.2}) placed={} gate_rejected={} edge_rejected={} player_rejected={} density_gate={:.3}",
        terrain,
        domain_source,
        origin.x,
        origin.y,
        origin.z,
        half_x,
        half_z,
        placements.len(),
        gate_rejected,
        edge_rejected,
        player_rejected,
        density_gate
    );
    newengine_ulog_api::ulog::info!(
        "game-ready foliage ground projection: mode='{}' placed={} ground_rejected={} surface_offset={:.3} policy='confirmed surface hit required; no origin-y fallback'",
        if static_ground { "static_collision" } else { "terrain_sampler" },
        placements.len(),
        ground_rejected,
        spec.surface_offset,
    );
    placements
}
