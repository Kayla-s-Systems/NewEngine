use super::*;

const HITSCAN_QUERY_SALT: u64 = 0x243f_6a88_85a3_08d3;
const INTERACTION_QUERY_SALT: u64 = 0x1319_8a2e_0370_7344;
const WEAPON_OBSTRUCTION_QUERY_SALT: u64 = 0x082e_fa98_ec4e_6c89;

#[inline]
pub(super) fn hitscan_query_seq(player: EntityId, shot_sequence: u64) -> u64 {
    avalanche_u64(player.stable_u64() ^ HITSCAN_QUERY_SALT ^ shot_sequence.rotate_left(17))
}

#[inline]
pub(super) fn hitscan_bounce_query_seq(
    player: EntityId,
    shot_sequence: u64,
    bounce_count: u8,
) -> u64 {
    avalanche_u64(
        hitscan_query_seq(player, shot_sequence)
            ^ (u64::from(bounce_count).wrapping_mul(0x9E37_79B9_7F4A_7C15)),
    )
}

#[inline]
pub(super) fn interaction_query_seq(player: EntityId, source_frame: u64) -> u64 {
    avalanche_u64(player.stable_u64() ^ INTERACTION_QUERY_SALT ^ source_frame.rotate_left(29))
}

#[inline]
fn weapon_obstruction_query_seq(player: EntityId, fixed_tick: u64) -> u64 {
    avalanche_u64(player.stable_u64() ^ WEAPON_OBSTRUCTION_QUERY_SALT ^ fixed_tick.rotate_left(11))
}

/// Queue one short body-to-muzzle ray every fixed step while a firearm is equipped. This is the
/// NorthStar gameplay-side authored weapon-obstruction probe: it detects when the authored
/// barrel would cross solid geometry before the shot query is created.
pub(super) fn queue_weapon_obstruction_probe(world: &mut World, player: EntityId, fixed_tick: u64) {
    let Some(muzzle) = active_equipped_weapon_muzzle(world, player) else {
        let _ = world.remove::<PendingWeaponObstructionProbe>(player);
        let _ = world.remove::<WeaponObstructionState>(player);
        return;
    };
    let Some((player_position, _)) = player_view_pose(world, player) else {
        return;
    };
    let eye_height = world
        .get::<PlayerStanceState>(player)
        .map(|stance| stance.current_eye_height)
        .or_else(|| {
            crate::game_data::active_game_data(world)
                .map(|data| data.player.tuning.camera_eye_height)
        })
        .unwrap_or(1.6)
        .max(0.2);

    // Shoulder/chest origin instead of camera origin. A wall between the body and muzzle is a
    // genuine weapon obstruction even when a third-person camera can still see around it.
    let origin = player_position + Vec3::Y * (eye_height * 0.72);
    let to_muzzle = muzzle.position - origin;
    let distance = to_muzzle.length();
    if !distance.is_finite() || !(0.12..=1.60).contains(&distance) {
        let _ = world.remove::<PendingWeaponObstructionProbe>(player);
        let _ = world.insert(
            player,
            WeaponObstructionState::clear(muzzle.position, fixed_tick),
        );
        return;
    }
    let direction = to_muzzle / distance;
    let _ = world.insert(
        player,
        PendingWeaponObstructionProbe {
            query_seq: weapon_obstruction_query_seq(player, fixed_tick),
            origin,
            direction,
            muzzle_distance: distance,
            muzzle_position: muzzle.position,
        },
    );
}

#[inline]
pub(super) fn signed_unit(seed: u64) -> f32 {
    let value = (avalanche_u64(seed) >> 40) as u32;
    (value as f32 / 0x00ff_ffffu32 as f32) * 2.0 - 1.0
}

#[inline]
fn player_view_pose(world: &World, player: EntityId) -> Option<(Vec3, Quat)> {
    let (position, body_rotation) =
        newengine_transform::read_entity_world_pose_local_chain(world, player)?;
    let rotation = world
        .get::<CharacterMotor>(player)
        .map(|motor| Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0))
        .unwrap_or(body_rotation)
        .normalize_or_identity();
    Some((position, rotation))
}

pub(super) fn melee_origin_and_direction(
    world: &World,
    player: EntityId,
    tuning: MeleeWeaponTuning,
) -> Option<(Vec3, Vec3)> {
    let tuning = tuning.sanitized();
    let (player_position, rotation) = player_view_pose(world, player)?;
    let eye_height = world
        .get::<PlayerStanceState>(player)
        .map(|stance| stance.current_eye_height)
        .or_else(|| {
            crate::game_data::active_game_data(world)
                .map(|data| data.player.tuning.camera_eye_height)
        })
        .unwrap_or(1.6)
        .max(0.2);
    let origin = player_position + Vec3::Y * (eye_height * 0.72);
    let direction = (rotation * -Vec3::Z).normalize_or_zero();
    (direction.length_squared() > 1.0e-8 && tuning.range > 0.0).then_some((origin, direction))
}

