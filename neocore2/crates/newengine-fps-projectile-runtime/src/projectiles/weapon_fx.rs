fn payload_vec3(payload: &serde_json::Value, key: &str) -> Option<Vec3> {
    let values = payload.get(key)?.as_array()?;
    if values.len() != 3 {
        return None;
    }
    let value = Vec3::new(
        values[0].as_f64()? as f32,
        values[1].as_f64()? as f32,
        values[2].as_f64()? as f32,
    );
    value.is_finite().then_some(value)
}

#[inline]
fn weapon_segment_correlation_id(shot_sequence: u64, bounce_count: u8) -> u64 {
    if bounce_count == 0 {
        shot_sequence
    } else {
        avalanche_u64(shot_sequence ^ u64::from(bounce_count).wrapping_mul(0x9E37_79B9_7F4A_7C15))
    }
}

fn entity_from_stable_id(world: &World, stable_id: u64) -> Option<EntityId> {
    world
        .iter_entities()
        .find(|entity| entity.stable_u64() == stable_id)
}

/// Built-in weapon presentation subscriber. Combat publishes semantic facts; this consumer owns
/// the default muzzle/tracer/impact composition. Project policy receives the same event batch and
/// can independently attach audio, scripting or additional presentation without changing combat.
pub fn consume_weapon_gameplay_events(world: &mut World, events: &[GameplayEvent]) {
    for event in events {
        let Some(owner) = event
            .source
            .and_then(|source| entity_from_stable_id(world, source))
        else {
            continue;
        };
        let shot_sequence = event
            .payload
            .get("shot_sequence")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        match event.id.as_str() {
            GAMEPLAY_EVENT_WEAPON_FIRED => {
                let Some(origin) = payload_vec3(&event.payload, "shot_origin") else {
                    continue;
                };
                let Some(direction) = payload_vec3(&event.payload, "shot_direction") else {
                    continue;
                };
                let range = event
                    .payload
                    .get("range")
                    .and_then(serde_json::Value::as_f64)
                    .map(|value| value as f32)
                    .unwrap_or(0.0);
                spawn_weapon_shot_fx(world, owner, shot_sequence, origin, direction, range);
            }
            GAMEPLAY_EVENT_WEAPON_HIT => {
                if event
                    .payload
                    .get("attack_kind")
                    .and_then(serde_json::Value::as_str)
                    != Some("firearm")
                {
                    continue;
                }
                let Some(point) = payload_vec3(&event.payload, "point") else {
                    continue;
                };
                let normal = payload_vec3(&event.payload, "normal").unwrap_or(Vec3::Y);
                let incoming_direction = payload_vec3(&event.payload, "shot_direction")
                    .unwrap_or(-normal)
                    .normalize_or_zero();
                let target = event
                    .payload
                    .get("target")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|target| entity_from_stable_id(world, target));
                let bounce_count = event
                    .payload
                    .get("bounce_count")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
                    .min(u8::MAX as u64) as u8;
                resolve_weapon_shot_hit_fx_segment(
                    world,
                    owner,
                    shot_sequence,
                    bounce_count,
                    point,
                    normal,
                    incoming_direction,
                    target,
                );
                if event
                    .payload
                    .get("ricochet")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                {
                    let reflected = payload_vec3(&event.payload, "ricochet_direction")
                        .unwrap_or_else(|| {
                            let incoming = payload_vec3(&event.payload, "shot_direction")
                                .unwrap_or(-normal)
                                .normalize_or_zero();
                            let n = normal.normalize_or_zero();
                            (incoming - n * (2.0 * incoming.dot(n))).normalize_or_zero()
                        });
                    let remaining = event
                        .payload
                        .get("ricochet_range")
                        .and_then(serde_json::Value::as_f64)
                        .map(|value| value as f32)
                        .unwrap_or(0.0);
                    spawn_weapon_ricochet_fx(
                        world,
                        owner,
                        shot_sequence,
                        bounce_count.saturating_add(1),
                        point,
                        normal,
                        reflected,
                        remaining,
                    );
                }
            }
            GAMEPLAY_EVENT_WEAPON_PENETRATED => {
                let Some(point) = payload_vec3(&event.payload, "exit_point") else {
                    continue;
                };
                let normal = payload_vec3(&event.payload, "exit_normal").unwrap_or(Vec3::Y);
                let direction = payload_vec3(&event.payload, "shot_direction")
                    .unwrap_or(-normal)
                    .normalize_or_zero();
                let effect = equipped_weapon_vfx_definition(world, owner).and_then(|vfx| vfx.exit);
                let Some(effect) = effect else {
                    continue;
                };
                let effect_owner = equipped_weapon_entity(world, owner).unwrap_or(owner);
                let request = newengine_vfx_api::VfxSpawnRequestV1 {
                    effect: newengine_vfx_api::VfxEffectRef::new(effect),
                    owner: Some(newengine_vfx_api::EntityHandle::new(
                        effect_owner.stable_u64(),
                    )),
                    correlation_id: weapon_segment_correlation_id(shot_sequence, 0)
                        ^ 0x4558_4954_5f56_4658,
                    position: vec3_array(point + direction * 0.004),
                    direction: vec3_array(direction),
                    normal: vec3_array(normal),
                    seed: effect_owner.stable_u64() ^ shot_sequence.rotate_left(31),
                    surface: event
                        .payload
                        .get("surface")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned),
                    tags: vec!["weapon".to_owned(), "penetration_exit".to_owned()],
                    ..Default::default()
                };
                let _ = newengine_vfx_runtime::spawn_vfx(world, request);
            }
            _ => {}
        }
    }
}

