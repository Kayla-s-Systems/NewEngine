#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    BindGroupDesc, BindGroupLayoutDesc, BindingKind, BufferBinding, BufferDesc, BufferSlice,
    BufferUsage, DrawIndexedArgs, IndexFormat, MemoryHint, PipelineDesc,
    PrimitiveTopology, ShaderDesc, ShaderStage, TextureFormat, VertexAttribute, VertexFormat,
    VertexLayout,
};
use newengine_core::{EngineError, EngineResult as CoreResult};
use newengine_math::collections::FxHashMap;
use newengine_primitives::{PrimitiveId, PrimitiveRegistry, PrimitiveVertex};

use shaderc::{CompileOptions, Compiler, OptimizationLevel, ShaderKind};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn assets_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        if let Ok(exe) = std::env::current_exe() {
            let mut cur = exe.parent().map(Path::to_path_buf);
            for _ in 0..12 {
                if let Some(dir) = cur.as_ref() {
                    let cand = dir.join("assets");
                    if cand.is_dir() {
                        return cand;
                    }
                    cur = dir.parent().map(Path::to_path_buf);
                } else {
                    break;
                }
            }
        }
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("assets")
    })
}

fn load_text_asset(rel: &str) -> CoreResult<String> {
    let path = assets_root().join(rel);
    let s = std::fs::read_to_string(&path).map_err(|e| {
        EngineError::other(format!("asset.read failed rel='{rel}' path='{}' err='{e}'", path.display()))
    })?;
    log::debug!("asset.read ok kind=text rel='{rel}' path='{}' bytes={}", path.display(), s.len());
    Ok(s)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct GridMeshParams {
    pub half_lines: i32,
    pub major_every: i32,
    pub minor_color: [f32; 4],
    pub major_color: [f32; 4],
}

#[derive(Clone, Copy)]
pub(super) struct GridGpu {
    pub vb: newengine_core::render::BufferId,
    #[allow(dead_code)]
    pub vs: newengine_core::render::ShaderId,
    #[allow(dead_code)]
    pub fs: newengine_core::render::ShaderId,
    pub pipeline: newengine_core::render::PipelineId,
    pub vertex_count: u32,
    pub params: GridMeshParams,
}

#[derive(Clone, Copy)]
pub(super) struct LitPipeline {
    pub ubo: newengine_core::render::BufferId,
    pub bg: newengine_core::render::BindGroupId,
    /// Dedicated UBO for grid pass to avoid per-draw UBO overwrite hazards.
    pub grid_ubo: newengine_core::render::BufferId,
    pub grid_bg: newengine_core::render::BindGroupId,
    pub bgl: newengine_core::render::BindGroupLayoutId,
    #[allow(dead_code)]
    pub vs: newengine_core::render::ShaderId,
    #[allow(dead_code)]
    pub fs: newengine_core::render::ShaderId,
    pub pipeline: newengine_core::render::PipelineId,
}

// std140 layout (see assets/shaders/editor_lit.*):
// mat4 mvp (64)
// mat4 model (64)
// vec4 base_color (16)
// vec4 ambient (16)
// vec4 dir_dir_intensity (16)
// vec4 dir_color (16)
// point lights: 4 * (vec4 pos_range + vec4 color_intensity) = 4 * 32 = 128
// vec4 point_count_pad (16)
// Total: 336 bytes.
const LIT_UBO_SIZE: u64 = 336;

#[derive(Clone, Copy)]
pub(super) struct PrimitiveGpu {
    pub vb: newengine_core::render::BufferId,
    pub ib: newengine_core::render::BufferId,
    pub index_count: u32,
}

pub(super) fn ensure_lit_pipeline(
    cached: &mut Option<LitPipeline>,
    r: &mut dyn newengine_core::render::RenderApi,
) -> CoreResult<LitPipeline> {
    if let Some(p) = *cached {
        return Ok(p);
    }

    let ubo = r.create_buffer(
        BufferDesc::new(LIT_UBO_SIZE, BufferUsage::Uniform, MemoryHint::CpuToGpu)
            .with_label("editor_lit_ubo"),
    )?;
    let grid_ubo = r.create_buffer(
        BufferDesc::new(LIT_UBO_SIZE, BufferUsage::Uniform, MemoryHint::CpuToGpu)
            .with_label("editor_grid_ubo"),
    )?;

    let bgl = r.create_bind_group_layout(
        BindGroupLayoutDesc::new(vec![BindingKind::UniformBuffer]).with_label("editor_lit_bgl"),
    )?;
    let bg = r.create_bind_group(
        BindGroupDesc::new(bgl)
            .with_label("editor_lit_bg")
            .with_uniform0(BufferBinding::new(ubo, 0, LIT_UBO_SIZE)),
    )?;
    let grid_bg = r.create_bind_group(
        BindGroupDesc::new(bgl)
            .with_label("editor_grid_bg")
            .with_uniform0(BufferBinding::new(grid_ubo, 0, LIT_UBO_SIZE)),
    )?;

    let compiler = shaderc::Compiler::new().map_err(|e| EngineError::other(format!("shaderc: Compiler: {e}")))?;

    let vs_src = load_text_asset("shaders/editor_lit.vert")?;
    let fs_src = load_text_asset("shaders/editor_lit.frag")?;

    let vs_spv = compile_glsl(&compiler, ShaderKind::Vertex, "editor_lit.vert", &vs_src)?;
    let fs_spv = compile_glsl(&compiler, ShaderKind::Fragment, "editor_lit.frag", &fs_src)?;

    let vs = r.create_shader(ShaderDesc::new(ShaderStage::Vertex, "main", vs_spv).with_label("editor_lit_vs"))?;
    let fs = r.create_shader(ShaderDesc::new(ShaderStage::Fragment, "main", fs_spv).with_label("editor_lit_fs"))?;

    let stride = std::mem::size_of::<PrimitiveVertex>() as u32;
    let layout = VertexLayout::new(
        stride,
        vec![
            VertexAttribute::new(0, 0, VertexFormat::Float32x3),
            VertexAttribute::new(1, 12, VertexFormat::Float32x3),
        ],
    );

    let pipeline = r.create_pipeline(
        PipelineDesc::new(vs, fs, TextureFormat::Bgra8Unorm)
            .with_label("editor_lit_pipeline")
            .with_topology(PrimitiveTopology::TriangleList)
            .with_vertex_layouts(vec![layout])
            .with_bind_group_layouts(vec![bgl])
            .with_depth(TextureFormat::Depth32Float),
    )?;

    let p = LitPipeline {
        ubo,
        bg,
        grid_ubo,
        grid_bg,
        bgl,
        vs,
        fs,
        pipeline,
    };

    *cached = Some(p);
    Ok(p)
}

pub(super) fn ensure_primitive_gpu(
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

    let mut vbytes: Vec<u8> = Vec::with_capacity(mesh.vertices.len() * std::mem::size_of::<PrimitiveVertex>());
    for v in &mesh.vertices {
        vbytes.extend_from_slice(&v.pos[0].to_ne_bytes());
        vbytes.extend_from_slice(&v.pos[1].to_ne_bytes());
        vbytes.extend_from_slice(&v.pos[2].to_ne_bytes());
        vbytes.extend_from_slice(&v.nrm[0].to_ne_bytes());
        vbytes.extend_from_slice(&v.nrm[1].to_ne_bytes());
        vbytes.extend_from_slice(&v.nrm[2].to_ne_bytes());
    }

    let mut ibytes: Vec<u8> = Vec::with_capacity(mesh.indices.len() * 4);
    for i in &mesh.indices {
        ibytes.extend_from_slice(&i.to_ne_bytes());
    }

    let vb = r.create_buffer(
        BufferDesc::new(vbytes.len() as u64, BufferUsage::Vertex, MemoryHint::CpuToGpu)
            .with_label("editor_prim_vb"),
    )?;
    r.write_buffer(vb, 0, &vbytes)?;

    let ib = r.create_buffer(
        BufferDesc::new(ibytes.len() as u64, BufferUsage::Index, MemoryHint::CpuToGpu)
            .with_label("editor_prim_ib"),
    )?;
    r.write_buffer(ib, 0, &ibytes)?;

    let gpu = PrimitiveGpu {
        vb,
        ib,
        index_count: mesh.indices.len() as u32,
    };

    cache.insert(id, gpu);
    Ok(gpu)
}


pub(super) fn ensure_grid(
    cached: &mut Option<GridGpu>,
    r: &mut dyn newengine_core::render::RenderApi,
    bgl: newengine_core::render::BindGroupLayoutId,
    params: GridMeshParams,
) -> CoreResult<GridGpu> {
    if let Some(g) = *cached {
        if g.params == params {
            return Ok(g);
        }
    }

    let compiler = shaderc::Compiler::new().map_err(|e| EngineError::other(format!("shaderc: Compiler: {e}")))?;

    let vs_src = load_text_asset("shaders/editor_grid.vert")?;
    let fs_src = load_text_asset("shaders/editor_grid.frag")?;

    let vs_spv = compile_glsl(&compiler, ShaderKind::Vertex, "editor_grid.vert", &vs_src)?;
    let fs_spv = compile_glsl(&compiler, ShaderKind::Fragment, "editor_grid.frag", &fs_src)?;

    let vs = r.create_shader(ShaderDesc::new(ShaderStage::Vertex, "main", vs_spv).with_label("editor_grid_vs"))?;
    let fs = r.create_shader(ShaderDesc::new(ShaderStage::Fragment, "main", fs_spv).with_label("editor_grid_fs"))?;

    let vb = build_unit_grid_vb(r, params)?;

    let stride = (7 * std::mem::size_of::<f32>()) as u32;
    let layout = VertexLayout::new(
        stride,
        vec![
            VertexAttribute::new(0, 0, VertexFormat::Float32x3),
            VertexAttribute::new(1, (3 * std::mem::size_of::<f32>()) as u32, VertexFormat::Float32x4),
        ],
    );

    let pipeline = r.create_pipeline(
        PipelineDesc::new(vs, fs, TextureFormat::Bgra8Unorm)
            .with_label("editor_grid_pipeline")
            .with_topology(PrimitiveTopology::LineList)
            .with_vertex_layouts(vec![layout])
            .with_bind_group_layouts(vec![bgl])
            .with_depth(TextureFormat::Depth32Float),
    )?;

    let vertex_count = unit_grid_vertex_count(params.half_lines.max(1));

    let g = GridGpu {
        vb,
        vs,
        fs,
        pipeline,
        vertex_count,
        params,
    };

    *cached = Some(g);
    Ok(g)
}

fn unit_grid_vertex_count(half_lines: i32) -> u32 {
    let per_axis = (2 * half_lines + 1) as u32;
    2 * per_axis * 2
}


fn build_unit_grid_vb(
    r: &mut dyn newengine_core::render::RenderApi,
    params: GridMeshParams,
) -> CoreResult<newengine_core::render::BufferId> {
    let half_lines = params.half_lines.max(1);
    let major_every = params.major_every.max(1);

    let half = half_lines as f32;
    let vertex_count = unit_grid_vertex_count(half_lines) as usize;

    let mut bytes: Vec<u8> = Vec::with_capacity(vertex_count * (7 * 4));

    let mut push = |p: [f32; 3], c: [f32; 4]| {
        bytes.extend_from_slice(&p[0].to_ne_bytes());
        bytes.extend_from_slice(&p[1].to_ne_bytes());
        bytes.extend_from_slice(&p[2].to_ne_bytes());
        bytes.extend_from_slice(&c[0].to_ne_bytes());
        bytes.extend_from_slice(&c[1].to_ne_bytes());
        bytes.extend_from_slice(&c[2].to_ne_bytes());
        bytes.extend_from_slice(&c[3].to_ne_bytes());
    };

    // Lines parallel to X (vary Z)
    for i in -half_lines..=half_lines {
        let z = i as f32;
        let is_major = (i.rem_euclid(major_every)) == 0;
        let col = if is_major { params.major_color } else { params.minor_color };
        push([-half, 0.0, z], col);
        push([half, 0.0, z], col);
    }

    // Lines parallel to Z (vary X)
    for i in -half_lines..=half_lines {
        let x = i as f32;
        let is_major = (i.rem_euclid(major_every)) == 0;
        let col = if is_major { params.major_color } else { params.minor_color };
        push([x, 0.0, -half], col);
        push([x, 0.0, half], col);
    }

    let vb = r.create_buffer(
        BufferDesc::new(bytes.len() as u64, BufferUsage::Vertex, MemoryHint::CpuToGpu)
            .with_label("editor_grid_vb"),
    )?;

    // Upload vertex data (pos + color). Without this the grid buffer stays zeroed and nothing renders.
    r.write_buffer(vb, 0, &bytes)?;

    Ok(vb)
}


fn compile_glsl(
    compiler: &Compiler,
    kind: ShaderKind,
    name: &str,
    src: &str,
) -> CoreResult<Vec<u32>> {
    let mut opts = CompileOptions::new()
        .map_err(|e| EngineError::other(format!("shaderc: CompileOptions: {e}")))?;
    opts.set_optimization_level(OptimizationLevel::Performance);

    let bin = compiler
        .compile_into_spirv(src, kind, name, "main", Some(&opts))
        .map_err(|e| EngineError::other(format!("shaderc: {name}: {e}")))?;

    Ok(bin.as_binary().to_vec())
}

#[allow(dead_code)]
pub(super) fn draw_primitive_indexed(
    r: &mut dyn newengine_core::render::RenderApi,
    gpu: PrimitiveGpu,
) -> CoreResult<()> {
    r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
    r.set_index_buffer(BufferSlice::new(gpu.ib, 0), IndexFormat::U32)?;
    r.draw_indexed(DrawIndexedArgs::new(gpu.index_count))?;
    Ok(())
}
