#![forbid(unsafe_op_in_unsafe_fn)]

mod content;

use std::time::Duration;

use newengine_assets::{wait_ready, AssetAccess, AssetServiceClient};
use newengine_bounds::Bounds;
use newengine_ecs::EntityId;
use newengine_lighting::{AmbientLight, DirectionalLight, ShadowSettings};
use newengine_materials::{
    material_source_from_parts, parse_material_source_slice, MaterialDescriptor, MaterialFlags,
    MaterialId, MaterialRegistry, MaterialSourceDocument, MaterialTextureBindings,
};
use newengine_math::{EulerRot, Quat, Vec3};
use newengine_primitives::{
    builtins, fnv1a_64, Primitive, PrimitiveId, PrimitiveMesh, PrimitiveRegistry, PrimitiveVertex,
};
use newengine_procedural_noise::{
    NoiseAlgorithm, NoiseCombineMode, NoiseGraph2D, NoiseLayer2D, NoiseShape, ProceduralTerrain,
    TerrainCollisionTileSettings, TerrainHeightfieldDescriptor,
};
use newengine_plugin_host::default_host_api;
use newengine_scene::{spawn_named, Scene};
use newengine_transform::Transform;

use crate::gameplay::{
    ensure_collision_body, spawn_default_player_with_tuning, CollisionBody, CollisionShape,
    DisplayMode, DisplayVisibility, FpsDemoRules, FpsDemoState, FpsPlayerTuning,
    GameReadyWorldLaunchGate,
};
use crate::scene_bootstrap::bootstrap_runtime_scene;

use self::content::{
    load_game_ready_map_profile, GameReadyFoliageSpec, GameReadyGameplaySpec,
    GameReadyLightingSpec, GameReadyMaterialSetSpec, GameReadyMaterialSpec,
    GameReadyPaletteSpec, GameReadyPrefabSpec, GameReadySkySpec, GameReadyTerrainSpec,
};
use super::helpers::{
    apply_exact_material, apply_primitive_instance, ensure_primitive_base, ensure_root, primitive_bounds,
};

#[inline]
pub(super) fn game_ready_demo_enabled() -> bool {
    std::env::var("NEWENGINE_GAME_READY_DEMO")
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            !matches!(v.as_str(), "" | "0" | "false" | "off" | "no")
        })
        .unwrap_or(false)
}

#[derive(Clone, Copy)]
struct DemoMaterials {
    terrain: MaterialId,
    sky: MaterialId,
    tree_bark: MaterialId,
    tree_leaf: MaterialId,
    tree_branch: MaterialId,
}

#[derive(Clone, Copy)]
struct PrimitiveSpawnSpec<'a> {
    parent: EntityId,
    primitive_id: PrimitiveId,
    material_id: MaterialId,
    name: &'a str,
    position: Vec3,
    scale: Vec3,
    color: [f32; 4],
}

#[inline]
fn spawn_game_primitive(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    spec: PrimitiveSpawnSpec<'_>,
) -> EntityId {
    let entity = spawn_named(world, spec.name);
    let _ = newengine_transform::set_parent(world, entity, Some(spec.parent));
    let _ = world.insert(entity, Primitive { id: spec.primitive_id, color: spec.color });

    if let Some(bounds) = primitive_bounds(prims, spec.primitive_id) {
        let _ = world.insert(entity, bounds);
    }

    ensure_primitive_base(world, entity, spec.material_id);
    apply_primitive_instance(world, mats, entity, spec.material_id, spec.color);

    if let Some(t) = world.get_mut_tracked::<Transform>(entity) {
        t.position = spec.position;
        t.scale = spec.scale;
    }

    entity
}


