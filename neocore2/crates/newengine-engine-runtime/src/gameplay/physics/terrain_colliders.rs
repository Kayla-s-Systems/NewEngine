use newengine_ecs::World;
use newengine_math::{Mat4, Vec3};
use newengine_physics_api::{
    CollisionShapeDto, HeightfieldColliderDto, PhysicsBodyFlagsDto, PhysicsBodyKindDto,
    PhysicsColliderDto, PhysicsFrameBodySnapshot, PhysicsFrameColliderSnapshot, PhysicsMaterialDto,
};
use newengine_procedural_noise::ProceduralTerrain;
use newengine_transform::{GlobalTransform, Transform};

use super::util::{arr_to_vec3, vec3_to_arr};

const TERRAIN_COLLIDER_SAMPLE_COUNT: usize = 17;
const TERRAIN_COLLIDER_MIN_HALF_EXTENT: f32 = 8.0;
const TERRAIN_COLLIDER_MAX_HALF_EXTENT: f32 = 24.0;
const TERRAIN_COLLIDER_TILE_SIZE: f32 = 4.0;

#[derive(Clone)]
struct RuntimeTerrainSurface {
    key: u64,
    terrain: ProceduralTerrain,
    world_from_local: Mat4,
    local_from_world: Mat4,
}

pub(super) fn collect_terrain_colliders(
    world: &World,
    bodies: &[PhysicsFrameBodySnapshot],
    contact_skin: f32,
) -> Vec<PhysicsFrameColliderSnapshot> {
    let terrains = collect_runtime_terrain_surfaces(world);
    if terrains.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let dynamic_bodies = bodies
        .iter()
        .copied()
        .filter(|body| body.kind == PhysicsBodyKindDto::Dynamic && !body.flags.is_trigger);

    for body in dynamic_bodies {
        for terrain in &terrains {
            if let Some(collider) =
                build_body_local_heightfield_collider(&body, terrain, contact_skin)
            {
                out.push(collider);
            }
        }
    }
    out
}

fn collect_runtime_terrain_surfaces(world: &World) -> Vec<RuntimeTerrainSurface> {
    let mut terrains = Vec::new();
    for (entity, terrain) in world.query::<ProceduralTerrain>() {
        let world_from_local = world
            .get::<GlobalTransform>(entity)
            .map(|gt| gt.0)
            .or_else(|| world.get::<Transform>(entity).map(|t| t.to_mat4()))
            .unwrap_or(Mat4::IDENTITY);
        terrains.push(RuntimeTerrainSurface {
            key: entity.stable_u64(),
            terrain: terrain.clone(),
            world_from_local,
            local_from_world: world_from_local.inverse(),
        });
    }
    terrains.sort_by_key(|terrain| terrain.key);
    terrains
}