fn equipped_weapon_vfx_definition(world: &World, owner: EntityId) -> Option<WeaponVfxDefinition> {
    let binding = world.get::<EquippedWeaponBinding>(owner).copied()?;
    let mut vfx = world
        .resource::<ItemCatalog>()?
        .get(binding.item)
        .map(|definition| definition.weapon_vfx.clone())?;
    let (_, muzzle_override, tracer_override) =
        newengine_engine_runtime::gameplay::active_equipped_weapon_component_overrides(
            world, owner,
        );
    if muzzle_override.is_some() {
        vfx.shot = muzzle_override;
    }
    if tracer_override.is_some() {
        vfx.tracer = tracer_override;
    }
    Some(vfx)
}

#[inline]
fn equipped_weapon_entity(world: &World, owner: EntityId) -> Option<EntityId> {
    let binding = world.get::<EquippedWeaponBinding>(owner).copied()?;
    let link = world.get::<EquippedWeaponEntity>(owner).copied()?;
    (link.instance_id == binding.instance_id
        && link.item == binding.item
        && world.exists(link.entity))
    .then_some(link.entity)
}

#[inline]
fn signed_casing_noise(
    owner: EntityId,
    weapon_item_id: u64,
    shot_sequence: u64,
    channel: u64,
) -> f32 {
    let seed = owner.stable_u64()
        ^ weapon_item_id.rotate_left(17)
        ^ shot_sequence.rotate_left(31)
        ^ channel.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let bits = (newengine_math::avalanche_u64(seed) >> 40) as u32 & 0x00ff_ffff;
    (bits as f32 / 0x00ff_ffffu32 as f32) * 2.0 - 1.0
}

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

const SHELL_SETTLE_LINEAR_SPEED_MPS: f32 = 0.055;
const SHELL_SETTLE_ANGULAR_SPEED_RADPS: f32 = 1.0;
const SHELL_WAKE_LINEAR_SPEED_MPS: f32 = 0.22;
const SHELL_WAKE_ANGULAR_SPEED_RADPS: f32 = 4.0;
const SHELL_SETTLE_HOLD_SECONDS: f32 = 0.20;

#[derive(Clone, Copy, Debug, Default)]
struct WeaponShellContactRuntime {
    impact_cooldown_seconds: f32,
    rolling_cooldown_seconds: f32,
    quiet_seconds: f32,
    settled: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct WeaponShellPhysicsEventCursor {
    fixed_tick: u64,
}

fn weapon_shell_definition(
    world: &World,
    casing: WeaponShellCasing,
) -> Option<newengine_engine_runtime::gameplay::WeaponCasingDefinition> {
    world
        .resource::<ItemCatalog>()?
        .get(ItemId(casing.weapon_item_id))
        .map(|definition| definition.weapon_casing.clone().sanitized())
}

fn casing_contact_class(
    definition: &newengine_engine_runtime::gameplay::WeaponCasingDefinition,
    surface: &str,
    impulse: f32,
) -> Option<&'static str> {
    if impulse < definition.contact_min_impulse {
        return None;
    }
    let surface_lower = surface.trim().to_ascii_lowercase();
    if definition.soft_surface_contains.iter().any(|needle| {
        !needle.trim().is_empty() && surface_lower.contains(&needle.trim().to_ascii_lowercase())
    }) {
        return Some("dirt");
    }
    if impulse >= definition.contact_hard_impulse {
        Some("hard")
    } else if impulse >= definition.contact_medium_impulse {
        Some("medium")
    } else {
        Some("small")
    }
}

fn publish_shell_physics_event(
    world: &mut World,
    casing_entity: EntityId,
    other_entity: EntityId,
    casing: WeaponShellCasing,
    event_id: &'static str,
    contact_class: &str,
    point: Vec3,
    impulse: f32,
    fixed_tick: u64,
) {
    let velocity = world
        .get::<Velocity>(casing_entity)
        .copied()
        .unwrap_or_default()
        .0;
    let angular_velocity = world
        .get::<AngularVelocity>(casing_entity)
        .copied()
        .unwrap_or_default()
        .0;
    let surface = world
        .get::<PhysicsSurface>(other_entity)
        .map(|surface| surface.id.clone())
        .unwrap_or_default();
    let weapon = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(ItemId(casing.weapon_item_id)))
        .map(|definition| definition.name.clone());
    let _ = emit_gameplay_event(
        world,
        event_id,
        Some(casing_entity),
        serde_json::json!({
            "schema": "newengine.gameplay.weapon_shell_physics_event.v1",
            "version": 1,
            "owner": casing.owner_stable_id,
            "weapon_item_id": casing.weapon_item_id,
            "weapon": weapon,
            "shot_sequence": casing.shot_sequence,
            "casing": casing_entity.stable_u64(),
            "other": other_entity.stable_u64(),
            "position": vec3_array(point),
            "surface": surface,
            "contact_class": contact_class,
            "impulse": impulse,
            "linear_speed": velocity.length(),
            "angular_speed": angular_velocity.length(),
            "fixed_tick": fixed_tick,
        }),
    );
}