#[inline]
fn load_material_source_asset(path: &str) -> Option<MaterialSourceDocument> {
    let assets = AssetServiceClient::new(default_host_api());
    let id = match assets.load(path) {
        Ok(id) => id,
        Err(e) => {
            log::warn!("game-ready: material asset unavailable path='{}' err='{}'", path, e);
            return None;
        }
    };

    if let Err(e) = wait_ready(&assets, &id, Duration::from_secs(2)) {
        log::warn!(
            "game-ready: material asset not ready path='{}' id='{}' err='{:?}'",
            path,
            id,
            e
        );
        return None;
    }

    let payload = match assets.blob_wire_v1(&id) {
        Ok((_meta, payload)) => payload,
        Err(e) => {
            log::warn!("game-ready: material asset read failed path='{}' err='{}'", path, e);
            return None;
        }
    };

    match parse_material_source_slice(&payload) {
        Ok(source) => Some(source),
        Err(e) => {
            log::warn!("game-ready: material asset parse failed path='{}' err='{}'", path, e);
            None
        }
    }
}

#[inline]
fn material_textures(spec: &GameReadyMaterialSpec) -> MaterialTextureBindings {
    MaterialTextureBindings {
        base_color_texture: spec.base_color_texture.clone(),
        normal_texture: spec.normal_texture.clone(),
        metallic_texture: None,
        roughness_texture: spec.roughness_texture.clone(),
        occlusion_texture: None,
        emissive_texture: None,
        uv_scale: spec.uv_scale,
        uv_offset: spec.uv_offset,
    }
}

#[inline]
fn register_material(
    mats: &MaterialRegistry,
    name: &str,
    base_color: [f32; 4],
    emissive: [f32; 3],
    emissive_strength: f32,
    flags: MaterialFlags,
    spec: &GameReadyMaterialSpec,
) -> MaterialId {
    if let Some(asset_path) = spec.asset.as_deref() {
        if let Some(source) = load_material_source_asset(asset_path) {
            let source = source.with_fallback_name(name.to_owned());
            let mut desc = source.desc;
            desc.flags = desc.flags.union(flags);
            desc.sanitize_in_place();
            let material_name = source.name.clone().unwrap_or_else(|| name.to_owned());
            return mats.upsert_named_with_textures(&material_name, desc, source.textures);
        }
    }

    let source = material_source_from_parts(
        name,
        MaterialDescriptor {
            base_color,
            emissive,
            emissive_strength,
            roughness: spec.roughness,
            normal_scale: spec.normal_scale,
            occlusion_strength: spec.occlusion_strength,
            flags,
            ..MaterialDescriptor::default()
        },
        material_textures(spec),
    );
    let material_name = source.name.clone().unwrap_or_else(|| name.to_owned());
    mats.upsert_named_with_textures(&material_name, source.desc, source.textures)
}

#[inline]
fn register_demo_materials(
    mats: &MaterialRegistry,
    palette: &GameReadyPaletteSpec,
    materials: &GameReadyMaterialSetSpec,
) -> DemoMaterials {
    DemoMaterials {
        terrain: register_material(
            mats,
            "FPS/ProceduralTerrain",
            palette.terrain,
            [0.0, 0.0, 0.0],
            1.0,
            MaterialFlags::CAST_SHADOWS,
            &materials.terrain,
        ),
        sky: register_material(
            mats,
            "FPS/SkyDome",
            palette.sky,
            palette.sky_emissive,
            2.6,
            MaterialFlags::DOUBLE_SIDED,
            &materials.sky,
        ),
        tree_bark: register_material(
            mats,
            "FPS/Tree/Bark",
            palette.tree_bark,
            [0.0, 0.0, 0.0],
            1.0,
            MaterialFlags::CAST_SHADOWS,
            &materials.tree_bark,
        ),
        tree_leaf: register_material(
            mats,
            "FPS/Tree/Leaf",
            palette.tree_leaf,
            [0.0, 0.0, 0.0],
            1.0,
            MaterialFlags::DOUBLE_SIDED
                .union(MaterialFlags::ALPHA_TEST)
                .union(MaterialFlags::CAST_SHADOWS),
            &materials.tree_leaf,
        ),
        tree_branch: register_material(
            mats,
            "FPS/Tree/Branch",
            palette.tree_branch,
            [0.0, 0.0, 0.0],
            1.0,
            MaterialFlags::DOUBLE_SIDED.union(MaterialFlags::CAST_SHADOWS),
            &materials.tree_branch,
        ),
    }
}