fn build_body_local_heightfield_collider(
    body: &PhysicsFrameBodySnapshot,
    terrain: &RuntimeTerrainSurface,
    contact_skin: f32,
) -> Option<PhysicsFrameColliderSnapshot> {
    let body_pos = arr_to_vec3(body.position);
    let footprint = terrain_body_footprint(body);
    let half_extent = (footprint * 6.0)
        .max(TERRAIN_COLLIDER_MIN_HALF_EXTENT)
        .min(TERRAIN_COLLIDER_MAX_HALF_EXTENT);
    let tile_center = quantized_xz(body_pos, TERRAIN_COLLIDER_TILE_SIZE);
    let spacing = (half_extent * 2.0) / (TERRAIN_COLLIDER_SAMPLE_COUNT as f32 - 1.0);
    let local_origin = Vec3::new(
        tile_center.x - half_extent,
        0.0,
        tile_center.z - half_extent,
    );

    let mut heights =
        Vec::with_capacity(TERRAIN_COLLIDER_SAMPLE_COUNT * TERRAIN_COLLIDER_SAMPLE_COUNT);
    let mut min_height = f32::INFINITY;
    let mut max_height = f32::NEG_INFINITY;

    for z in 0..TERRAIN_COLLIDER_SAMPLE_COUNT {
        for x in 0..TERRAIN_COLLIDER_SAMPLE_COUNT {
            let world_x = local_origin.x + x as f32 * spacing;
            let world_z = local_origin.z + z as f32 * spacing;
            let local_sample = terrain
                .local_from_world
                .transform_point3(Vec3::new(world_x, body_pos.y, world_z));
            let h_local = terrain.terrain.heightfield.sample_height_local_checked(
                local_sample.x,
                local_sample.z,
                half_extent + contact_skin + 0.5,
            )?;
            let world_ground = terrain.world_from_local.transform_point3(Vec3::new(
                local_sample.x,
                h_local,
                local_sample.z,
            ));
            let height = world_ground.y;
            min_height = min_height.min(height);
            max_height = max_height.max(height);
            heights.push(height);
        }
    }

    if !min_height.is_finite() || !max_height.is_finite() {
        return None;
    }

    let bounds_min = Vec3::new(
        local_origin.x,
        min_height - contact_skin.max(0.0),
        local_origin.z,
    );
    let bounds_max = Vec3::new(
        local_origin.x + spacing * (TERRAIN_COLLIDER_SAMPLE_COUNT as f32 - 1.0),
        max_height + contact_skin.max(0.0),
        local_origin.z + spacing * (TERRAIN_COLLIDER_SAMPLE_COUNT as f32 - 1.0),
    );

    Some(PhysicsFrameColliderSnapshot {
        entity: terrain_heightfield_entity_key(body.entity, terrain.key, tile_center),
        collider: PhysicsColliderDto::Heightfield(HeightfieldColliderDto {
            sample_count_x: TERRAIN_COLLIDER_SAMPLE_COUNT as u32,
            sample_count_z: TERRAIN_COLLIDER_SAMPLE_COUNT as u32,
            spacing: [spacing, spacing],
            local_origin: vec3_to_arr(local_origin),
            heights,
            min_height,
            max_height,
        }),
        flags: PhysicsBodyFlagsDto {
            is_trigger: false,
            participates_in_queries: false,
            casts_contacts: true,
        },
        material: PhysicsMaterialDto {
            friction: 0.85,
            restitution: 0.0,
            density: 1.0,
        },
        position: [0.0, 0.0, 0.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        bounds_min: vec3_to_arr(bounds_min),
        bounds_max: vec3_to_arr(bounds_max),
    })
}

fn terrain_body_footprint(body: &PhysicsFrameBodySnapshot) -> f32 {
    let shape_radius = match body.shape {
        CollisionShapeDto::Box { half_extents } => half_extents[0].abs().max(half_extents[2].abs()),
        CollisionShapeDto::Sphere { radius } => radius.abs(),
        CollisionShapeDto::Capsule { radius, .. } => radius.abs(),
    };
    let bounds_radius = ((body.bounds_max[0] - body.bounds_min[0])
        .abs()
        .max((body.bounds_max[2] - body.bounds_min[2]).abs()))
        * 0.5;
    shape_radius.max(bounds_radius).max(0.35).min(4.0) + 0.25
}

fn quantized_xz(pos: Vec3, tile_size: f32) -> Vec3 {
    let tile_size = tile_size.max(0.25);
    Vec3::new(
        (pos.x / tile_size).round() * tile_size,
        0.0,
        (pos.z / tile_size).round() * tile_size,
    )
}

fn terrain_heightfield_entity_key(
    dynamic_entity: u64,
    terrain_entity: u64,
    tile_center: Vec3,
) -> u64 {
    let tile_x = (tile_center.x * 8.0).round() as i64 as u64;
    let tile_z = (tile_center.z * 8.0).round() as i64 as u64;
    let mut h = 0x9e37_79b9_7f4a_7c15_u64;
    h ^= dynamic_entity
        .wrapping_add(0xbf58_476d_1ce4_e5b9)
        .rotate_left(17);
    h = h.wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^= terrain_entity
        .wrapping_add(0x2545_f491_4f6c_dd1d)
        .rotate_left(29);
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^= tile_x.rotate_left(7) ^ tile_z.rotate_left(39);
    0x8000_0000_0000_0000 | (h & 0x7fff_ffff_ffff_ffff)
}
