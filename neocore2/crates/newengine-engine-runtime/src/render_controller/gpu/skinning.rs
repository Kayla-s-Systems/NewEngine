use newengine_core::render::*;
use newengine_core::{EngineError, EngineResult};
use newengine_math::collections::FxHashMap;
use newengine_primitives::PrimitiveId;

use crate::gameplay::{PlayerSkinBinding, PlayerSkinPose};
use crate::render_controller::resource_lifetime::RenderGpuLifetimeQueue;

use super::types::{PlayerSkinGpu, PrimitiveGpu, SkinPaletteGpu};

const MIN_SKIN_PALETTE_CAPACITY: usize = 256;
const MAX_SKIN_PALETTE_JOINTS: usize = 4096;

#[inline]
fn required_skin_palette_capacity(joint_count: usize) -> usize {
    debug_assert!((1..=MAX_SKIN_PALETTE_JOINTS).contains(&joint_count));
    joint_count
        .max(MIN_SKIN_PALETTE_CAPACITY)
        .next_power_of_two()
        .min(MAX_SKIN_PALETTE_JOINTS)
}

#[inline]
fn skin_palette_growth_capacity(
    current_capacity: Option<usize>,
    joint_count: usize,
) -> Option<usize> {
    match current_capacity {
        Some(capacity) if capacity >= joint_count => None,
        _ => Some(required_skin_palette_capacity(joint_count)),
    }
}

fn allocate_skin_palette_gpu(
    owner_key: u64,
    ring_slot: u8,
    capacity: usize,
    skin_bgl: BindGroupLayoutId,
    r: &mut dyn newengine_core::render::RenderApi,
) -> EngineResult<SkinPaletteGpu> {
    let size = capacity as u64 * 64;
    let buffer = r.create_buffer(
        BufferDesc::new(size, BufferUsage::Storage, MemoryHint::CpuToGpu).with_label(format!(
            "player_skin_palette_{}_slot_{}_capacity_{}",
            owner_key, ring_slot, capacity
        )),
    )?;
    let bg = match r.create_bind_group(
        BindGroupDesc::new(skin_bgl)
            .with_label(format!(
                "player_skin_palette_{}_slot_{}_bg",
                owner_key, ring_slot
            ))
            .with_storage0(BufferBinding::new(buffer, 0, size)),
    ) {
        Ok(bg) => bg,
        Err(error) => {
            r.destroy_buffer(buffer);
            return Err(error);
        }
    };
    Ok(SkinPaletteGpu {
        buffer,
        bg,
        capacity_joints: capacity as u32,
        generation: u64::MAX,
        revision: 0,
    })
}

pub fn ensure_player_skin_gpu(
    cache: &mut FxHashMap<PrimitiveId, PlayerSkinGpu>,
    primitive_id: PrimitiveId,
    primitive_gpu: PrimitiveGpu,
    skin: &PlayerSkinBinding,
    r: &mut dyn newengine_core::render::RenderApi,
) -> EngineResult<PlayerSkinGpu> {
    if skin.vertices.len() != primitive_gpu.vertex_count as usize {
        return Err(EngineError::other(format!(
            "skinned primitive vertex-count mismatch primitive={} mesh_vertices={} skin_vertices={}",
            primitive_id.0,
            primitive_gpu.vertex_count,
            skin.vertices.len()
        )));
    }
    if let Some(gpu) = cache.get(&primitive_id).copied() {
        if gpu.vertex_count != primitive_gpu.vertex_count {
            return Err(EngineError::other(format!(
                "cached skin vertex-count mismatch primitive={} cached={} mesh={}",
                primitive_id.0, gpu.vertex_count, primitive_gpu.vertex_count
            )));
        }
        return Ok(gpu);
    }
    if skin.vertices.is_empty() {
        return Err(EngineError::other(format!(
            "skinned primitive has empty skin stream primitive={}",
            primitive_id.0
        )));
    }

    let mut bytes = Vec::with_capacity(skin.vertices.len() * 48);
    let mut max_joint_index = 0u16;
    for vertex in &skin.vertices {
        max_joint_index = max_joint_index
            .max(*vertex.joints.iter().max().unwrap_or(&0))
            .max(*vertex.joints_extra.iter().max().unwrap_or(&0));
        for joint in vertex.joints {
            bytes.extend_from_slice(&joint.to_ne_bytes());
        }
        let sum = vertex
            .weights
            .iter()
            .chain(vertex.weights_extra.iter())
            .copied()
            .sum::<f32>();
        if !sum.is_finite() || sum <= 1.0e-8 {
            return Err(EngineError::other(format!(
                "skinned primitive contains invalid weights primitive={} weights={:?}",
                primitive_id.0, vertex.weights
            )));
        }
        for weight in vertex.weights {
            if !weight.is_finite() || weight < 0.0 {
                return Err(EngineError::other(format!(
                    "skinned primitive contains invalid weight primitive={} weight={}",
                    primitive_id.0, weight
                )));
            }
            bytes.extend_from_slice(&weight.to_ne_bytes());
        }
        for joint in vertex.joints_extra {
            bytes.extend_from_slice(&joint.to_ne_bytes());
        }
        for weight in vertex.weights_extra {
            if !weight.is_finite() || weight < 0.0 {
                return Err(EngineError::other(format!(
                    "skinned primitive contains invalid extra weight primitive={} weight={}",
                    primitive_id.0, weight
                )));
            }
            bytes.extend_from_slice(&weight.to_ne_bytes());
        }
    }
    debug_assert_eq!(bytes.len(), skin.vertices.len() * 48);
    let vb = r.create_buffer(
        BufferDesc::new(
            bytes.len() as u64,
            BufferUsage::Vertex,
            MemoryHint::CpuToGpu,
        )
        .with_label(format!("player_skin_{}_vb", primitive_id.0)),
    )?;
    r.write_buffer(vb, 0, &bytes)?;
    let gpu = PlayerSkinGpu {
        vb,
        vertex_count: skin.vertices.len() as u32,
        max_joint_index,
    };
    cache.insert(primitive_id, gpu);
    Ok(gpu)
}

