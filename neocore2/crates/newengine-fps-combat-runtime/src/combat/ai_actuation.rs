use super::*;

#[inline]
fn ai_actor_operational(world: &World, entity: EntityId) -> bool {
    world
        .get::<AIController>(entity)
        .is_some_and(|controller| controller.enabled)
        && world
            .get::<CharacterControlState>(entity)
            .is_none_or(|state| state.enabled)
        && world
            .get::<CharacterLifeState>(entity)
            .is_none_or(|state| state.alive())
        && world
            .get::<Health>(entity)
            .is_none_or(|health| health.alive())
}

#[inline]
fn actor_eye_point(world: &World, entity: EntityId) -> Option<Vec3> {
    let transform = world.get::<Transform>(entity)?;
    let eye_height = world
        .get::<CharacterBody>(entity)
        .map(|body| body.sanitized().standing_eye_height)
        .unwrap_or(0.0);
    Some(transform.position + Vec3::Y * eye_height)
}

#[inline]
fn view_alignment(world: &World, actor: EntityId, target: EntityId) -> Option<(f32, f32)> {
    let origin = actor_eye_point(world, actor)?;
    let target = actor_eye_point(world, target)?;
    let delta = target - origin;
    let distance = delta.length();
    if !distance.is_finite() || distance <= 1.0e-5 {
        return None;
    }
    let direction = delta / distance;
    let rotation = world
        .get::<CharacterMotor>(actor)
        .map(|motor| Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0))
        .or_else(|| {
            world
                .get::<Transform>(actor)
                .map(|transform| transform.rotation)
        })?
        .normalize_or_identity();
    let forward = (rotation * -Vec3::Z).normalize_or_zero();
    if forward.length_squared() <= 1.0e-8 {
        return None;
    }
    let angle = forward.dot(direction).clamp(-1.0, 1.0).acos();
    Some((distance, angle))
}

fn update_ai_weapon_mount(world: &mut World, actor: EntityId) {
    let Some(mount) = world
        .get::<FpsActorWeaponMountTuning>(actor)
        .copied()
        .map(FpsActorWeaponMountTuning::sanitized)
    else {
        return;
    };
    if active_equipped_weapon_binding(world, actor).is_none() {
        let _ = world.remove::<EquippedWeaponMuzzle>(actor);
        return;
    }
    let Some(transform) = world.get::<Transform>(actor).copied() else {
        return;
    };
    let rotation = world
        .get::<CharacterMotor>(actor)
        .map(|motor| Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0))
        .unwrap_or(transform.rotation)
        .normalize_or_identity();
    let local_offset = Vec3::new(
        mount.local_offset[0],
        mount.local_offset[1],
        mount.local_offset[2],
    );
    let local_forward = Vec3::new(
        mount.local_forward[0],
        mount.local_forward[1],
        mount.local_forward[2],
    )
    .normalize_or_zero();
    if local_forward.length_squared() <= 1.0e-8 {
        let _ = world.remove::<EquippedWeaponMuzzle>(actor);
        return;
    }
    let position = transform.position + rotation * local_offset;
    let forward = (rotation * local_forward).normalize_or_zero();
    if let Some(muzzle) = EquippedWeaponMuzzle::new(position, forward) {
        let _ = world.insert(actor, muzzle);
    } else {
        let _ = world.remove::<EquippedWeaponMuzzle>(actor);
    }
}

