use newengine_core::render::*;
use newengine_core::{EngineError, EngineResult};
use newengine_math::collections::FxHashMap;
use newengine_primitives::PrimitiveId;

use crate::gameplay::{PlayerSkinBinding, PlayerSkinPose};

use super::types::{PlayerSkinGpu, PrimitiveGpu, SkinPaletteGpu};

const MIN_SKIN_PALETTE_CAPACITY: usize = 256;
const MAX_SKIN_PALETTE_JOINTS: usize = 4096;
// Four palette slots prevent host-visible writes from racing in-flight GPU skinning.
// The current Vulkan backend uses two frames in flight; four slots leave extra reuse margin.
const SKIN_PALETTE_RING_SIZE: u64 = 4;

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
    owner_key: u64,
    pose_generation: u64,
    pose: &PlayerSkinPose,
    skin_bgl: BindGroupLayoutId,
    frame_index: u64,
    r: &mut dyn newengine_core::render::RenderApi,
) -> EngineResult<SkinPaletteGpu> {
    let joint_count = pose.palette.len();
    if joint_count == 0 || joint_count > MAX_SKIN_PALETTE_JOINTS {
        return Err(EngineError::other(format!(
            "invalid player skin palette owner={} joints={} supported=1..={}",
            owner_key, joint_count, MAX_SKIN_PALETTE_JOINTS
        )));
    }

    let ring_slot = (frame_index % SKIN_PALETTE_RING_SIZE) as u8;
    let cache_key = (owner_key, ring_slot);

    if !cache.contains_key(&cache_key) {
        let capacity = joint_count
            .max(MIN_SKIN_PALETTE_CAPACITY)
            .next_power_of_two()
            .min(MAX_SKIN_PALETTE_JOINTS);
        let size = capacity as u64 * 64;
        let buffer = r.create_buffer(
            BufferDesc::new(size, BufferUsage::Storage, MemoryHint::CpuToGpu)
                .with_label(format!("player_skin_palette_{}", owner_key)),
        )?;
        let bg = r.create_bind_group(
            BindGroupDesc::new(skin_bgl)
                .with_label(format!("player_skin_palette_{}_bg", owner_key))
                .with_storage0(BufferBinding::new(buffer, 0, size)),
        )?;
        cache.insert(
            cache_key,
            SkinPaletteGpu {
                buffer,
                bg,
                capacity_joints: capacity as u32,
                generation: u64::MAX,
                revision: 0,
            },
        );
    }

    let mut gpu = cache[&cache_key];
    if joint_count > gpu.capacity_joints as usize {
        return Err(EngineError::other(format!(
            "player skin palette exceeded persistent GPU capacity owner={} joints={} capacity={} action='model swap requires palette cache retirement'",
            owner_key, joint_count, gpu.capacity_joints
        )));
    }
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