fn process_shell_physics_events(world: &mut World, dt: f32) {
    let shells = world
        .query::<WeaponShellCasing>()
        .map(|(entity, casing)| (entity, *casing))
        .collect::<Vec<_>>();
    for (entity, _) in &shells {
        let mut state = world
            .get::<WeaponShellContactRuntime>(*entity)
            .copied()
            .unwrap_or_default();
        state.impact_cooldown_seconds = (state.impact_cooldown_seconds - dt).max(0.0);
        state.rolling_cooldown_seconds = (state.rolling_cooldown_seconds - dt).max(0.0);

        let linear_speed = world
            .get::<Velocity>(*entity)
            .copied()
            .unwrap_or_default()
            .0
            .length();
        let angular_speed = world
            .get::<AngularVelocity>(*entity)
            .copied()
            .unwrap_or_default()
            .0
            .length();

        if state.settled {
            // Sleeping/settled brass must remain acoustically silent through solver jitter.
            // Only a real wake impulse is allowed to re-arm rolling/contact audio.
            if linear_speed >= SHELL_WAKE_LINEAR_SPEED_MPS
                || angular_speed >= SHELL_WAKE_ANGULAR_SPEED_RADPS
            {
                state.settled = false;
                state.quiet_seconds = 0.0;
            }
        } else if linear_speed <= SHELL_SETTLE_LINEAR_SPEED_MPS
            && angular_speed <= SHELL_SETTLE_ANGULAR_SPEED_RADPS
        {
            state.quiet_seconds = (state.quiet_seconds + dt).min(SHELL_SETTLE_HOLD_SECONDS);
            if state.quiet_seconds >= SHELL_SETTLE_HOLD_SECONDS {
                state.settled = true;
                state.impact_cooldown_seconds = 0.0;
                state.rolling_cooldown_seconds = 0.0;
                newengine_ulog_api::ulog::debug!(
                    "weapon casing physically settled entity={} linear_speed={:.4} angular_speed={:.4} physics='dynamic-sleepable'",
                    entity.stable_u64(),
                    linear_speed,
                    angular_speed,
                );
            }
        } else {
            state.quiet_seconds = 0.0;
        }

        let _ = world.insert(*entity, state);
    }

    let Some(report) = world.resource::<PhysicsStepReport>().cloned() else {
        return;
    };
    if report.fixed_tick == 0 {
        return;
    }
    let last_tick = world
        .resource::<WeaponShellPhysicsEventCursor>()
        .copied()
        .unwrap_or_default()
        .fixed_tick;
    if report.fixed_tick <= last_tick {
        return;
    }

    for event in &report.events {
        let (contact, is_begin) = match event {
            PhysicsEvent::ContactBegin(contact) => (*contact, true),
            PhysicsEvent::ContactPersist(contact) => (*contact, false),
            _ => continue,
        };
        let a = entity_from_stable_id(world, contact.a.stable_id);
        let b = entity_from_stable_id(world, contact.b.stable_id);
        let Some((casing_entity, other_entity, casing)) = a
            .and_then(|entity| {
                world
                    .get::<WeaponShellCasing>(entity)
                    .copied()
                    .map(|casing| (entity, b, casing))
            })
            .or_else(|| {
                b.and_then(|entity| {
                    world
                        .get::<WeaponShellCasing>(entity)
                        .copied()
                        .map(|casing| (entity, a, casing))
                })
            })
            .and_then(|(casing_entity, other, casing)| {
                other.map(|other| (casing_entity, other, casing))
            })
        else {
            continue;
        };
        let Some(definition) = weapon_shell_definition(world, casing) else {
            continue;
        };
        let surface = world
            .get::<PhysicsSurface>(other_entity)
            .map(|surface| surface.id.as_str())
            .unwrap_or("");
        let surface_lower = surface.trim().to_ascii_lowercase();
        let soft_surface = definition.soft_surface_contains.iter().any(|needle| {
            let needle = needle.trim().to_ascii_lowercase();
            !needle.is_empty() && surface_lower.contains(&needle)
        });
        let contact_class = casing_contact_class(&definition, surface, contact.impulse);
        let mut state = world
            .get::<WeaponShellContactRuntime>(casing_entity)
            .copied()
            .unwrap_or_default();
        if state.settled {
            // ContactPersist continues while a rigid body rests on a surface. Once the casing has
            // physically settled, those maintenance contacts are not audible impacts/rolling.
            continue;
        }

        if let Some(contact_class) = contact_class {
            // Impact audio belongs to collision transitions only. A resting/rolling rigid body
            // receives ContactPersist support impulses every solver tick; treating those as new
            // impacts is exactly what produced the repeating ground-hit sound while brass rolled.
            // A genuine bounce separates first, so its next collision arrives as ContactBegin.
            if is_begin && state.impact_cooldown_seconds <= 0.0 {
                publish_shell_physics_event(
                    world,
                    casing_entity,
                    other_entity,
                    casing,
                    GAMEPLAY_EVENT_WEAPON_SHELL_CONTACT,
                    contact_class,
                    contact.point,
                    contact.impulse,
                    report.fixed_tick,
                );
                state.impact_cooldown_seconds = 0.035;
            }
        }

        if !is_begin && state.rolling_cooldown_seconds <= 0.0 {
            let velocity = world
                .get::<Velocity>(casing_entity)
                .copied()
                .unwrap_or_default()
                .0;
            let angular = world
                .get::<AngularVelocity>(casing_entity)
                .copied()
                .unwrap_or_default()
                .0;
            let normal = contact.normal.normalize_or_zero();
            let normal_speed = velocity.dot(normal).abs();
            let tangent_speed = (velocity - normal * velocity.dot(normal)).length();
            let angular_speed = angular.length();
            if tangent_speed >= 0.08 && angular_speed >= 1.25 && normal_speed <= 0.9 {
                publish_shell_physics_event(
                    world,
                    casing_entity,
                    other_entity,
                    casing,
                    GAMEPLAY_EVENT_WEAPON_SHELL_ROLLING,
                    if soft_surface { "dirt" } else { "small" },
                    contact.point,
                    contact.impulse,
                    report.fixed_tick,
                );
                state.rolling_cooldown_seconds = (0.14 - tangent_speed * 0.025).clamp(0.045, 0.14);
            }
        }
        let _ = world.insert(casing_entity, state);
    }
    world.insert_resource(WeaponShellPhysicsEventCursor {
        fixed_tick: report.fixed_tick,
    });
}

