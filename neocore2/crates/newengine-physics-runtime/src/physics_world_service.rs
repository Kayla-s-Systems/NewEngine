use newengine_bounds::{Aabb, Bounds};
use newengine_ecs::{EntityId, World};
use newengine_math::{Mat4, Vec3};
use newengine_physics_contracts::{PhysicsBodyDesc, PhysicsBodyKind, PhysicsStepReport, PhysicsWorldDesc};
use newengine_procedural_noise::ProceduralTerrain;
use newengine_sim::Velocity;
use newengine_transform::{GlobalTransform, Transform};

use crate::fixed_step::FixedStepClock;

#[derive(Clone, Copy, Debug)]
pub struct PhysicsWorldStepSettings {
    pub gravity: f32,
    pub contact_skin: f32,
}

impl Default for PhysicsWorldStepSettings {
    #[inline]
    fn default() -> Self { Self { gravity: 9.81, contact_skin: 0.035 } }
}

#[derive(Clone, Debug)]
pub struct PhysicsWorldService {
    desc: PhysicsWorldDesc,
    fixed: FixedStepClock,
    last_report: PhysicsStepReport,
}

impl PhysicsWorldService {
    #[inline]
    pub fn new(desc: PhysicsWorldDesc) -> Self {
        let desc = desc.sanitized();
        Self { fixed: FixedStepClock::new(desc.fixed_dt, desc.max_substeps), desc, last_report: PhysicsStepReport::default() }
    }

    #[inline]
    pub fn desc(&self) -> PhysicsWorldDesc { self.desc }

    #[inline]
    pub fn last_report(&self) -> &PhysicsStepReport { &self.last_report }

    pub fn step(&mut self, world: &mut World, dt: f32, settings: PhysicsWorldStepSettings) -> PhysicsStepReport {
        let mut final_report = PhysicsStepReport::default();
        for (tick, fixed_dt) in self.fixed.drain(dt) {
            final_report = step_physics_world(world, fixed_dt, tick, settings);
        }
        self.last_report = final_report.clone();
        final_report
    }
}

#[inline]
fn translate_aabb(aabb: Aabb, delta: Vec3) -> Aabb {
    Aabb::new(aabb.min + delta, aabb.max + delta)
}

#[inline]
fn minimal_separation(a: &Aabb, b: &Aabb) -> Option<Vec3> {
    if !a.intersects(b) { return None; }
    let overlap_x = (a.max.x - b.min.x).min(b.max.x - a.min.x);
    let overlap_y = (a.max.y - b.min.y).min(b.max.y - a.min.y);
    let overlap_z = (a.max.z - b.min.z).min(b.max.z - a.min.z);
    let ac = a.center();
    let bc = b.center();
    if overlap_x <= overlap_y && overlap_x <= overlap_z {
        Some(Vec3::new(if ac.x < bc.x { -overlap_x } else { overlap_x }, 0.0, 0.0))
    } else if overlap_y <= overlap_z {
        Some(Vec3::new(0.0, if ac.y < bc.y { -overlap_y } else { overlap_y }, 0.0))
    } else {
        Some(Vec3::new(0.0, 0.0, if ac.z < bc.z { -overlap_z } else { overlap_z }))
    }
}

#[derive(Clone)]
struct RuntimeTerrainSurface {
    key: u64,
    terrain: ProceduralTerrain,
    world_from_local: Mat4,
    local_from_world: Mat4,
}

#[inline]
fn collect_runtime_terrain_surfaces(world: &World) -> Vec<RuntimeTerrainSurface> {
    let mut terrains: Vec<RuntimeTerrainSurface> = world
        .query2::<ProceduralTerrain, GlobalTransform>()
        .map(|(entity, terrain, gt)| RuntimeTerrainSurface {
            key: entity.stable_u64(),
            terrain: terrain.clone(),
            world_from_local: gt.0,
            local_from_world: gt.0.inverse(),
        })
        .collect();
    terrains.sort_by_key(|it| it.key);
    terrains
}

