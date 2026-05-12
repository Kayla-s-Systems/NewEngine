
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

