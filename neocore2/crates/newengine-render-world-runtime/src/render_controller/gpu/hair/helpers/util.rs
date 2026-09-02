pub(super) fn slots_to_bytes(slots: &[HairSlot]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(slots.len() * HAIR_SLOT_BYTES);
    for slot in slots {
        for value in slot.0 {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
    }
    bytes
}

fn f32_array_bytes<const N: usize, const B: usize>(values: [f32; N]) -> [u8; B] {
    debug_assert_eq!(B, N * 4);
    let mut out = [0u8; B];
    for (index, value) in values.into_iter().enumerate() {
        let offset = index * 4;
        out[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
    }
    out
}

pub(super) fn topology_key(
    scene: &HairSceneV1,
    registry_generation: u64,
    pose_layout_generation: u64,
    poses: Option<&HairSkinPoseRegistryV1>,
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash = fnv_u64(hash, registry_generation);
    hash = fnv_u64(hash, pose_layout_generation);
    for instance in &scene.instances {
        hash = fnv_u64(hash, instance.instance_id);
        for byte in instance.groom.as_str().as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash = fnv_u64(hash, quality_code(instance.quality) as u64);
        let pose_id = instance.skin_pose_id.unwrap_or(0);
        hash = fnv_u64(hash, pose_id);
        let joint_count = instance
            .skin_pose_id
            .and_then(|pose_id| poses.and_then(|registry| registry.get(pose_id)))
            .map(|pose| pose.joint_deforms.len() as u64)
            .unwrap_or(0);
        hash = fnv_u64(hash, joint_count);
    }
    hash
}

pub(super) fn shader_set_key(shaders: &HairShaderSetV1) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for value in [
        Some(shaders.simulation.as_str()),
        Some(shaders.strands_vertex.as_str()),
        Some(shaders.strands_fragment.as_str()),
        shaders.shadow_vertex.as_deref(),
        shaders.shadow_fragment.as_deref(),
    ] {
        match value {
            Some(value) => {
                hash = fnv_u64(hash, 1);
                for byte in value.as_bytes() {
                    hash ^= u64::from(*byte);
                    hash = hash.wrapping_mul(0x100000001b3);
                }
            }
            None => hash = fnv_u64(hash, 0),
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[inline]
pub(super) fn fnv_u64(mut hash: u64, value: u64) -> u64 {
    for byte in value.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[inline]
pub(super) fn deterministic_angle(seed: u64, follower: u64) -> f32 {
    let mixed = seed
        .wrapping_mul(0x9e3779b97f4a7c15)
        .wrapping_add(follower.wrapping_mul(0xbf58476d1ce4e5b9));
    let unit = ((mixed >> 40) & 0x00ff_ffff) as f32 / 16_777_215.0;
    unit * std::f32::consts::TAU
}

#[inline]
pub(super) fn sanitize_dt(dt: f32) -> f32 {
    if dt.is_finite() {
        dt.clamp(0.0, 0.1)
    } else {
        0.0
    }
}

#[inline]
pub(super) fn quality_followers(authored: u8, quality: HairQualityTier) -> usize {
    let authored = usize::from(authored);
    match quality {
        HairQualityTier::Off => 0,
        HairQualityTier::Low => authored.min(1),
        HairQualityTier::Medium => authored.min(3),
        HairQualityTier::High => authored.min(7),
        HairQualityTier::Ultra => authored.min(16),
    }
}

#[inline]
pub(super) fn quality_code(quality: HairQualityTier) -> f32 {
    match quality {
        HairQualityTier::Off => 0.0,
        HairQualityTier::Low => 1.0,
        HairQualityTier::Medium => 2.0,
        HairQualityTier::High => 3.0,
        HairQualityTier::Ultra => 4.0,
    }
}
