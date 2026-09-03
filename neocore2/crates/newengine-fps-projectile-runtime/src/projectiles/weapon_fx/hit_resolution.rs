const DEFAULT_WEAPON_IMPACT_PARTICLE_EFFECT: &str =
    "shared/vfx/weapon/firearm.fxd@bullet_impact";

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

#[allow(clippy::too_many_arguments)]
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
    // A firearm contact always emits a short-lived particle composition at the authoritative
    // hit point. Weapon data may specialize it per surface; Shared owns the production fallback.
    let effect = equipped_weapon_vfx_definition(world, owner)
        .and_then(|vfx| vfx.impact_effect(surface.as_deref()).map(str::to_owned))
        .unwrap_or_else(|| DEFAULT_WEAPON_IMPACT_PARTICLE_EFFECT.to_owned());
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
