use newengine_core::render::*;
use newengine_core::EngineResult as CoreResult;

use super::shader_manifest::load_runtime_shader_program_manifest;
use super::types::{DebugLineGpu, DEBUG_LINE_UBO_SIZE};

pub fn ensure_debug_line_pipeline(
    cached: &mut Option<DebugLineGpu>,
    r: &mut dyn newengine_core::render::RenderApi,
    min_vertices: u32,
) -> CoreResult<DebugLineGpu> {
    if let Some(g) = *cached {
        if g.capacity_vertices >= min_vertices {
            return Ok(g);
        }
        r.destroy_bind_group(g.bg);
        r.destroy_bind_group_layout(g.bgl);
        r.destroy_buffer(g.ubo);
        r.destroy_buffer(g.vb);
        r.destroy_pipeline(g.pipeline);
        r.destroy_shader(g.vs);
        r.destroy_shader(g.fs);
        *cached = None;
    }

    let capacity_vertices = min_vertices.max(256).next_power_of_two();

    let manifest = load_runtime_shader_program_manifest("shaders/pipelines/debug_lines.pipeline.json")?;
    let vs_asset = manifest.shaders.vertex;
    let fs_asset = manifest.shaders.fragment;

    let vs_kind = vs_asset.source_kind()?;
    let vs_desc = ShaderDesc::from_asset(
        ShaderStage::Vertex,
        vs_asset.entry.clone(),
        vs_asset.logical_path.clone(),
        vs_kind,
    )
    .with_asset(
        ShaderAssetDesc::new(vs_asset.logical_path, vs_kind)
            .with_entry(vs_asset.entry)
            .with_variant(vs_asset.variant_id),
    )
    .with_label("game_debug_lines_vs");

    let fs_kind = fs_asset.source_kind()?;
    let fs_desc = ShaderDesc::from_asset(
        ShaderStage::Fragment,
        fs_asset.entry.clone(),
        fs_asset.logical_path.clone(),
        fs_kind,
    )
    .with_asset(
        ShaderAssetDesc::new(fs_asset.logical_path, fs_kind)
            .with_entry(fs_asset.entry)
            .with_variant(fs_asset.variant_id),
    )
    .with_label("game_debug_lines_fs");

    let vs = r.create_shader(vs_desc)?;
    let fs = r.create_shader(fs_desc)?;

    let layout = VertexLayout::new(
        32,
        vec![
            VertexAttribute::new(0, 0, VertexFormat::Float32x4),
            VertexAttribute::new(1, 16, VertexFormat::Float32x4),
        ],
    );

    let bgl = r.create_bind_group_layout(
        BindGroupLayoutDesc::new(vec![BindingKind::UniformBuffer])
            .with_label("game_debug_lines_bgl"),
    )?;
    let ubo = r.create_buffer(
        BufferDesc::new(
            DEBUG_LINE_UBO_SIZE,
            BufferUsage::Uniform,
            MemoryHint::CpuToGpu,
        )
            .with_label("game_debug_lines_ubo"),
    )?;
    r.write_buffer(ubo, 0, &[0u8; DEBUG_LINE_UBO_SIZE as usize])?;
    let bg = r.create_bind_group(
        BindGroupDesc::new(bgl)
            .with_label("game_debug_lines_bg")
            .with_uniform0(BufferBinding::new(ubo, 0, DEBUG_LINE_UBO_SIZE)),
    )?;

    let pipeline = r.create_pipeline(
        PipelineDesc::new(vs, fs, TextureFormat::Bgra8Unorm)
            .with_label("game_debug_lines_pipeline")
            .with_topology(PrimitiveTopology::LineList)
            .with_vertex_layouts(vec![layout])
            .with_bind_group_layouts(vec![bgl])
            .with_depth(TextureFormat::Depth32Float),
    )?;

    let vb = r.create_buffer(
        BufferDesc::new(
            capacity_vertices as u64 * 32,
            BufferUsage::Vertex,
            MemoryHint::CpuToGpu,
        )
            .with_label("game_debug_lines_vb"),
    )?;

    let gpu = DebugLineGpu {
        vb,
        ubo,
        bg,
        bgl,
        vs,
        fs,
        pipeline,
        capacity_vertices,
    };

    *cached = Some(gpu);
    Ok(gpu)
}

