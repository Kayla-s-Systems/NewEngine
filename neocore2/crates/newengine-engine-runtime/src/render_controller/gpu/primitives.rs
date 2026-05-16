use newengine_core::render::*;
use newengine_core::{EngineError, EngineResult as CoreResult};
use newengine_math::collections::FxHashMap;
use newengine_primitives::{PrimitiveId, PrimitiveMesh, PrimitiveRegistry, PrimitiveVertex};

use super::types::PrimitiveGpu;

pub fn upload_primitive_mesh(
    r: &mut dyn newengine_core::render::RenderApi,
    mesh: &PrimitiveMesh,
    label: &str,
) -> CoreResult<PrimitiveGpu> {
    if mesh.vertices.is_empty() || mesh.indices.is_empty() {
        return Err(EngineError::other(format!(
            "{label}: cannot upload empty primitive mesh"
        )));
    }

    let vertex_stride = std::mem::size_of::<PrimitiveVertex>();
    let mut vbytes: Vec<u8> = Vec::with_capacity(mesh.vertices.len() * vertex_stride);
    for v in &mesh.vertices {
        for f in &v.pos {
            vbytes.extend_from_slice(&f.to_ne_bytes());
        }
        for f in &v.nrm {
            vbytes.extend_from_slice(&f.to_ne_bytes());
        }
        for f in &v.uv {
            vbytes.extend_from_slice(&f.to_ne_bytes());
        }
    }

    debug_assert_eq!(
        vbytes.len(),
        mesh.vertices.len() * vertex_stride,
        "PrimitiveVertex upload size mismatch"
    );

    let mut ibytes: Vec<u8> = Vec::with_capacity(mesh.indices.len() * 4);
    for i in &mesh.indices {
        ibytes.extend_from_slice(&i.to_ne_bytes());
    }

    let vb = r.create_buffer(
        BufferDesc::new(
            vbytes.len() as u64,
            BufferUsage::Vertex,
            MemoryHint::CpuToGpu,
        )
            .with_label(format!("{label}_vb")),
    )?;
    r.write_buffer(vb, 0, &vbytes)?;

    let ib = r.create_buffer(
        BufferDesc::new(
            ibytes.len() as u64,
            BufferUsage::Index,
            MemoryHint::CpuToGpu,
        )
            .with_label(format!("{label}_ib")),
    )?;
    r.write_buffer(ib, 0, &ibytes)?;

    Ok(PrimitiveGpu {
        vb,
        ib,
        index_count: mesh.indices.len() as u32,
    })
}

pub fn ensure_primitive_gpu(
    reg: &PrimitiveRegistry,
    id: PrimitiveId,
    cache: &mut FxHashMap<PrimitiveId, PrimitiveGpu>,
    r: &mut dyn newengine_core::render::RenderApi,
) -> CoreResult<PrimitiveGpu> {
    if let Some(g) = cache.get(&id).copied() {
        return Ok(g);
    }

    let mesh = reg
        .build_mesh(id)
        .map_err(|e| EngineError::other(format!("{e}")))?;
    let gpu = upload_primitive_mesh(r, &mesh, "game_prim")?;

    cache.insert(id, gpu);
    Ok(gpu)
}

#[allow(dead_code)]
pub fn draw_primitive_indexed(
    r: &mut dyn newengine_core::render::RenderApi,
    gpu: PrimitiveGpu,
) -> CoreResult<()> {
    r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
    r.set_index_buffer(BufferSlice::new(gpu.ib, 0), IndexFormat::U32)?;
    r.draw_indexed(DrawIndexedArgs::new(gpu.index_count))?;
    Ok(())
}