#[inline]
fn resolve_heightfield_contact(
    terrains: &[RuntimeTerrainSurface],
    body: PhysicsBodyDesc,
    next_pos: &mut Vec3,
    velocity: &mut Velocity,
    contact_skin: f32,
) {
    let local_aabb = body.shape.local_aabb();
    let contact_skin = contact_skin.clamp(0.0, 0.50);

    for surface in terrains {
        let local_pos = surface.local_from_world.transform_point3(*next_pos);
        let Some(local_ground_y) = surface
            .terrain
            .heightfield
            .sample_height_local_checked(local_pos.x, local_pos.z, 0.08)
        else {
            continue;
        };

        let world_ground = surface
            .world_from_local
            .transform_point3(Vec3::new(local_pos.x, local_ground_y, local_pos.z));
        let bottom_y = next_pos.y + local_aabb.min.y;
        let penetration = world_ground.y + contact_skin - bottom_y;
        if penetration <= 0.0 || !penetration.is_finite() { continue; }

        next_pos.y += penetration;
        if velocity.0.y < 0.0 { velocity.0.y = 0.0; }
    }
}

pub fn step_physics_world(
    world: &mut World,
    dt: f32,
    fixed_tick: u64,
    settings: PhysicsWorldStepSettings,
) -> PhysicsStepReport {
    let dt = dt.clamp(0.0001, 0.05);
    let gravity = settings.gravity.clamp(0.0, 80.0);
    let contact_skin = settings.contact_skin.clamp(0.0, 0.50);

    let mut static_colliders: Vec<(EntityId, Aabb)> = world
        .query2::<PhysicsBodyDesc, Bounds>()
        .filter_map(|(entity, body, bounds)| {
            if body.kind == PhysicsBodyKind::Static && !body.flags.is_trigger {
                Some((entity, bounds.world_aabb))
            } else {
                None
            }
        })
        .collect();
    static_colliders.sort_by_key(|it| it.0.stable_u64());

    let terrain_surfaces = collect_runtime_terrain_surfaces(world);

    let mut dynamic_ids: Vec<EntityId> = world
        .query::<PhysicsBodyDesc>()
        .filter_map(|(entity, body)| (body.kind == PhysicsBodyKind::Dynamic && !body.flags.is_trigger).then_some(entity))
        .collect();
    dynamic_ids.sort_by_key(|id| id.stable_u64());

    let mut report = PhysicsStepReport {
        fixed_tick,
        dt,
        substeps: 1,
        active_bodies: static_colliders.len() + dynamic_ids.len(),
        static_bodies: static_colliders.len(),
        dynamic_bodies: dynamic_ids.len(),
        contacts: 0,
        commands_applied: 0,
        events: Vec::new(),
    };

    for entity in dynamic_ids {
        let Some(body) = world.get::<PhysicsBodyDesc>(entity).copied() else { continue; };
        let Some(transform) = world.get::<Transform>(entity).copied() else { continue; };

        let mut velocity = world.get::<Velocity>(entity).copied().unwrap_or_default();
        velocity.0.y -= gravity * dt;

        let mut next_pos = transform.position;
        next_pos.y += velocity.0.y * dt;

        let local_aabb = body.shape.local_aabb();
        let mut world_aabb = translate_aabb(local_aabb, next_pos);

        for (other, static_aabb) in &static_colliders {
            if *other == entity { continue; }
            let Some(push) = minimal_separation(&world_aabb, static_aabb) else { continue; };
            next_pos = next_pos + push;
            world_aabb = translate_aabb(world_aabb, push);
            report.contacts += 1;
            if push.y != 0.0 { velocity.0.y = 0.0; }
            if push.x != 0.0 { velocity.0.x = 0.0; }
            if push.z != 0.0 { velocity.0.z = 0.0; }
        }

        resolve_heightfield_contact(&terrain_surfaces, body, &mut next_pos, &mut velocity, contact_skin);

        if let Some(t) = world.get_mut::<Transform>(entity) { t.position = next_pos; }
        let _ = world.insert(entity, velocity);
    }

    report
}