/// Projects AI combat intent into the same FPS action vocabulary used by player input.
///
/// This stage never creates a hit, consumes health, or resolves damage. It only requests aim,
/// trigger, or reload actions; `step_actor_combat` remains the sole firearm state-machine path.
pub fn step_ai_combat_actuation(world: &mut World, fixed_tick: u64) {
    let actors = world
        .query::<FpsAiCombatTuning>()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();

    for actor in actors {
        let mut frame = CombatActuationState {
            source_frame: fixed_tick,
            ..CombatActuationState::default()
        };
        if !ai_actor_operational(world, actor) {
            let _ = world.insert(actor, frame);
            continue;
        }
        update_ai_weapon_mount(world, actor);
        let intent = world
            .get::<CombatIntent>(actor)
            .copied()
            .unwrap_or_default();
        if intent.kind != CombatIntentKind::Engage {
            let _ = world.insert(actor, frame);
            continue;
        }
        let Some(target) = intent.target else {
            let _ = world.insert(actor, frame);
            continue;
        };
        let visible = world
            .get::<PerceptionState>(actor)
            .is_some_and(|perception| perception.visible_target == Some(target));
        if !visible {
            let _ = world.insert(actor, frame);
            continue;
        }
        let tuning = world
            .get::<FpsAiCombatTuning>(actor)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let Some((distance, angle)) = view_alignment(world, actor, target) else {
            let _ = world.insert(actor, frame);
            continue;
        };
        let Some(binding) = active_equipped_weapon_binding(world, actor) else {
            let _ = world.insert(actor, frame);
            continue;
        };

        match (binding.weapon.firearm, binding.weapon.melee) {
            (Some(firearm), _) => {
                let max_distance = tuning.fire_distance.min(firearm.tuning.sanitized().range);
                let in_range = distance <= max_distance;
                frame.aim = in_range;
                let state = world.get::<PlayerWeaponState>(actor).copied();
                let reserve = equipped_reserve_ammo(world, actor).unwrap_or(0);
                let empty = state.is_some_and(|state| state.ammo_in_magazine == 0);
                if empty && reserve > 0 {
                    frame.reload_pressed = true;
                } else if in_range && angle <= tuning.aim_tolerance_radians {
                    // This is an AI actuation pulse, not a physical button edge. Keeping the pulse
                    // true while eligible lets the shared firing-pattern controller enforce
                    // semi/auto/burst cadence and weapon cooldown authoritatively.
                    frame.trigger_pressed = true;
                    frame.trigger_held = true;
                }
            }
            (None, Some(melee)) => {
                let melee = melee.sanitized();
                if distance <= melee.range && angle <= tuning.aim_tolerance_radians {
                    frame.trigger_pressed = true;
                    frame.trigger_held = true;
                }
            }
            (None, None) => {}
        }
        let _ = world.insert(actor, frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_engage_intent_never_synthesizes_weapon_actions() {
        let mut world = World::new();
        let actor = world.spawn();
        let _ = world.insert(actor, AIController::default());
        let _ = world.insert(actor, CharacterControlState::enabled());
        let _ = world.insert(actor, CharacterLifeState::Alive);
        let _ = world.insert(actor, Health::new(100.0));
        let _ = world.insert(actor, FpsAiCombatTuning::default());
        let _ = world.insert(actor, CombatIntent::default());
        step_ai_combat_actuation(&mut world, 17);
        let frame = world.get::<CombatActuationState>(actor).copied().unwrap();
        assert_eq!(frame.source_frame, 17);
        assert_eq!(
            frame,
            CombatActuationState {
                source_frame: 17,
                ..CombatActuationState::default()
            }
        );
    }

    #[test]
    fn dead_ai_clears_stale_trigger_actuation() {
        let mut world = World::new();
        let actor = world.spawn();
        let _ = world.insert(actor, AIController::default());
        let _ = world.insert(actor, CharacterControlState::disabled());
        let _ = world.insert(actor, CharacterLifeState::Dead);
        let _ = world.insert(actor, FpsAiCombatTuning::default());
        let _ = world.insert(
            actor,
            CombatActuationState {
                aim: true,
                trigger_pressed: true,
                trigger_held: true,
                source_frame: 1,
                ..CombatActuationState::default()
            },
        );
        step_ai_combat_actuation(&mut world, 2);
        assert_eq!(
            *world.get::<CombatActuationState>(actor).unwrap(),
            CombatActuationState {
                source_frame: 2,
                ..CombatActuationState::default()
            }
        );
    }
}
