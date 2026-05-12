fn terrain_height(world: &newengine_ecs::World, terrain: EntityId, x: f32, z: f32) -> f32 {
    world
        .get::<ProceduralTerrain>(terrain)
        .map(|t| t.heightfield.sample_height_local(x, z))
        .unwrap_or(0.0)
}

#[derive(Clone, Copy, Debug)]
struct TreePlacement {
    index: u32,
    position: Vec3,
    yaw: f32,
    scale: f32,
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
        .or_else(|| prefabs.iter().find(|p| p.enabled && p.proxy == "primitive_tree_cluster"))
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

fn spawn_tree_part(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    parent: EntityId,
    name: String,
    primitive_id: PrimitiveId,
    material_id: MaterialId,
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
    color: [f32; 4],
) -> EntityId {
    let entity = spawn_game_primitive(
        world,
        prims,
        mats,
        PrimitiveSpawnSpec {
            parent,
            primitive_id,
            material_id,
            name: &name,
            position,
            scale,
            color,
        },
    );
    if let Some(t) = world.get_mut_tracked::<Transform>(entity) {
        t.rotation = rotation;
    }
    entity
}

fn spawn_tree_proxy_prefab(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    root: EntityId,
    materials: DemoMaterials,
    palette: &GameReadyPaletteSpec,
    placement: TreePlacement,
) {
    let base = placement.position;
    let yaw = Quat::from_rotation_y(placement.yaw);
    let s = placement.scale;
    let trunk_height = 2.65 * s;
    let trunk_radius = 0.34 * s;

    let trunk = spawn_tree_part(
        world,
        prims,
        mats,
        root,
        format!("Foliage/TreeAnimate-{}/Trunk", placement.index),
        builtins::ID_CYLINDER,
        materials.tree_bark,
        base + Vec3::new(0.0, trunk_height * 0.5, 0.0),
        yaw,
        Vec3::new(trunk_radius, trunk_height, trunk_radius),
        palette.tree_bark,
    );
    ensure_collision_body(
        world,
        trunk,
        CollisionBody {
            shape: CollisionShape::Box {
                half_extents: [trunk_radius * 0.55, trunk_height * 0.50, trunk_radius * 0.55],
            },
            dynamic: false,
            is_trigger: false,
        },
    );

    let branch_color = palette.tree_branch;
    let branch_height = 1.55 * s;
    let branch_radius = 0.16 * s;
    for side in 0..3 {
        let spin = placement.yaw + side as f32 * (core::f32::consts::TAU / 3.0);
        let rot = Quat::from_rotation_y(spin) * Quat::from_rotation_z(0.86);
        let out = Quat::from_rotation_y(spin) * Vec3::new(0.0, 0.0, 0.62 * s);
        spawn_tree_part(
            world,
            prims,
            mats,
            root,
            format!("Foliage/TreeAnimate-{}/Branch-{side}", placement.index),
            builtins::ID_CYLINDER,
            materials.tree_branch,
            base + Vec3::new(0.0, trunk_height * (0.55 + side as f32 * 0.08), 0.0) + out,
            rot,
            Vec3::new(branch_radius, branch_height, branch_radius),
            branch_color,
        );
    }

    let leaf_color = palette.tree_leaf;
    let crown_base = base + Vec3::new(0.0, trunk_height + 0.42 * s, 0.0);
    let crowns = [
        (Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.20, 1.55, 2.20)),
        (Vec3::new(0.88, 0.16, 0.35), Vec3::new(1.45, 1.05, 1.45)),
        (Vec3::new(-0.82, 0.08, -0.42), Vec3::new(1.35, 0.98, 1.35)),
    ];
    for (i, (offset, scale)) in crowns.into_iter().enumerate() {
        let offset = yaw * (offset * s);
        spawn_tree_part(
            world,
            prims,
            mats,
            root,
            format!("Foliage/TreeAnimate-{}/Crown-{i}", placement.index),
            builtins::ID_SPHERE_UV,
            materials.tree_leaf,
            crown_base + offset,
            yaw,
            scale * s,
            leaf_color,
        );
    }
}

fn spawn_foliage_prefabs(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    root: EntityId,
    terrain: EntityId,
    materials: DemoMaterials,
    palette: &GameReadyPaletteSpec,
    foliage: &GameReadyFoliageSpec,
    prefabs: &[GameReadyPrefabSpec],
    player_start: Vec3,
) {
    let Some(prefab) = choose_foliage_prefab(prefabs, &foliage.prefab) else {
        if foliage.enabled {
            log::warn!(
                "game-ready: foliage enabled but prefab id='{}' is not declared or disabled",
                foliage.prefab
            );
        }
        return;
    };

    if prefab.proxy != "primitive_tree_cluster" {
        log::warn!(
            "game-ready: prefab id='{}' proxy='{}' is unsupported by standalone runtime; skipping",
            prefab.id,
            prefab.proxy
        );
        return;
    }

    let placements = collect_tree_placements(world, terrain, foliage, player_start);
    let count = placements.len();
    for placement in placements {
        spawn_tree_proxy_prefab(world, prims, mats, root, materials, palette, placement);
    }

    log::info!(
        "game-ready: foliage prefab placement prefab='{}' source='{}' proxy='{}' placed={} max_count={} grid={}..{} spacing={:.2}",
        prefab.id,
        prefab.source,
        prefab.proxy,
        count,
        foliage.max_count,
        foliage.grid_min,
        foliage.grid_max,
        foliage.spacing,
    );
}

const SKYDOME_PRIMITIVE_ID: PrimitiveId = PrimitiveId(fnv1a_64("kalitech.asset.skydome.high.v1"));