pub fn ensure_skin_palette_gpu(
    cache: &mut FxHashMap<(u64, u8), SkinPaletteGpu>,
    lifetimes: &mut RenderGpuLifetimeQueue,
    owner_key: u64,
    pose_generation: u64,
    pose: &PlayerSkinPose,
    skin_bgl: BindGroupLayoutId,
    frame_index: u64,
    host_visible_ring_slots: u64,
    r: &mut dyn newengine_core::render::RenderApi,
) -> EngineResult<SkinPaletteGpu> {
    let joint_count = pose.palette.len();
    if joint_count == 0 || joint_count > MAX_SKIN_PALETTE_JOINTS {
        return Err(EngineError::other(format!(
            "invalid player skin palette owner={} joints={} supported=1..={}",
            owner_key, joint_count, MAX_SKIN_PALETTE_JOINTS
        )));
    }

    let ring_slot = (frame_index % host_visible_ring_slots.max(1)) as u8;
    let cache_key = (owner_key, ring_slot);

    let current_capacity = cache
        .get(&cache_key)
        .map(|gpu| gpu.capacity_joints as usize);
    if let Some(capacity) = skin_palette_growth_capacity(current_capacity, joint_count) {
        let replacement = allocate_skin_palette_gpu(owner_key, ring_slot, capacity, skin_bgl, r)?;
        if let Some(retired) = cache.insert(cache_key, replacement) {
            lifetimes.retire_bind_group_after_frame(retired.bg, frame_index);
            lifetimes.retire_buffer_after_frame(retired.buffer, frame_index);
            newengine_ulog_api::ulog::info!(
                "render skin palette: persistent cache grown owner={} ring_slot={} joints={} capacity={} -> {} generation={} retirement_after_frame={}",
                owner_key,
                ring_slot,
                joint_count,
                retired.capacity_joints,
                capacity,
                pose_generation,
                frame_index,
            );
        }
    }

    let mut gpu = cache.get(&cache_key).copied().ok_or_else(|| {
        EngineError::other(format!(
            "player skin palette allocation missing owner={} ring_slot={}",
            owner_key, ring_slot
        ))
    })?;
    if gpu.generation != pose_generation || gpu.revision != pose.revision {
        let mut bytes = Vec::with_capacity(joint_count * 64);
        for matrix in &pose.palette {
            for value in matrix.to_cols_array() {
                if !value.is_finite() {
                    return Err(EngineError::other(format!(
                        "player skin palette contains non-finite value owner={} revision={}",
                        owner_key, pose.revision
                    )));
                }
                bytes.extend_from_slice(&value.to_ne_bytes());
            }
        }
        r.write_buffer(gpu.buffer, 0, &bytes)?;
        gpu.generation = pose_generation;
        gpu.revision = pose.revision;
        cache.insert(cache_key, gpu);
    }
    Ok(gpu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_capacity_grows_across_large_small_large_character_swaps() {
        assert_eq!(skin_palette_growth_capacity(None, 727), Some(1024));
        assert_eq!(skin_palette_growth_capacity(Some(1024), 125), None);
        assert_eq!(skin_palette_growth_capacity(None, 125), Some(256));
        assert_eq!(skin_palette_growth_capacity(Some(256), 727), Some(1024));
        assert_eq!(skin_palette_growth_capacity(Some(1024), 727), None);
    }

    #[test]
    fn palette_capacity_is_bounded_by_supported_joint_limit() {
        assert_eq!(
            required_skin_palette_capacity(MAX_SKIN_PALETTE_JOINTS),
            MAX_SKIN_PALETTE_JOINTS
        );
    }
}
