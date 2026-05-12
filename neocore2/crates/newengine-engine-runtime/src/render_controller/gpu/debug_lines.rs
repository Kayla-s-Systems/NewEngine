use newengine_core::render::*;
use newengine_core::EngineResult as CoreResult;

use super::shader_assets::{compile_glsl, load_text_asset};
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

    let vs_src = load_text_asset("shaders/editor_debug_lines.vert")?;
    let fs_src = load_text_asset("shaders/editor_debug_lines.frag")?;
    let vs_spv = compile_glsl(ShaderStage::Vertex, "editor_debug_lines.vert", &vs_src)?;
    let fs_spv = compile_glsl(ShaderStage::Fragment, "editor_debug_lines.frag", &fs_src)?;

    let vs = r.create_shader(
        ShaderDesc::new(ShaderStage::Vertex, "main", vs_spv).with_label("editor_debug_lines_vs"),
    )?;
    let fs = r.create_shader(
        ShaderDesc::new(ShaderStage::Fragment, "main", fs_spv).with_label("editor_debug_lines_fs"),
    )?;

    let layout = VertexLayout::new(
        32,
        vec![
            VertexAttribute::new(0, 0, VertexFormat::Float32x4),
            VertexAttribute::new(1, 16, VertexFormat::Float32x4),
        ],
    );

    let bgl = r.create_bind_group_layout(
        BindGroupLayoutDesc::new(vec![BindingKind::UniformBuffer])
            .with_label("editor_debug_lines_bgl"),
    )?;
    let ubo = r.create_buffer(
        BufferDesc::new(
            DEBUG_LINE_UBO_SIZE,
            BufferUsage::Uniform,
            MemoryHint::CpuToGpu,
        )
            .with_label("editor_debug_lines_ubo"),
    )?;
    r.write_buffer(ubo, 0, &[0u8; DEBUG_LINE_UBO_SIZE as usize])?;
    let bg = r.create_bind_group(
        BindGroupDesc::new(bgl)
            .with_label("editor_debug_lines_bg")
            .with_uniform0(BufferBinding::new(ubo, 0, DEBUG_LINE_UBO_SIZE)),
    )?;

    let pipeline = r.create_pipeline(
        PipelineDesc::new(vs, fs, TextureFormat::Bgra8Unorm)
            .with_label("editor_debug_lines_pipeline")
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
            .with_label("editor_debug_lines_vb"),
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