fn publish_impact_debris_contact_event(
    world: &mut World,
    debris_entity: EntityId,
    other_entity: EntityId,
    debris: PersistentImpactDebris,
    point: Vec3,
    impulse: f32,
    fixed_tick: u64,
) {
    let surface = world
        .get::<PhysicsSurface>(other_entity)
        .map(|surface| surface.id.clone())
        .unwrap_or_default();
    let velocity = world
        .get::<Velocity>(debris_entity)
        .copied()
        .unwrap_or_default()
        .0;
    let _ = emit_gameplay_event(
        world,
        GAMEPLAY_EVENT_WEAPON_IMPACT_DEBRIS_CONTACT,
        Some(debris_entity),
        serde_json::json!({
            "schema": "newengine.gameplay.weapon_impact_debris_contact.v1",
            "version": 1,
            "owner": debris.owner_stable_id,
            "shot_sequence": debris.shot_sequence,
            "debris": debris_entity.stable_u64(),
            "debris_kind": debris.kind.label(),
            "variant": debris.variant,
            "position": vec3_array(point),
            "surface": surface,
            "impulse": impulse,
            "linear_speed": velocity.length(),
            "fixed_tick": fixed_tick,
        }),
    );
}

#[inline]
fn impact_debris_contact_threshold(kind: PersistentImpactDebrisKind) -> f32 {
    match kind {
        PersistentImpactDebrisKind::Concrete => 0.012,
        PersistentImpactDebrisKind::Metal => 0.006,
        PersistentImpactDebrisKind::Wood => 0.010,
        PersistentImpactDebrisKind::Glass => 0.004,
    }
}

fn freeze_impact_debris_to_persistent_clutter(world: &mut World, entity: EntityId) {
    // Once a shard has physically settled, gameplay/physics relinquishes ownership permanently.
    // Removing PhysicsBodyDesc makes the entity disappear from the next PhysicsFrameInput; the
    // packet backend then destroys the stale Jolt body. The render hierarchy and persistent
    // identity remain in ECS with no TTL, no age update and no periodic cleanup scan.
    let _ = world.remove::<PhysicsBodyDesc>(entity);
    let _ = world.remove::<Velocity>(entity);
    let _ = world.remove::<AngularVelocity>(entity);
    let _ = world.remove::<GameplayActor>(entity);
    let _ = world.remove::<ActiveImpactDebris>(entity);
    let _ = world.remove::<ImpactDebrisContactRuntime>(entity);
}

fn process_impact_debris_physics_events(world: &mut World, dt: f32) {
    // Hot path is bounded exclusively by active rigid shards. Frozen clutter keeps
    // PersistentImpactDebris but loses ActiveImpactDebris, so it never enters this query again.
    let active_entities = world
        .query::<ActiveImpactDebris>()
        .filter_map(|(entity, _)| {
            world
                .get::<PersistentImpactDebris>(entity)
                .copied()
                .map(|debris| (entity, debris))
        })
        .collect::<Vec<_>>();

    // Consume each physics contact report at most once and publish secondary debris/clatter audio
    // only for still-active rigid shards. Frozen clutter is intentionally non-colliding/non-ticking.
    if let Some(report) = world.resource::<PhysicsStepReport>().cloned() {
        if report.fixed_tick > 0 {
            let last_tick = world
                .resource::<ImpactDebrisPhysicsEventCursor>()
                .copied()
                .unwrap_or_default()
                .fixed_tick;
            if report.fixed_tick > last_tick {
                for event in &report.events {
                    let PhysicsEvent::ContactBegin(contact) = event else {
                        continue;
                    };
                    let a = entity_from_stable_id(world, contact.a.stable_id);
                    let b = entity_from_stable_id(world, contact.b.stable_id);
                    let Some((debris_entity, other_entity, debris)) = a
                        .and_then(|entity| {
                            world
                                .get::<ActiveImpactDebris>(entity)
                                .and_then(|_| world.get::<PersistentImpactDebris>(entity))
                                .copied()
                                .map(|debris| (entity, b, debris))
                        })
                        .or_else(|| {
                            b.and_then(|entity| {
                                world
                                    .get::<ActiveImpactDebris>(entity)
                                    .and_then(|_| world.get::<PersistentImpactDebris>(entity))
                                    .copied()
                                    .map(|debris| (entity, a, debris))
                            })
                        })
                        .and_then(|(debris_entity, other, debris)| {
                            other.map(|other| (debris_entity, other, debris))
                        })
                    else {
                        continue;
                    };
                    if world.get::<PersistentImpactDebris>(other_entity).is_some() {
                        continue;
                    }
                    let mut state = world
                        .get::<ImpactDebrisContactRuntime>(debris_entity)
                        .copied()
                        .unwrap_or_default();
                    if state.contact_cooldown_seconds > 0.0
                        || contact.impulse < impact_debris_contact_threshold(debris.kind)
                    {
                        continue;
                    }
                    publish_impact_debris_contact_event(
                        world,
                        debris_entity,
                        other_entity,
                        debris,
                        contact.point,
                        contact.impulse,
                        report.fixed_tick,
                    );
                    state.contact_cooldown_seconds = 0.09;
                    let _ = world.insert(debris_entity, state);
                }
                world.insert_resource(ImpactDebrisPhysicsEventCursor {
                    fixed_tick: report.fixed_tick,
                });
            }
        }
    }

    let mut freeze = Vec::new();
    for (entity, _) in active_entities {
        if world.get::<ActiveImpactDebris>(entity).is_none() {
            continue;
        }
        let mut state = world
            .get::<ImpactDebrisContactRuntime>(entity)
            .copied()
            .unwrap_or_default();
        state.contact_cooldown_seconds = (state.contact_cooldown_seconds - dt).max(0.0);
        let linear_speed = world
            .get::<Velocity>(entity)
            .copied()
            .unwrap_or_default()
            .0
            .length();
        let angular_speed = world
            .get::<AngularVelocity>(entity)
            .copied()
            .unwrap_or_default()
            .0
            .length();
        if linear_speed <= IMPACT_DEBRIS_SETTLE_LINEAR_SPEED_MPS
            && angular_speed <= IMPACT_DEBRIS_SETTLE_ANGULAR_SPEED_RADPS
        {
            state.quiet_seconds = (state.quiet_seconds + dt).min(IMPACT_DEBRIS_SETTLE_HOLD_SECONDS);
            if state.quiet_seconds >= IMPACT_DEBRIS_SETTLE_HOLD_SECONDS {
                freeze.push(entity);
                continue;
            }
        } else {
            state.quiet_seconds = 0.0;
        }
        let _ = world.insert(entity, state);
    }

    for entity in freeze {
        freeze_impact_debris_to_persistent_clutter(world, entity);
    }
}

