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

    let entity_index = physics_event_entity_index(world, &report.events);
    for event in &report.events {
        let (contact, is_begin) = match event {
            PhysicsEvent::ContactBegin(contact) => (*contact, true),
            PhysicsEvent::ContactPersist(contact) => (*contact, false),
            _ => continue,
        };
        let a = entity_index.get(&contact.a.stable_id).copied();
        let b = entity_index.get(&contact.b.stable_id).copied();
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
                let entity_index = physics_event_entity_index(world, &report.events);
                for event in &report.events {
                    let PhysicsEvent::ContactBegin(contact) = event else {
                        continue;
                    };
                    let a = entity_index.get(&contact.a.stable_id).copied();
                    let b = entity_index.get(&contact.b.stable_id).copied();
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
