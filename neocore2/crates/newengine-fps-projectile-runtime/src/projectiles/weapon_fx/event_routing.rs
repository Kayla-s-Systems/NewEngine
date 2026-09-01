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

fn entity_index_from_stable_ids(
    world: &World,
    stable_ids: impl IntoIterator<Item = u64>,
) -> std::collections::HashMap<u64, EntityId> {
    let mut unresolved = stable_ids
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    if unresolved.is_empty() {
        return std::collections::HashMap::new();
    }

    let mut resolved = std::collections::HashMap::with_capacity(unresolved.len());
    for entity in world.iter_entities() {
        let stable_id = entity.stable_u64();
        if unresolved.remove(&stable_id) {
            resolved.insert(stable_id, entity);
            if unresolved.is_empty() {
                break;
            }
        }
    }
    resolved
}

fn weapon_event_entity_index(
    world: &World,
    events: &[GameplayEvent],
) -> std::collections::HashMap<u64, EntityId> {
    entity_index_from_stable_ids(
        world,
        events.iter().flat_map(|event| {
            event.source.into_iter().chain(
                event
                    .payload
                    .get("target")
                    .and_then(serde_json::Value::as_u64),
            )
        }),
    )
}

fn physics_event_entity_index(
    world: &World,
    events: &[PhysicsEvent],
) -> std::collections::HashMap<u64, EntityId> {
    entity_index_from_stable_ids(
        world,
        events
            .iter()
            .filter_map(|event| match event {
                PhysicsEvent::ContactBegin(contact) | PhysicsEvent::ContactPersist(contact) => {
                    Some([contact.a.stable_id, contact.b.stable_id])
                }
                _ => None,
            })
            .flatten(),
    )
}

/// Built-in weapon presentation subscriber. Combat publishes semantic facts; this consumer owns
/// the default muzzle/tracer/impact composition. Project policy receives the same event batch and
/// can independently attach audio, scripting or additional presentation without changing combat.
pub fn consume_weapon_gameplay_events(world: &mut World, events: &[GameplayEvent]) {
    let entity_index = weapon_event_entity_index(world, events);
    for event in events {
        let Some(owner) = event
            .source
            .and_then(|source| entity_index.get(&source).copied())
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
                    .and_then(|target| entity_index.get(&target).copied());
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