#[inline]
fn configure_game_ready_lighting(world: &mut newengine_ecs::World, spec: &GameReadyLightingSpec) {
    let ambient = AmbientLight {
        color: spec.ambient_color,
        intensity: spec.ambient_intensity,
    };
    match world.resource_mut::<AmbientLight>() {
        Some(a) => *a = ambient,
        None => world.insert_resource(ambient),
    }

    let sun_dir = Vec3::new(
        spec.sun_direction[0],
        spec.sun_direction[1],
        spec.sun_direction[2],
    )
    .normalize_or_zero();
    let sun = DirectionalLight {
        direction_ws: [sun_dir.x, sun_dir.y, sun_dir.z],
        color: spec.sun_color,
        intensity: spec.sun_intensity,
    };
    let sun_entity = world.query::<DirectionalLight>().next().map(|(entity, _)| entity);
    if let Some(sun_entity) = sun_entity {
        if let Some(light) = world.get_mut_tracked::<DirectionalLight>(sun_entity) {
            *light = sun;
        }
    }

    world.insert_resource(ShadowSettings {
        enabled: spec.shadows.enabled,
        method: newengine_lighting::ShadowMethod::DirectionalDepthMap,
        resolution: spec.shadows.resolution,
        cascade_count: spec.shadows.cascade_count,
        max_distance: spec.shadows.max_distance,
        softness: spec.shadows.softness,
        bias: spec.shadows.bias,
        normal_bias: spec.shadows.normal_bias,
        contact_strength: spec.shadows.contact_strength,
    });
}

fn spawn_procedural_terrain(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    root: EntityId,
    material: MaterialId,
    spec: &GameReadyTerrainSpec,
    color: [f32; 4],
) -> EntityId {
    log::info!(
        "game-ready: terrain generator id='{}' seed={} cells={}x{} size={}x{}",
        spec.generator.id,
        spec.seed,
        spec.cells_x,
        spec.cells_z,
        spec.size_x,
        spec.size_z,
    );

    let terrain_graph = NoiseGraph2D::soft_cells(spec.seed)
        .with_layer(
            NoiseLayer2D::new(NoiseAlgorithm::Ridged)
                .seed_offset(spec.seed ^ spec.generator.ridged_seed_xor)
                .frequency(spec.generator.ridged_frequency)
                .amplitude(spec.generator.ridged_amplitude)
                .shape(NoiseShape::SmoothStep {
                    edge0: spec.generator.ridged_shape_edge0,
                    edge1: spec.generator.ridged_shape_edge1,
                })
                .combine(NoiseCombineMode::Add),
        )
        .with_layer(
            NoiseLayer2D::new(NoiseAlgorithm::Veins)
                .seed_offset(spec.seed ^ spec.generator.veins_seed_xor)
                .frequency(spec.generator.veins_frequency)
                .amplitude(spec.generator.veins_amplitude)
                .combine(NoiseCombineMode::Add),
        );

    let terrain = ProceduralTerrain::generate_descriptor(
        TerrainHeightfieldDescriptor {
            cells_x: spec.cells_x,
            cells_z: spec.cells_z,
            size_x: spec.size_x,
            size_z: spec.size_z,
            base_height: spec.base_height,
            height_scale: spec.height_scale,
            graph: terrain_graph,
            smoothing_passes: spec.generator.smoothing_passes,
            smoothing_strength: spec.generator.smoothing_strength,
        },
        color,
    );

    let bounds = Bounds::from_local_aabb(terrain.heightfield.local_bounds());
    let entity = spawn_named(world, "Terrain/HeightField-Procedural");
    let _ = newengine_transform::set_parent(world, entity, Some(root));
    let _ = world.insert(entity, Transform::default());
    let _ = world.insert(entity, terrain);
    let _ = world.insert(entity, bounds);
    let _ = apply_exact_material(world, mats, entity, material, material, color);
    entity
}

