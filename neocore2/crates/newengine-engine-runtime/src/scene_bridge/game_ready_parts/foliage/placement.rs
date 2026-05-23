fn terrain_height(world: &newengine_ecs::World, terrain: EntityId, x: f32, z: f32) -> f32 {
    let origin = world
        .get::<Transform>(terrain)
        .map(|t| t.position)
        .unwrap_or(Vec3::ZERO);
    world
        .get::<ProceduralTerrain>(terrain)
        .map(|t| t.heightfield.sample_height_local(x - origin.x, z - origin.z) + origin.y)
        .unwrap_or(0.0)
}

#[inline]
fn hash_cell(seed: u64, x: i32, z: i32, salt: u64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64 ^ seed ^ salt;
    h = h.wrapping_mul(0x100_0000_01b3) ^ (x as i64 as u64);
    h = h.wrapping_mul(0x100_0000_01b3) ^ (z as i64 as u64);
    h ^ (h >> 32)
}

#[inline]
fn unit_from_hash(h: u64) -> f32 {
    ((h >> 40) as u32 as f32) / ((1u32 << 24) as f32)
}

#[inline]
fn choose_foliage_prefab<'a>(
    prefabs: &'a [GameReadyPrefabSpec],
    id: &str,
) -> Option<&'a GameReadyPrefabSpec> {
    prefabs
        .iter()
        .find(|p| p.enabled && p.id == id)
        .or_else(|| prefabs.iter().find(|p| p.enabled && p.proxy == "ydd_runtime_mesh"))
}

fn collect_tree_placements(
    world: &newengine_ecs::World,
    terrain: EntityId,
    spec: &GameReadyFoliageSpec,
    player_start: Vec3,
) -> Vec<TreePlacement> {
    if !spec.enabled {
        return Vec::new();
    }

    let settings = {
        let Some(terrain_data) = world.get::<ProceduralTerrain>(terrain) else {
            return Vec::new();
        };
        terrain_data.heightfield.settings()
    };
    let half_x = settings.size_x * 0.5 - spec.edge_margin;
    let half_z = settings.size_z * 0.5 - spec.edge_margin;
    if half_x <= 0.5 || half_z <= 0.5 {
        return Vec::new();
    }

    let min_player_dist2 = spec.min_player_distance * spec.min_player_distance;
    let mut placements = Vec::with_capacity(spec.max_count.min(512) as usize);

    for gz in spec.grid_min..=spec.grid_max {
        for gx in spec.grid_min..=spec.grid_max {
            if placements.len() as u32 >= spec.max_count {
                return placements;
            }

            let gate = unit_from_hash(hash_cell(spec.seed, gx, gz, 0xa11c_e101));
            if gate > spec.gate_threshold {
                continue;
            }

            let jx = (unit_from_hash(hash_cell(spec.seed, gx, gz, 0x41f0_0001)) * 2.0 - 1.0)
                * spec.spacing
                * spec.jitter;
            let jz = (unit_from_hash(hash_cell(spec.seed, gx, gz, 0x41f0_0002)) * 2.0 - 1.0)
                * spec.spacing
                * spec.jitter;
            let x = gx as f32 * spec.spacing + jx;
            let z = gz as f32 * spec.spacing + jz;
            if x.abs() > half_x || z.abs() > half_z {
                continue;
            }

            let dx = x - player_start.x;
            let dz = z - player_start.z;
            if dx * dx + dz * dz < min_player_dist2 {
                continue;
            }

            let y = terrain_height(world, terrain, x, z) + spec.surface_offset;
            let scale_t = unit_from_hash(hash_cell(spec.seed, gx, gz, 0x51ca_1e00));
            let scale = spec.min_scale + (spec.max_scale - spec.min_scale) * scale_t;
            let yaw = unit_from_hash(hash_cell(spec.seed, gx, gz, 0x7a77_0001)) * core::f32::consts::TAU;

            placements.push(TreePlacement {
                index: placements.len() as u32,
                position: Vec3::new(x, y, z),
                yaw,
                scale,
            });
        }
    }

    placements
}