pub(super) fn shot_origin_and_direction(
    world: &World,
    player: EntityId,
    tuning: HitscanWeaponTuning,
    aiming: bool,
    shot_sequence: u64,
) -> Option<(Vec3, Vec3)> {
    let (player_position, view_rotation) = player_view_pose(world, player)?;
    let eye_height = match world.get::<PlayerStanceState>(player) {
        Some(stance) => stance.current_eye_height,
        None => {
            crate::game_data::active_game_data(world)?
                .player
                .tuning
                .camera_eye_height
        }
    };
    let camera_forward = (view_rotation * -Vec3::Z).normalize_or_zero();
    let right = (view_rotation * Vec3::X).normalize_or_zero();
    let up = (view_rotation * Vec3::Y).normalize_or_zero();
    if camera_forward.length_squared() <= 1.0e-8 {
        return None;
    }

    // Third-person shooting is camera-targeted but physically originates at the barrel. Resolve a
    // convergence point on the view axis, then shoot from the real/safe muzzle toward that point.
    // This keeps the reticle and ballistic path coherent without allowing a camera ray to shoot
    // through cover that the barrel itself cannot clear.
    let active_camera_position = world
        .resource::<newengine_scene::SceneState>()
        .and_then(|state| state.active_camera.or(state.root))
        .and_then(|camera| world.get::<newengine_sim::CameraRigComp>(camera))
        .map(|rig| rig.0.position)
        .filter(|position| position.is_finite());
    let view_origin = active_camera_position.unwrap_or(player_position + Vec3::Y * eye_height);

    // A firearm shot is invalid until presentation has published a physical muzzle socket.
    // The camera only selects a convergence target; it can never synthesize the shot origin.
    let muzzle = active_equipped_weapon_muzzle(world, player)?;
    let obstruction = world
        .get::<WeaponObstructionState>(player)
        .copied()
        .filter(|state| state.blocked && state.alpha > 0.001);
    let muzzle_origin = obstruction
        .map(|state| state.safe_muzzle_position)
        .filter(|position| position.is_finite())
        .unwrap_or_else(|| muzzle.position + muzzle.forward.normalize_or_zero() * 0.008);

    let hip_convergence = world
        .get::<EquippedWeaponBinding>(player)
        .and_then(|binding| {
            world
                .resource::<newengine_engine_runtime::gameplay::ItemCatalog>()
                .and_then(|catalog| catalog.get(binding.item))
        })
        .map(|definition| definition.weapon_presentation.clone().sanitized())
        .filter(|presentation| presentation.enabled)
        .map(|presentation| presentation.first_person_hip_convergence_m)
        .unwrap_or(12.0);
    let convergence_distance = if aiming {
        tuning.range.clamp(12.0, 80.0)
    } else {
        hip_convergence.clamp(4.0, tuning.range.max(4.0))
    };
    let aim_point = view_origin + camera_forward * convergence_distance;
    let ballistic_forward = (aim_point - muzzle_origin).normalize_or_zero();
    let forward = if ballistic_forward.length_squared() > 1.0e-8 {
        ballistic_forward
    } else {
        let muzzle_forward = muzzle.forward.normalize_or_zero();
        if muzzle_forward.length_squared() > 1.0e-8 {
            muzzle_forward
        } else {
            camera_forward
        }
    };

    let base_spread = if aiming {
        tuning.aim_spread_radians
    } else {
        tuning.hip_spread_radians
    };
    let horizontal_speed = world
        .get::<Velocity>(player)
        .map(|velocity| Vec3::new(velocity.0.x, 0.0, velocity.0.z).length())
        .unwrap_or(0.0);
    let movement_alpha = (horizontal_speed / 4.5).clamp(0.0, 1.0);
    let movement_multiplier =
        1.0 + (tuning.movement_spread_multiplier - 1.0) * movement_alpha;
    let stance_multiplier = world
        .get::<PlayerStanceState>(player)
        .map(|stance| match stance.current {
            PlayerStanceKind::Standing => 1.0,
            PlayerStanceKind::Crouched => tuning.crouch_spread_multiplier,
        })
        .unwrap_or(1.0);
    let authored_modifiers = world
        .get::<WeaponAccuracyModifiers>(player)
        .copied()
        .unwrap_or_default()
        .combined()
        * active_equipped_weapon_component_modifiers(world, player).accuracy_multiplier;
    let bloom = world
        .get::<WeaponAccuracyState>(player)
        .filter(|state| {
            world
                .get::<EquippedWeaponBinding>(player)
                .is_some_and(|binding| binding.instance_id == state.weapon_instance_id)
        })
        .map(|state| state.bloom_radians)
        .unwrap_or(0.0);
    let spread = (base_spread * movement_multiplier * stance_multiplier * authored_modifiers + bloom)
        .clamp(0.0, core::f32::consts::FRAC_PI_2 - 0.001);
    let spread_scale = spread.tan();
    let offset_x = signed_unit(shot_sequence ^ 0x9e37_79b9) * spread_scale;
    let offset_y = signed_unit(shot_sequence ^ 0x7f4a_7c15) * spread_scale;
    let direction = (forward + right * offset_x + up * offset_y).normalize_or_zero();
    Some((muzzle_origin, direction))
}

pub(super) fn interaction_ray(
    world: &World,
    player: EntityId,
    tuning: PlayerInteractionTuning,
) -> Option<(Vec3, Vec3)> {
    let (player_position, view_rotation) = player_view_pose(world, player)?;
    let eye_height = match world.get::<PlayerStanceState>(player) {
        Some(stance) => stance.current_eye_height,
        None => {
            crate::game_data::active_game_data(world)?
                .player
                .tuning
                .camera_eye_height
        }
    };
    let direction = (view_rotation * -Vec3::Z).normalize_or_zero();
    if direction.length_squared() <= 1.0e-8 {
        return None;
    }
    Some((
        player_position + Vec3::Y * eye_height + direction * tuning.ray_origin_forward_offset,
        direction,
    ))
}
