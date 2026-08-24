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

pub(super) fn shot_origin_and_direction(
    world: &World,
    player: EntityId,
    tuning: HitscanWeaponTuning,
    aiming: bool,
    shot_sequence: u64,
) -> Option<(Vec3, Vec3)> {
    let transform = world.get::<Transform>(player).copied()?;
    let eye_height = world
        .get::<PlayerStanceState>(player)
        .map(|stance| stance.current_eye_height)
        .unwrap_or_else(|| {
            crate::game_data::active_game_data(world)
                .player
                .tuning
                .camera_eye_height
        });
    let forward = (transform.rotation * Vec3::new(0.0, 0.0, -1.0)).normalize_or_zero();
    let right = (transform.rotation * Vec3::X).normalize_or_zero();
    let up = (transform.rotation * Vec3::Y).normalize_or_zero();
    if forward.length_squared() <= 1.0e-8 {
        return None;
    }

    let spread = if aiming {
        tuning.aim_spread_radians
    } else {
        tuning.hip_spread_radians
    };
    let spread_scale = spread.tan();
    let offset_x = signed_unit(shot_sequence ^ 0x9e37_79b9) * spread_scale;
    let offset_y = signed_unit(shot_sequence ^ 0x7f4a_7c15) * spread_scale;
    let direction = (forward + right * offset_x + up * offset_y).normalize_or_zero();
    let origin = transform.position + Vec3::Y * eye_height + forward * tuning.muzzle_forward_offset;
    Some((origin, direction))
}

pub(super) fn interaction_ray(
    world: &World,
    player: EntityId,
    tuning: PlayerInteractionTuning,
) -> Option<(Vec3, Vec3)> {
    let transform = world.get::<Transform>(player).copied()?;
    let eye_height = world
        .get::<PlayerStanceState>(player)
        .map(|stance| stance.current_eye_height)
        .unwrap_or_else(|| {
            crate::game_data::active_game_data(world)
                .player
                .tuning
                .camera_eye_height
        });
    let direction = (transform.rotation * Vec3::new(0.0, 0.0, -1.0)).normalize_or_zero();
    if direction.length_squared() <= 1.0e-8 {
        return None;
    }
    Some((
        transform.position + Vec3::Y * eye_height + direction * tuning.ray_origin_forward_offset,
        direction,
    ))
}
