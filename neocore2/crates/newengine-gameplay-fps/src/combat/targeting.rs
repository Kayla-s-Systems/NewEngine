use super::*;

const HITSCAN_QUERY_SALT: u64 = 0x243f_6a88_85a3_08d3;
const MELEE_QUERY_SALT: u64 = 0xa409_3822_299f_31d0;
const INTERACTION_QUERY_SALT: u64 = 0x1319_8a2e_0370_7344;

#[inline]
pub(super) fn hitscan_query_seq(player: EntityId, shot_sequence: u64) -> u64 {
    avalanche_u64(player.stable_u64() ^ HITSCAN_QUERY_SALT ^ shot_sequence.rotate_left(17))
}

#[inline]
pub(super) fn melee_query_seq(player: EntityId, attack_sequence: u64) -> u64 {
    avalanche_u64(player.stable_u64() ^ MELEE_QUERY_SALT ^ attack_sequence.rotate_left(23))
}

#[inline]
pub(super) fn interaction_query_seq(player: EntityId, source_frame: u64) -> u64 {
    avalanche_u64(player.stable_u64() ^ INTERACTION_QUERY_SALT ^ source_frame.rotate_left(29))
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

    // The equipped visual publishes the real barrel pose every presentation frame. Use its +Z
    // axis as the ballistic origin/direction so hip fire and ADS visibly leave the barrel. The
    // camera path remains a deterministic fallback for headless simulation and early startup.
    let muzzle = world.get::<EquippedWeaponMuzzle>(player).copied();
    let forward = muzzle
        .map(|muzzle| muzzle.forward.normalize_or_zero())
        .filter(|forward| forward.length_squared() > 1.0e-8)
        .unwrap_or(camera_forward);

    let spread = if aiming {
        tuning.aim_spread_radians
    } else {
        tuning.hip_spread_radians
    };
    let spread_scale = spread.tan();
    let offset_x = signed_unit(shot_sequence ^ 0x9e37_79b9) * spread_scale;
    let offset_y = signed_unit(shot_sequence ^ 0x7f4a_7c15) * spread_scale;
    let direction = (forward + right * offset_x + up * offset_y).normalize_or_zero();
    let origin = muzzle
        .map(|muzzle| muzzle.position + forward * 0.008)
        .unwrap_or(player_position + Vec3::Y * eye_height + forward * tuning.muzzle_forward_offset);
    Some((origin, direction))
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