fn spawn_terrain_collision_tiles(
    world: &mut newengine_ecs::World,
    root: EntityId,
    terrain_entity: EntityId,
    spec: &GameReadyTerrainSpec,
) {
    let Some(terrain) = world.get::<ProceduralTerrain>(terrain_entity).cloned() else {
        return;
    };

    for (i, tile) in terrain
        .heightfield
        .collision_tiles(TerrainCollisionTileSettings {
            tile_cells: spec.collision_tile_cells,
            floor_depth: spec.collision_floor_depth,
            horizontal_skin: spec.collision_horizontal_skin,
        })
        .into_iter()
        .enumerate()
    {
        let entity = spawn_named(world, format!("Terrain/CollisionTile-{i:03}"));
        let _ = newengine_transform::set_parent(world, entity, Some(root));
        let _ = world.insert(
            entity,
            Transform {
                position: tile.center,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        );
        let _ = world.insert(entity, DisplayVisibility { mode: DisplayMode::EditorOnly });
        ensure_collision_body(
            world,
            entity,
            CollisionBody {
                shape: CollisionShape::Box {
                    half_extents: [tile.half_extents.x, tile.half_extents.y, tile.half_extents.z],
                },
                dynamic: false,
                is_trigger: true,
            },
        );
    }
}

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

#[inline]
fn read_u32_le(payload: &[u8], offset: &mut usize) -> Result<u32, String> {
    let end = offset.saturating_add(4);
    let bytes = payload
        .get(*offset..end)
        .ok_or_else(|| "NE3D payload truncated while reading u32".to_owned())?;
    *offset = end;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[inline]
fn read_f32_le(payload: &[u8], offset: &mut usize) -> Result<f32, String> {
    Ok(f32::from_bits(read_u32_le(payload, offset)?))
}

fn decode_ne3d_mesh(payload: &[u8]) -> Result<PrimitiveMesh, String> {
    if payload.len() < 20 || payload.get(0..4) != Some(b"NE3D") {
        return Err("invalid NE3D header".to_owned());
    }

    let mut offset = 4usize;
    let version = read_u32_le(payload, &mut offset)?;
    if version != 1 {
        return Err(format!("unsupported NE3D version={version}"));
    }

    let vertex_count = read_u32_le(payload, &mut offset)? as usize;
    let index_count = read_u32_le(payload, &mut offset)? as usize;
    let flags = read_u32_le(payload, &mut offset)?;

    if vertex_count == 0 || index_count == 0 {
        return Err(format!(
            "empty NE3D mesh vertices={vertex_count} indices={index_count}"
        ));
    }
    if vertex_count > 1_000_000 || index_count > 6_000_000 {
        return Err(format!(
            "NE3D mesh exceeds runtime limits vertices={vertex_count} indices={index_count}"
        ));
    }

    let has_normals = (flags & 0x1) != 0;
    let has_uvs = (flags & 0x2) != 0;

    let mut positions = Vec::with_capacity(vertex_count);
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);

    for _ in 0..vertex_count {
        let pos = [
            read_f32_le(payload, &mut offset)?,
            read_f32_le(payload, &mut offset)?,
            read_f32_le(payload, &mut offset)?,
        ];
        min.x = min.x.min(pos[0]);
        min.y = min.y.min(pos[1]);
        min.z = min.z.min(pos[2]);
        max.x = max.x.max(pos[0]);
        max.y = max.y.max(pos[1]);
        max.z = max.z.max(pos[2]);
        positions.push(pos);
    }

    let mut normals = Vec::with_capacity(vertex_count);
    if has_normals {
        for _ in 0..vertex_count {
            normals.push([
                read_f32_le(payload, &mut offset)?,
                read_f32_le(payload, &mut offset)?,
                read_f32_le(payload, &mut offset)?,
            ]);
        }
    } else {
        normals.resize(vertex_count, [0.0, 1.0, 0.0]);
    }

    let mut uvs = Vec::with_capacity(vertex_count);
    if has_uvs {
        for _ in 0..vertex_count {
            uvs.push([
                read_f32_le(payload, &mut offset)?,
                read_f32_le(payload, &mut offset)?,
            ]);
        }
    } else {
        uvs.resize(vertex_count, [0.0, 0.0]);
    }

    let mut indices = Vec::with_capacity(index_count);
    for _ in 0..index_count {
        let index = read_u32_le(payload, &mut offset)?;
        if index as usize >= vertex_count {
            return Err(format!(
                "NE3D index out of bounds index={index} vertex_count={vertex_count}"
            ));
        }
        indices.push(index);
    }

    let vertices = positions
        .into_iter()
        .zip(normals)
        .zip(uvs)
        .map(|((pos, nrm), uv)| PrimitiveVertex { pos, nrm, uv })
        .collect::<Vec<_>>();

    let bounds_center = (min + max) * 0.5;
    let mut bounds_radius = 0.0f32;
    for v in &vertices {
        let p = Vec3::new(v.pos[0], v.pos[1], v.pos[2]);
        bounds_radius = bounds_radius.max((p - bounds_center).length());
    }

    Ok(PrimitiveMesh {
        vertices,
        indices,
        bounds_center,
        bounds_radius: bounds_radius.max(0.001),
    })
}

fn load_ne3d_mesh_asset(logical_path: &str) -> Result<PrimitiveMesh, String> {
    let assets = AssetServiceClient::new(default_host_api());
    let id = assets
        .load(logical_path)
        .map_err(|e| format!("asset.load failed path='{logical_path}' err='{e}'"))?;

    wait_ready(&assets, &id, Duration::from_secs(3))
        .map_err(|e| format!("asset not ready path='{logical_path}' id='{id}' err='{e:?}'"))?;

    let (meta, payload) = assets
        .blob_wire_v1(&id)
        .map_err(|e| format!("asset.blob_wire_v1 failed path='{logical_path}' id='{id}' err='{e}'"))?;

    if !meta.contains("kalitech.model3d.meta.v1") {
        log::warn!(
            "game-ready: geometry asset meta is not model3d schema path='{}' meta='{}'",
            logical_path,
            meta
        );
    }

    decode_ne3d_mesh(&payload)
}

fn ensure_skydome_primitive(prims: &mut PrimitiveRegistry, logical_path: &str) -> Option<PrimitiveId> {
    if prims.is_registered(SKYDOME_PRIMITIVE_ID) {
        return Some(SKYDOME_PRIMITIVE_ID);
    }

    match load_ne3d_mesh_asset(logical_path) {
        Ok(mesh) => {
            let vertex_count = mesh.vertices.len();
            let index_count = mesh.indices.len();
            prims.register_mesh(
                SKYDOME_PRIMITIVE_ID,
                format!("Imported/SkyDome ({logical_path})"),
                mesh,
            );
            log::info!(
                "game-ready: skydome imported through AssetManager/geometryImporter path='{}' vertices={} indices={}",
                logical_path,
                vertex_count,
                index_count
            );
            Some(SKYDOME_PRIMITIVE_ID)
        }
        Err(e) => {
            log::error!(
                "game-ready: skydome import failed through required AssetManager/geometryImporter path='{}' err='{}'",
                logical_path,
                e
            );
            None
        }
    }
}

fn spawn_skydome(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    materials: DemoMaterials,
    root: EntityId,
    spec: &GameReadySkySpec,
    color: [f32; 4],
) {
    let Some(primitive_id) = ensure_skydome_primitive(prims, &spec.mesh) else {
        return;
    };

    let sky = spawn_game_primitive(
        world,
        &*prims,
        mats,
        PrimitiveSpawnSpec {
            parent: root,
            primitive_id,
            material_id: materials.sky,
            name: "Sky/Imported-SkyDome",
            position: Vec3::ZERO,
            scale: Vec3::splat(spec.radius),
            color,
        },
    );
    let _ = apply_exact_material(world, mats, sky, materials.sky, materials.sky, color);
}

fn to_fps_demo_rules(spec: &GameReadyGameplaySpec) -> FpsDemoRules {
    FpsDemoRules {
        default_status: spec.default_status.clone(),
        pickup_status: spec.pickup_status.clone(),
        hazard_status: spec.hazard_status.clone(),
        goal_locked_status: spec.goal_locked_status.clone(),
        goal_complete_status: spec.goal_complete_status.clone(),
        failed_progress_label: spec.failed_progress_label.clone(),
        completed_progress_label: spec.completed_progress_label.clone(),
        player: FpsPlayerTuning {
            body_radius: spec.player_collision.radius,
            body_half_height: spec.player_collision.half_height,
            visual_radius: spec.player_visual.radius,
            visual_half_height: spec.player_visual.half_height,
            camera_eye_height: spec.player_visual.camera_eye_height,
            sprint_multiplier: spec.player_visual.sprint_multiplier,
            gravity: spec.physics.gravity,
            contact_skin: spec.physics.contact_skin,
        }
        .sanitized(),
    }
}

pub(super) fn bootstrap_fps_game_ready_scene(
    scene: &mut Scene,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
) -> Option<EntityId> {
    *scene = Scene::new();
    bootstrap_runtime_scene(scene);

    let root = ensure_root(scene);
    let active_camera = scene.active_camera();
    let map = load_game_ready_map_profile();
    let materials = register_demo_materials(mats, &map.palette, &map.materials);
    let world = scene.world_mut();

    let rules = to_fps_demo_rules(&map.gameplay);
    world.insert_resource(rules.clone());
    world.insert_resource(FpsDemoState::from_rules(
        0,
        map.title.clone(),
        map.objective.clone(),
        &rules,
    ));
    world.insert_resource(GameReadyWorldLaunchGate::new(
        "waiting for CPU scene assembly and GPU material residency",
    ));

    configure_game_ready_lighting(world, &map.lighting);

    let terrain = spawn_procedural_terrain(world, mats, root, materials.terrain, &map.terrain, map.palette.terrain);
    spawn_terrain_collision_tiles(world, root, terrain, &map.terrain);
    spawn_foliage_prefabs(
        world,
        &*prims,
        mats,
        root,
        terrain,
        materials,
        &map.palette,
        &map.foliage,
        &map.prefabs,
        map.player.start,
    );
    spawn_skydome(world, prims, mats, materials, root, &map.sky, map.palette.sky);

    let start_x = map.player.start.x;
    let start_z = map.player.start.z;
    let player_tuning = rules.player.sanitized();
    let start_y = terrain_height(world, terrain, start_x, start_z)
        + player_tuning.body_half_height
        + player_tuning.body_radius
        + player_tuning.contact_skin;
    let player = spawn_default_player_with_tuning(
        world,
        Some(root),
        "Player/FPS",
        Vec3::new(start_x, start_y, start_z),
        player_tuning,
    );
    let _ = world.insert(player, DisplayVisibility { mode: DisplayMode::EditorOnly });
    if let Some(motor) = world.get_mut::<newengine_sim::CharacterMotor>(player) {
        motor.move_speed = map.player.move_speed;
        motor.look_sens = map.player.look_sens;
        motor.yaw = map.player.yaw;
    }
    if let Some(t) = world.get_mut_tracked::<Transform>(player) {
        t.rotation = Quat::from_euler(EulerRot::YXZ, map.player.yaw, 0.0, 0.0);
    }

    if let Some(cam) = active_camera {
        if let Some(t) = world.get_mut_tracked::<Transform>(cam) {
            t.position = Vec3::new(start_x, start_y + player_tuning.camera_eye_height, start_z);
            t.rotation = Quat::from_euler(EulerRot::YXZ, map.player.yaw, 0.0, 0.0);
        }
    }

    let _ = scene.validate_invariants();
    Some(player)
}