fn fallback_weapon_socket(position: Vec3, forward: Vec3) -> Option<WeaponSocketPose> {
    let forward = forward.normalize_or_zero();
    if !position.is_finite() || forward.length_squared() <= 1.0e-8 {
        return None;
    }
    let rotation = Quat::from_rotation_arc(Vec3::Z, forward).normalize_or_identity();
    WeaponSocketPose::stationary(position, rotation)
}
fn spawn_weapon_segment_effect(
    world: &mut World,
    owner: EntityId,
    effect: String,
    shot_sequence: u64,
    bounce_count: u8,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
    tag: &str,
) {
    let direction = direction.normalize_or_zero();
    if !origin.is_finite() || direction.length_squared() <= 1.0e-8 || max_distance <= 0.0 {
        return;
    }
    let effect_owner = equipped_weapon_entity(world, owner).unwrap_or(owner);
    let correlation_id = weapon_segment_correlation_id(shot_sequence, bounce_count);
    let request = newengine_vfx_api::VfxSpawnRequestV1 {
        effect: newengine_vfx_api::VfxEffectRef::new(effect),
        owner: Some(newengine_vfx_api::EntityHandle::new(
            effect_owner.stable_u64(),
        )),
        correlation_id,
        position: vec3_array(origin),
        direction: vec3_array(direction),
        max_distance: max_distance.clamp(0.05, 1_000.0),
        seed: effect_owner.stable_u64()
            ^ correlation_id.rotate_left(23)
            ^ u64::from(bounce_count).rotate_left(41),
        tags: vec!["weapon".to_owned(), tag.to_owned()],
        ..Default::default()
    };
    if let Err(error) = newengine_vfx_runtime::spawn_vfx(world, request) {
        newengine_ulog_api::ulog::warn!(
            "project weapon segment VFX rejected owner={} shot={} bounce={} tag='{}' err='{}'",
            owner.stable_u64(),
            shot_sequence,
            bounce_count,
            tag,
            error,
        );
    }
}

fn spawn_weapon_ricochet_fx(
    world: &mut World,
    owner: EntityId,
    shot_sequence: u64,
    bounce_count: u8,
    point: Vec3,
    normal: Vec3,
    reflected: Vec3,
    remaining: f32,
) {
    let Some(vfx) = equipped_weapon_vfx_definition(world, owner) else {
        return;
    };
    let effect_owner = equipped_weapon_entity(world, owner).unwrap_or(owner);
    let correlation_id = weapon_segment_correlation_id(shot_sequence, bounce_count);
    if let Some(effect) = vfx.ricochet {
        let request = newengine_vfx_api::VfxSpawnRequestV1 {
            effect: newengine_vfx_api::VfxEffectRef::new(effect),
            owner: Some(newengine_vfx_api::EntityHandle::new(
                effect_owner.stable_u64(),
            )),
            correlation_id,
            position: vec3_array(point + normal.normalize_or_zero() * 0.004),
            direction: vec3_array(reflected),
            normal: vec3_array(normal),
            seed: effect_owner.stable_u64() ^ correlation_id.rotate_left(29),
            tags: vec!["weapon".to_owned(), "ricochet".to_owned()],
            ..Default::default()
        };
        let _ = newengine_vfx_runtime::spawn_vfx(world, request);
    }
    if let Some(tracer) = vfx.tracer {
        spawn_weapon_segment_effect(
            world,
            owner,
            tracer,
            shot_sequence,
            bounce_count,
            point + reflected.normalize_or_zero() * 0.012,
            reflected,
            remaining,
            "ricochet_tracer",
        );
    }
}

