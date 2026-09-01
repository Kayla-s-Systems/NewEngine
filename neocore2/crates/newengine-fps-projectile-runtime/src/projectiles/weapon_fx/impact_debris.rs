const IMPACT_DEBRIS_ACTIVE_BUDGET: usize = 256;
const IMPACT_DEBRIS_SETTLE_LINEAR_SPEED_MPS: f32 = 0.07;
const IMPACT_DEBRIS_SETTLE_ANGULAR_SPEED_RADPS: f32 = 1.5;
const IMPACT_DEBRIS_SETTLE_HOLD_SECONDS: f32 = 0.32;

#[derive(Clone, Copy, Debug)]
struct BulletImpactDebrisProfile {
    kind: PersistentImpactDebrisKind,
    count: usize,
    half_extents: Vec3,
    speed_min: f32,
    speed_max: f32,
    tangent_spread: f32,
    friction: f32,
    restitution: f32,
    density: f32,
    angular_speed: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct ActiveImpactDebris;

#[derive(Clone, Copy, Debug, Default)]
struct ImpactDebrisContactRuntime {
    contact_cooldown_seconds: f32,
    quiet_seconds: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct ImpactDebrisPhysicsEventCursor {
    fixed_tick: u64,
}

#[inline]
fn impact_debris_kind(surface: Option<&str>) -> Option<PersistentImpactDebrisKind> {
    let surface = surface.unwrap_or_default().trim().to_ascii_lowercase();
    if surface.contains("glass") {
        Some(PersistentImpactDebrisKind::Glass)
    } else if surface.contains("metal") || surface.contains("steel") || surface.contains("vehicle")
    {
        Some(PersistentImpactDebrisKind::Metal)
    } else if surface.contains("wood") || surface.contains("timber") || surface.contains("paper") {
        Some(PersistentImpactDebrisKind::Wood)
    } else if surface.contains("dirt")
        || surface.contains("sand")
        || surface.contains("soil")
        || surface.contains("snow")
    {
        None
    } else {
        Some(PersistentImpactDebrisKind::Concrete)
    }
}

#[inline]
fn impact_debris_profile(kind: PersistentImpactDebrisKind) -> BulletImpactDebrisProfile {
    match kind {
        PersistentImpactDebrisKind::Concrete => BulletImpactDebrisProfile {
            kind,
            count: 8,
            half_extents: Vec3::new(0.013, 0.009, 0.019),
            speed_min: 1.2,
            speed_max: 5.8,
            tangent_spread: 0.72,
            friction: 0.78,
            restitution: 0.16,
            density: 1.65,
            angular_speed: 13.0,
        },
        PersistentImpactDebrisKind::Metal => BulletImpactDebrisProfile {
            kind,
            count: 6,
            half_extents: Vec3::new(0.012, 0.0035, 0.022),
            speed_min: 2.4,
            speed_max: 8.5,
            tangent_spread: 0.82,
            friction: 0.48,
            restitution: 0.34,
            density: 3.2,
            angular_speed: 20.0,
        },
        PersistentImpactDebrisKind::Wood => BulletImpactDebrisProfile {
            kind,
            count: 10,
            half_extents: Vec3::new(0.006, 0.004, 0.036),
            speed_min: 1.4,
            speed_max: 7.2,
            tangent_spread: 1.05,
            friction: 0.70,
            restitution: 0.12,
            density: 0.48,
            angular_speed: 16.0,
        },
        PersistentImpactDebrisKind::Glass => BulletImpactDebrisProfile {
            kind,
            count: 12,
            half_extents: Vec3::new(0.009, 0.0025, 0.020),
            speed_min: 1.8,
            speed_max: 7.8,
            tangent_spread: 1.18,
            friction: 0.36,
            restitution: 0.26,
            density: 0.62,
            angular_speed: 22.0,
        },
    }
}

#[inline]
fn signed_impact_noise(
    owner: EntityId,
    shot_sequence: u64,
    shard_index: usize,
    channel: u64,
) -> f32 {
    let seed = owner.stable_u64()
        ^ shot_sequence.rotate_left(29)
        ^ (shard_index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ channel.wrapping_mul(0xD1B5_4A32_D192_ED03);
    let bits = (avalanche_u64(seed) >> 40) as u32 & 0x00ff_ffff;
    (bits as f32 / 0x00ff_ffffu32 as f32) * 2.0 - 1.0
}

#[inline]
fn impact_noise01(owner: EntityId, shot_sequence: u64, shard_index: usize, channel: u64) -> f32 {
    signed_impact_noise(owner, shot_sequence, shard_index, channel) * 0.5 + 0.5
}

#[inline]
fn impact_source_material_id(world: &World, target: Option<EntityId>) -> u64 {
    target
        .and_then(|entity| {
            world.get::<newengine_engine_runtime::scene_bridge::PrimitiveMaterialBase>(entity)
        })
        .map(|material| material.id.raw())
        .unwrap_or(0)
}

fn spawn_persistent_impact_debris(
    world: &mut World,
    owner: EntityId,
    shot_sequence: u64,
    point: Vec3,
    normal: Vec3,
    incoming_direction: Vec3,
    target: Option<EntityId>,
    surface: Option<&str>,
) -> usize {
    let Some(kind) = impact_debris_kind(surface) else {
        return 0;
    };
    let profile = impact_debris_profile(kind);
    let source_material_id = impact_source_material_id(world, target);
    let active = world.query::<ActiveImpactDebris>().count();
    let spawn_count = profile
        .count
        .min(IMPACT_DEBRIS_ACTIVE_BUDGET.saturating_sub(active));
    if spawn_count == 0 {
        return 0;
    }

    let normal = normal.normalize_or_zero();
    let normal = if normal.length_squared() > 1.0e-8 {
        normal
    } else {
        Vec3::Y
    };
    let incoming = incoming_direction.normalize_or_zero();
    let reference = if normal.y.abs() < 0.92 {
        Vec3::Y
    } else {
        Vec3::X
    };
    let tangent = normal.cross(reference).normalize_or_zero();
    let bitangent = tangent.cross(normal).normalize_or_zero();
    let reflection = if incoming.length_squared() > 1.0e-8 {
        (incoming - normal * (2.0 * incoming.dot(normal))).normalize_or_zero()
    } else {
        normal
    };
    let fixed_tick = world
        .resource::<PhysicsStepReport>()
        .map(|report| report.fixed_tick)
        .unwrap_or(0);

    for shard_index in 0..spawn_count {
        let variant = (shard_index % 3) as u16;
        let tangent_x = signed_impact_noise(owner, shot_sequence, shard_index, 0);
        let tangent_y = signed_impact_noise(owner, shot_sequence, shard_index, 1);
        let outward = 0.65 + impact_noise01(owner, shot_sequence, shard_index, 2) * 0.55;
        let direction = (normal * outward
            + reflection * 0.28
            + tangent * tangent_x * profile.tangent_spread
            + bitangent * tangent_y * profile.tangent_spread)
            .normalize_or_zero();
        let direction = if direction.length_squared() > 1.0e-8 {
            direction
        } else {
            normal
        };
        let speed_t = impact_noise01(owner, shot_sequence, shard_index, 3);
        let speed = profile.speed_min + (profile.speed_max - profile.speed_min) * speed_t;
        let size_scale = 0.72 + impact_noise01(owner, shot_sequence, shard_index, 4) * 0.62;
        let variant_scale = match variant {
            0 => Vec3::new(1.0, 0.82, 1.18),
            1 => Vec3::new(1.28, 0.72, 0.82),
            _ => Vec3::new(0.78, 1.12, 1.32),
        };
        let half_extents = Vec3::new(
            profile.half_extents.x * variant_scale.x,
            profile.half_extents.y * variant_scale.y,
            profile.half_extents.z * variant_scale.z,
        ) * size_scale;
        let spawn_position = point + normal * (0.010 + half_extents.y) + direction * 0.004;
        let rotation = Quat::from_rotation_arc(Vec3::Z, direction).normalize_or_identity();
        let entity = world.spawn();
        let _ = world.insert(
            entity,
            Name(format!(
                "WeaponFx/ImpactDebris/{}/{:016x}/{shot_sequence}/{shard_index}",
                kind.label(),
                owner.stable_u64()
            )),
        );
        let _ = world.insert(
            entity,
            Transform {
                position: spawn_position,
                rotation,
                scale: Vec3::ONE,
            },
        );
        let _ = world.insert(entity, DisplayVisibility::default());
        let _ = world.insert(entity, GameplayActor);
        let _ = world.insert(
            entity,
            PhysicsSurface {
                id: format!("debris.{}", kind.label()),
                ..PhysicsSurface::default()
            },
        );
        let mut body = PhysicsBodyDesc::dynamic_solid(CollisionShapeDesc::Box {
            half_extents: [half_extents.x, half_extents.y, half_extents.z],
        });
        body.material.friction = profile.friction;
        body.material.restitution = profile.restitution;
        body.material.density = profile.density;
        body.flags.continuous_collision = true;
        let _ = world.insert(entity, body);
        let _ = world.insert(entity, body.to_bounds());
        let _ = world.insert(entity, Velocity(direction * speed));
        let angular_axis = Vec3::new(
            signed_impact_noise(owner, shot_sequence, shard_index, 5),
            signed_impact_noise(owner, shot_sequence, shard_index, 6),
            signed_impact_noise(owner, shot_sequence, shard_index, 7),
        )
        .normalize_or_zero();
        let angular_axis = if angular_axis.length_squared() > 1.0e-8 {
            angular_axis
        } else {
            Vec3::Y
        };
        let angular_speed = profile.angular_speed
            * (0.55 + impact_noise01(owner, shot_sequence, shard_index, 8) * 0.9);
        let _ = world.insert(entity, AngularVelocity(angular_axis * angular_speed));
        let _ = world.insert(
            entity,
            PersistentImpactDebris::new(
                owner.stable_u64(),
                shot_sequence,
                profile.kind,
                variant,
                [half_extents.x, half_extents.y, half_extents.z],
                fixed_tick,
            )
            .with_source_material_id(source_material_id),
        );
        let _ = world.insert(entity, PendingImpactDebrisVisual);
        let _ = world.insert(entity, ActiveImpactDebris);
        let _ = world.insert(entity, ImpactDebrisContactRuntime::default());
    }

    newengine_ulog_api::ulog::info!(
        "weapon impact persistent debris spawned owner={} shot={} surface='{}' kind='{}' source_material={:016x} shards={} persistence='permanent-frozen-clutter' active_budget={} ttl='none' eviction='none'",
        owner.stable_u64(),
        shot_sequence,
        surface.unwrap_or_default(),
        kind.label(),
        source_material_id,
        spawn_count,
        IMPACT_DEBRIS_ACTIVE_BUDGET,
    );
    spawn_count
}