/// Publishes a semantic weapon-shot effect from the already-resolved physical muzzle.
/// Damage/collision remain authoritative in the hitscan path; transient visual composition,
/// budgets and lifetime are owned by `newengine-vfx-runtime`. Physical shell casings remain here
/// because they are persistent world objects with authored rigid-body behavior.
pub fn spawn_weapon_shot_fx(
    world: &mut World,
    owner: EntityId,
    shot_sequence: u64,
    origin: Vec3,
    direction: Vec3,
    range: f32,
) {
    let direction = direction.normalize_or_zero();
    if !origin.is_finite() || direction.length_squared() <= 1.0e-8 {
        return;
    }
    let max_distance = if range.is_finite() && range > 0.0 {
        range.clamp(0.1, 1_000.0)
    } else {
        0.0
    };
    if let Some(effect) = equipped_weapon_vfx_definition(world, owner).and_then(|vfx| vfx.shot) {
        let effect_owner = equipped_weapon_entity(world, owner).unwrap_or(owner);
        let request = newengine_vfx_api::VfxSpawnRequestV1 {
            effect: newengine_vfx_api::VfxEffectRef::new(effect),
            owner: Some(newengine_vfx_api::EntityHandle::new(
                effect_owner.stable_u64(),
            )),
            correlation_id: shot_sequence,
            position: vec3_array(origin),
            direction: vec3_array(direction),
            max_distance,
            seed: effect_owner.stable_u64() ^ shot_sequence.rotate_left(23),
            tags: vec!["weapon".to_owned(), "shot".to_owned()],
            ..Default::default()
        };
        if let Err(error) = newengine_vfx_runtime::spawn_vfx(world, request) {
            newengine_ulog_api::ulog::warn!(
                "project weapon shot VFX rejected owner={} shot={} err='{}'",
                owner.stable_u64(),
                shot_sequence,
                error
            );
        }
    }
    if let Some(tracer) = equipped_weapon_vfx_definition(world, owner).and_then(|vfx| vfx.tracer) {
        spawn_weapon_segment_effect(
            world,
            owner,
            tracer,
            shot_sequence,
            0,
            origin,
            direction,
            max_distance,
            "tracer",
        );
    }
    // Physical casing behavior belongs to the equipped weapon definition. Weapons without an
    // authored casing contract simply do not schedule a casing entity.
    let casing_contract = world
        .get::<EquippedWeaponBinding>(owner)
        .copied()
        .and_then(|binding| {
            world
                .resource::<ItemCatalog>()?
                .get(binding.item)
                .map(|definition| {
                    (
                        binding.item.raw(),
                        definition.weapon_casing.clone().sanitized(),
                    )
                })
        })
        .filter(|(_, casing)| casing.enabled());
    if let Some((weapon_item_id, casing)) = casing_contract {
        let pending = world.spawn();
        let _ = world.insert(
            pending,
            Name(format!(
                "WeaponFx/ShellEjectionPending/{:016x}/{shot_sequence}",
                owner.stable_u64()
            )),
        );
        let _ = world.insert(
            pending,
            PendingWeaponShellEjection {
                owner,
                weapon_entity: equipped_weapon_entity(world, owner),
                shot_sequence,
                weapon_item_id,
                shot_origin: origin,
                shot_direction: direction,
                remaining_seconds: casing.ejection_delay_seconds,
            },
        );
    }
}
fn spawn_persistent_shell_casing(
    world: &mut World,
    owner: EntityId,
    weapon_entity: Option<EntityId>,
    shot_sequence: u64,
    weapon_item_id: u64,
    fallback_origin: Vec3,
    fallback_direction: Vec3,
) -> Option<EntityId> {
    let casing_definition = world
        .resource::<ItemCatalog>()?
        .get(ItemId(weapon_item_id))?
        .weapon_casing
        .clone()
        .sanitized();
    if !casing_definition.enabled() {
        return None;
    }
    let authored_socket = weapon_entity
        .filter(|entity| world.exists(*entity))
        .and_then(|entity| world.get::<WeaponEntitySockets>(entity))
        .and_then(|sockets| sockets.casing_ejection);
    let muzzle_socket = weapon_entity
        .filter(|entity| world.exists(*entity))
        .and_then(|entity| world.get::<WeaponEntitySockets>(entity))
        .and_then(|sockets| sockets.muzzle)
        .or_else(|| {
            world
                .get::<EquippedWeaponMuzzle>(owner)
                .copied()
                .and_then(|muzzle| fallback_weapon_socket(muzzle.position, muzzle.forward))
        });
    let socket = if casing_definition.ejection_joint.is_some() {
        authored_socket.or(muzzle_socket)
    } else {
        muzzle_socket
    }
    .or_else(|| fallback_weapon_socket(fallback_origin, fallback_direction))?;

    let right = (socket.rotation * Vec3::X).normalize_or_zero();
    let up = (socket.rotation * Vec3::Y).normalize_or_zero();
    let forward = (socket.rotation * Vec3::Z).normalize_or_zero();
    if right.length_squared() <= 1.0e-8
        || up.length_squared() <= 1.0e-8
        || forward.length_squared() <= 1.0e-8
    {
        return None;
    }
    let local_vector = |value: [f32; 3]| right * value[0] + up * value[1] + forward * value[2];
    let casing_origin = socket.position + local_vector(casing_definition.origin_local);
    let velocity_local = [
        casing_definition.velocity_local[0]
            + signed_casing_noise(owner, weapon_item_id, shot_sequence, 0)
                * casing_definition.velocity_jitter[0],
        casing_definition.velocity_local[1]
            + signed_casing_noise(owner, weapon_item_id, shot_sequence, 1)
                * casing_definition.velocity_jitter[1],
        casing_definition.velocity_local[2]
            + signed_casing_noise(owner, weapon_item_id, shot_sequence, 2)
                * casing_definition.velocity_jitter[2],
    ];
    let casing_velocity = socket.linear_velocity * casing_definition.inherit_socket_linear_velocity
        + local_vector(velocity_local);
    let casing_axis = local_vector(casing_definition.axis_local).normalize_or_zero();
    let casing_axis = if casing_axis.length_squared() > 1.0e-8 {
        casing_axis
    } else {
        right
    };
    // Physics cylinders are Y-up in the provider-neutral contract. The authored shell mesh is
    // Z-long, so presentation applies a local Z->Y correction while the rigid body itself maps
    // +Y onto the ejection axis.
    let casing_rotation = Quat::from_rotation_arc(Vec3::Y, casing_axis).normalize_or_identity();
    let variant_count = casing_definition
        .variants
        .len()
        .min(u16::MAX as usize)
        .max(1);
    let variant = (shot_sequence % variant_count as u64) as u16;
    let casing = world.spawn();
    let _ = world.insert(
        casing,
        Name(format!(
            "WeaponFx/ShellCasing/{:016x}/{shot_sequence}",
            owner.stable_u64()
        )),
    );
    let _ = world.insert(
        casing,
        Transform {
            position: casing_origin,
            rotation: casing_rotation,
            scale: Vec3::ONE,
        },
    );
    // Model-backed casings use an invisible staging root. The world presentation provider admits
    // the authored model/material hierarchy atomically, so no generic brass-colored cube leaks in.
    let _ = world.insert(casing, DisplayVisibility::default());
    let _ = world.insert(casing, GameplayActor);
    let _ = world.insert(casing, PhysicsSurface::default());

    let casing_radius = casing_definition.half_extents[0]
        .abs()
        .max(casing_definition.half_extents[1].abs())
        .max(0.001);
    let casing_half_height = casing_definition.half_extents[2].abs().max(0.001);
    let mut body = PhysicsBodyDesc::dynamic_solid(CollisionShapeDesc::Cylinder {
        radius: casing_radius,
        half_height: casing_half_height,
    });
    body.material.friction = casing_definition.friction;
    body.material.restitution = casing_definition.restitution;
    body.material.density = casing_definition.density;
    body.linear_damping = Some(casing_definition.linear_damping);
    body.angular_damping = Some(casing_definition.angular_damping);
    body.flags.continuous_collision = true;
    let _ = world.insert(casing, body);
    let _ = world.insert(casing, body.to_bounds());
    let _ = world.insert(casing, Velocity(casing_velocity));
    let angular_local = [
        casing_definition.angular_velocity[0]
            + signed_casing_noise(owner, weapon_item_id, shot_sequence, 3)
                * casing_definition.angular_velocity_jitter[0],
        casing_definition.angular_velocity[1]
            + signed_casing_noise(owner, weapon_item_id, shot_sequence, 4)
                * casing_definition.angular_velocity_jitter[1],
        casing_definition.angular_velocity[2]
            + signed_casing_noise(owner, weapon_item_id, shot_sequence, 5)
                * casing_definition.angular_velocity_jitter[2],
    ];
    let casing_angular_velocity = socket.angular_velocity
        * casing_definition.inherit_socket_angular_velocity
        + local_vector(angular_local);
    let _ = world.insert(casing, AngularVelocity(casing_angular_velocity));
    let _ = world.insert(
        casing,
        WeaponShellCasing::new(owner.stable_u64(), shot_sequence, weapon_item_id, variant),
    );
    let _ = world.insert(casing, WeaponShellContactRuntime::default());
    let weapon_name = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(ItemId(weapon_item_id)))
        .map(|definition| definition.name.clone());
    if let Err(error) = emit_gameplay_event(
        world,
        GAMEPLAY_EVENT_WEAPON_SHELL_EJECTED,
        Some(owner),
        serde_json::json!({
            "schema": "newengine.gameplay.weapon_shell_event.v1",
            "version": 1,
            "weapon_item_id": weapon_item_id,
            "weapon": weapon_name,
            "shot_sequence": shot_sequence,
            "casing": casing.stable_u64(),
            "position": vec3_array(casing_origin),
            "velocity": vec3_array(casing_velocity),
            "variant": variant,
        }),
    ) {
        newengine_ulog_api::ulog::warn!(
            "weapon shell semantic event rejected owner={} shot={} err='{}'",
            owner.stable_u64(),
            shot_sequence,
            error,
        );
    }
    newengine_ulog_api::ulog::info!(
        "weapon casing ejected entity={} owner={} weapon_entity={:?} shot={} weapon_item={:016x} variant={} delay_ms={:.3} collider_cylinder=[radius={:.5},half_height={:.5}] inherited_linear={:.3} inherited_angular={:.3} physics='dynamic+damped' persistence='world' visual='authored-definition'",
        casing.stable_u64(),
        owner.stable_u64(),
        weapon_entity.map(EntityId::stable_u64),
        shot_sequence,
        weapon_item_id,
        variant,
        casing_definition.ejection_delay_seconds * 1000.0,
        casing_radius,
        casing_half_height,
        casing_definition.inherit_socket_linear_velocity,
        casing_definition.inherit_socket_angular_velocity,
    );
    Some(casing)
}

/// Narrows the semantic tracer to the authoritative hitscan impact without creating an impact
/// composition. Kept as a compatibility facade for callers that only know the hit point.
pub fn clamp_weapon_shot_fx_to_hit(
    world: &mut World,
    owner: EntityId,
    shot_sequence: u64,
    point: Vec3,
) {
    let effect_owner = equipped_weapon_entity(world, owner).unwrap_or(owner);
    newengine_vfx_runtime::clamp_vfx_tracers_to_hit(
        world,
        effect_owner.stable_u64(),
        shot_sequence,
        point,
    );
}

/// Resolves the complete weapon impact presentation: tracer termination plus surface-aware
/// sparks/smoke/decal composition. The physics query remains authoritative for point/normal/entity.
pub fn resolve_weapon_shot_hit_fx(
    world: &mut World,
    owner: EntityId,
    shot_sequence: u64,
    point: Vec3,
    normal: Vec3,
    target: Option<EntityId>,
) {
    resolve_weapon_shot_hit_fx_segment(
        world,
        owner,
        shot_sequence,
        0,
        point,
        normal,
        -normal,
        target,
    );
}

fn resolve_weapon_shot_hit_fx_segment(
    world: &mut World,
    owner: EntityId,
    shot_sequence: u64,
    bounce_count: u8,
    point: Vec3,
    normal: Vec3,
    incoming_direction: Vec3,
    target: Option<EntityId>,
) {
    if !point.is_finite() {
        return;
    }
    let effect_owner = equipped_weapon_entity(world, owner).unwrap_or(owner);
    newengine_vfx_runtime::clamp_vfx_tracers_to_hit(
        world,
        effect_owner.stable_u64(),
        weapon_segment_correlation_id(shot_sequence, bounce_count),
        point,
    );
    let mut normal = if normal.is_finite() && normal.length_squared() > 1.0e-8 {
        normal.normalize_or_zero()
    } else {
        Vec3::Y
    };
    let incoming_direction =
        if incoming_direction.is_finite() && incoming_direction.length_squared() > 1.0e-8 {
            incoming_direction.normalize_or_zero()
        } else {
            -normal
        };
    // Presentation always lives on the incident side of the contact plane. Physics providers are
    // allowed to expose either triangle winding convention, so canonicalize the visual normal
    // against the incoming ray before applying decal/particle normal offsets.
    if incoming_direction.dot(normal) > 0.0 {
        normal = -normal;
    }
    let surface = target
        .and_then(|entity| world.get::<PhysicsSurface>(entity))
        .map(|surface| surface.id.clone());
    let effect = equipped_weapon_vfx_definition(world, owner)
        .and_then(|vfx| vfx.impact_effect(surface.as_deref()).map(str::to_owned));
    let Some(effect) = effect else {
        return;
    };
    let request = newengine_vfx_api::VfxSpawnRequestV1 {
        effect: newengine_vfx_api::VfxEffectRef::new(effect),
        owner: Some(newengine_vfx_api::EntityHandle::new(
            effect_owner.stable_u64(),
        )),
        correlation_id: weapon_segment_correlation_id(shot_sequence, bounce_count),
        position: vec3_array(point),
        direction: vec3_array(incoming_direction),
        normal: vec3_array(normal),
        seed: effect_owner.stable_u64()
            ^ shot_sequence.rotate_left(23)
            ^ point.x.to_bits() as u64
            ^ (point.y.to_bits() as u64).rotate_left(11)
            ^ (point.z.to_bits() as u64).rotate_left(37),
        surface: surface.clone(),
        tags: vec!["weapon".to_owned(), "impact".to_owned()],
        ..Default::default()
    };
    if let Err(error) = newengine_vfx_runtime::spawn_vfx(world, request) {
        newengine_ulog_api::ulog::warn!(
            "project weapon impact VFX rejected owner={} shot={} err='{}'",
            owner.stable_u64(),
            shot_sequence,
            error
        );
    }
    let physical_surface = target
        .and_then(|entity| world.get::<newengine_engine_runtime::gameplay::DamageReceiver>(entity))
        .is_none_or(|receiver| {
            receiver.kind != newengine_engine_runtime::gameplay::DamageReceiverKind::Character
        });
    if physical_surface {
        let _ = spawn_persistent_impact_debris(
            world,
            owner,
            shot_sequence,
            point,
            normal,
            incoming_direction,
            target,
            surface.as_deref(),
        );
    }
}

pub fn step_weapon_shot_fx(world: &mut World, dt: f32) {
    let dt = finite_or(dt, 0.0).clamp(0.0, 0.1);
    if dt <= 0.0 {
        return;
    }

    newengine_vfx_runtime::step_vfx(world, dt);
    process_shell_physics_events(world, dt);
    process_impact_debris_physics_events(world, dt);

    let pending_ejections = world
        .query::<PendingWeaponShellEjection>()
        .map(|(entity, pending)| (entity, *pending))
        .collect::<Vec<_>>();
    for (entity, mut pending) in pending_ejections {
        pending.remaining_seconds -= dt;
        if pending.remaining_seconds <= 0.0 {
            let _ = spawn_persistent_shell_casing(
                world,
                pending.owner,
                pending.weapon_entity,
                pending.shot_sequence,
                pending.weapon_item_id,
                pending.shot_origin,
                pending.shot_direction,
            );
            let _ = world.despawn(entity);
        } else {
            let _ = world.insert(entity, pending);
        }
    }
}

#[inline]
fn vec3_array(value: Vec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}
