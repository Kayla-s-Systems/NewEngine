#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    BindGroupDesc, BindGroupLayoutDesc, BindingKind, BufferBinding, BufferDesc, BufferSlice,
    BufferUsage, DrawIndexedArgs, IndexFormat, MemoryHint, PipelineDesc, PrimitiveTopology,
    ShaderDesc, ShaderStage, TextureFormat, VertexAttribute, VertexFormat, VertexLayout,
};
use newengine_core::{EngineError, EngineResult as CoreResult};
use newengine_math::collections::FxHashMap;
use newengine_primitives::{PrimitiveId, PrimitiveRegistry, PrimitiveVertex};

use newengine_assets::{wait_ready, AssetAccess, AssetServiceClient};
use newengine_plugin_host::default_host_api;

use shaderc::{CompileOptions, Compiler, OptimizationLevel, ShaderKind};

fn load_text_asset(rel: &str) -> CoreResult<String> {
    // Hard rule: assets are loaded only through AssetManager/VFS so `.pak` layering works.
    // This codepath is kept for the legacy editor renderer and must not touch the filesystem.
    let assets = AssetServiceClient::new(default_host_api());

    let id = assets
        .load(rel)
        .map_err(|e| EngineError::other(format!("asset.load failed path='{rel}' err='{e}'")))?;

    if let Err(e) = wait_ready(&assets, &id, std::time::Duration::from_secs(2)) {
        if let Some(fallback) = builtin_text_asset(rel) {
            log::warn!("asset not ready, using builtin fallback path='{rel}' err='{e:?}'");
            return Ok(fallback.to_string());
        }
        return Err(EngineError::other(format!(
            "asset not ready path='{rel}' id='{id}' err='{e:?}'"
        )));
    }

    let (_meta, payload) = assets.blob_wire_v1(&id).map_err(|e| {
        EngineError::other(format!("asset.blob_wire_v1 failed path='{rel}' err='{e}'"))
    })?;

    let s = std::str::from_utf8(&payload)
        .map_err(|_| EngineError::other(format!("asset is not utf8 path='{rel}'")))?
        .to_string();

    Ok(s)
}

#[inline]
fn builtin_text_asset(rel: &str) -> Option<&'static str> {
    match rel {
        "shaders/editor_lit_v2.vert" => Some(BUILTIN_EDITOR_LIT_VERT),
        "shaders/editor_lit_v2.frag" => Some(BUILTIN_EDITOR_LIT_FRAG),
        _ => None,
    }
}

// Minimal, robust Vulkan GLSL fallbacks (no shadows).
// Layout matches the std140 comment above and LIT_UBO_SIZE.
const BUILTIN_EDITOR_LIT_VERT: &str = r#"#version 450

layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_nrm;

layout(set = 0, binding = 0, std140) uniform Ubo {
    mat4 u_mvp;
    mat4 u_model;
    vec4 u_base_color;
    vec4 u_emissive;
    vec4 u_ambient;
    vec4 u_dir_dir_intensity;
    vec4 u_dir_color;
    vec4 u_point_pos_range[4];
    vec4 u_point_color_intensity[4];
    vec4 u_point_count_pad;
} ubo;

layout(location = 0) out vec3 v_wpos;
layout(location = 1) out vec3 v_wnrm;
layout(location = 2) out vec4 v_base;

void main() {
    vec4 wpos4 = ubo.u_model * vec4(a_pos, 1.0);
    v_wpos = wpos4.xyz;
    v_wnrm = mat3(ubo.u_model) * a_nrm;
    v_base = ubo.u_base_color;
    gl_Position = ubo.u_mvp * vec4(a_pos, 1.0);
}
"#;

const BUILTIN_EDITOR_LIT_FRAG: &str = r#"#version 450

layout(location = 0) in vec3 v_wpos;
layout(location = 1) in vec3 v_wnrm;
layout(location = 2) in vec4 v_base;

layout(set = 0, binding = 0, std140) uniform Ubo {
    mat4 u_mvp;
    mat4 u_model;
    vec4 u_base_color;
    vec4 u_emissive;
    vec4 u_ambient;
    vec4 u_dir_dir_intensity;
    vec4 u_dir_color;
    vec4 u_point_pos_range[4];
    vec4 u_point_color_intensity[4];
    vec4 u_point_count_pad;
} ubo;

layout(location = 0) out vec4 o_color;

float saturate(float x) { return clamp(x, 0.0, 1.0); }

void main() {
    vec3 N = normalize(v_wnrm);
    vec3 base = v_base.rgb;
    vec3 emissive = ubo.u_emissive.rgb;

    vec3 lit = ubo.u_ambient.rgb * ubo.u_ambient.a;

    // Directional (points from light to scene).
    vec3 Ld = normalize(-ubo.u_dir_dir_intensity.xyz);
    float NdL = max(dot(N, Ld), 0.0);
    lit += ubo.u_dir_color.rgb * (ubo.u_dir_dir_intensity.a * NdL);

    int n = int(ubo.u_point_count_pad.x + 0.5);
    n = clamp(n, 0, 4);
    for (int i = 0; i < n; i++) {
        vec3 P = ubo.u_point_pos_range[i].xyz;
        float range = max(ubo.u_point_pos_range[i].w, 0.001);
        vec3 toL = P - v_wpos;
        float d2 = dot(toL, toL);
        float d = sqrt(max(d2, 1e-6));
        vec3 L = toL / d;
        float att = 1.0 / max(d2, 1e-4);
        float fade = 1.0 - saturate(d / range);
        float NdLp = max(dot(N, L), 0.0);
        vec3 col = ubo.u_point_color_intensity[i].rgb;
        float inten = ubo.u_point_color_intensity[i].a;
        lit += col * (inten * NdLp * att * fade * fade);
    }

    vec3 out_rgb = base * lit + emissive;
    o_color = vec4(out_rgb, v_base.a);
}
"#;

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
// vec4 emissive (16)
// vec4 ambient (16)
// vec4 dir_dir_intensity (16)
// vec4 dir_color (16)
// point lights: 4 * (vec4 pos_range + vec4 color_intensity) = 4 * 32 = 128
// vec4 point_count_pad (16)
// Total: 352 bytes.
pub(super) const LIT_UBO_SIZE: u64 = 352;

#[derive(Clone, Copy)]
pub(super) struct PrimitiveGpu {
    pub vb: newengine_core::render::BufferId,
    pub ib: newengine_core::render::BufferId,
    pub index_count: u32,
}

#[derive(Clone, Copy)]
pub(super) struct DebugLineGpu {
    pub vb: newengine_core::render::BufferId,
    pub ubo: newengine_core::render::BufferId,
    pub bg: newengine_core::render::BindGroupId,
    pub bgl: newengine_core::render::BindGroupLayoutId,
    #[allow(dead_code)]
    pub vs: newengine_core::render::ShaderId,
    #[allow(dead_code)]
    pub fs: newengine_core::render::ShaderId,
    pub pipeline: newengine_core::render::PipelineId,
    pub capacity_vertices: u32,
}

const DEBUG_LINE_UBO_SIZE: u64 = 16;

const BUILTIN_DEBUG_LINES_VERT: &str = r#"#version 450
layout(set = 0, binding = 0, std140) uniform DebugLineUbo {
    vec4 u_pad;
} ubo;

layout(location = 0) in vec4 a_clip_pos;
layout(location = 1) in vec4 a_color;
layout(location = 0) out vec4 v_color;
void main() {
    gl_Position = a_clip_pos + vec4(ubo.u_pad.xyz * 0.0, 0.0);
    v_color = a_color;
}
"#;

const BUILTIN_DEBUG_LINES_FRAG: &str = r#"#version 450
layout(location = 0) in vec4 v_color;
layout(location = 0) out vec4 o_color;
void main() {
    o_color = v_color;
}
"#;

pub(super) fn ensure_lit_pipeline(
    cached: &mut Option<LitPipeline>,
    r: &mut dyn newengine_core::render::RenderApi,
) -> CoreResult<LitPipeline> {
    if let Some(p) = *cached {
        return Ok(p);
    }
    let grid_ubo = r.create_buffer(
        BufferDesc::new(LIT_UBO_SIZE, BufferUsage::Uniform, MemoryHint::CpuToGpu)
            .with_label("editor_grid_ubo"),
    )?;

    let bgl = r.create_bind_group_layout(
        BindGroupLayoutDesc::new(vec![BindingKind::UniformBuffer]).with_label("editor_lit_bgl"),
    )?;
    let grid_bg = r.create_bind_group(
        BindGroupDesc::new(bgl)
            .with_label("editor_grid_bg")
            .with_uniform0(BufferBinding::new(grid_ubo, 0, LIT_UBO_SIZE)),
    )?;

    let compiler = shaderc::Compiler::new()
        .map_err(|e| EngineError::other(format!("shaderc: Compiler: {e}")))?;

    let vs_src = load_text_asset("shaders/editor_lit_v2.vert")?;
    let fs_src = load_text_asset("shaders/editor_lit_v2.frag")?;

    let vs_spv = compile_glsl(&compiler, ShaderKind::Vertex, "editor_lit_v2.vert", &vs_src)?;
    let fs_spv = compile_glsl(&compiler, ShaderKind::Fragment, "editor_lit_v2.frag", &fs_src)?;

    let vs = r.create_shader(
        ShaderDesc::new(ShaderStage::Vertex, "main", vs_spv).with_label("editor_lit_vs"),
    )?;
    let fs = r.create_shader(
        ShaderDesc::new(ShaderStage::Fragment, "main", fs_spv).with_label("editor_lit_fs"),
    )?;

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

    let mut vbytes: Vec<u8> =
        Vec::with_capacity(mesh.vertices.len() * std::mem::size_of::<PrimitiveVertex>());
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
        BufferDesc::new(
            vbytes.len() as u64,
            BufferUsage::Vertex,
            MemoryHint::CpuToGpu,
        )
            .with_label("editor_prim_vb"),
    )?;
    r.write_buffer(vb, 0, &vbytes)?;

    let ib = r.create_buffer(
        BufferDesc::new(
            ibytes.len() as u64,
            BufferUsage::Index,
            MemoryHint::CpuToGpu,
        )
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

    let compiler = shaderc::Compiler::new()
        .map_err(|e| EngineError::other(format!("shaderc: Compiler: {e}")))?;

    let vs_src = load_text_asset("shaders/editor_grid.vert")?;
    let fs_src = load_text_asset("shaders/editor_grid.frag")?;

    let vs_spv = compile_glsl(&compiler, ShaderKind::Vertex, "editor_grid.vert", &vs_src)?;
    let fs_spv = compile_glsl(&compiler, ShaderKind::Fragment, "editor_grid.frag", &fs_src)?;

    let vs = r.create_shader(
        ShaderDesc::new(ShaderStage::Vertex, "main", vs_spv).with_label("editor_grid_vs"),
    )?;
    let fs = r.create_shader(
        ShaderDesc::new(ShaderStage::Fragment, "main", fs_spv).with_label("editor_grid_fs"),
    )?;

    let vb = build_unit_grid_vb(r, params)?;

    let stride = (7 * std::mem::size_of::<f32>()) as u32;
    let layout = VertexLayout::new(
        stride,
        vec![
            VertexAttribute::new(0, 0, VertexFormat::Float32x3),
            VertexAttribute::new(
                1,
                (3 * std::mem::size_of::<f32>()) as u32,
                VertexFormat::Float32x4,
            ),
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
        let col = if is_major {
            params.major_color
        } else {
            params.minor_color
        };
        push([-half, 0.0, z], col);
        push([half, 0.0, z], col);
    }

    // Lines parallel to Z (vary X)
    for i in -half_lines..=half_lines {
        let x = i as f32;
        let is_major = (i.rem_euclid(major_every)) == 0;
        let col = if is_major {
            params.major_color
        } else {
            params.minor_color
        };
        push([x, 0.0, -half], col);
        push([x, 0.0, half], col);
    }

    let vb = r.create_buffer(
        BufferDesc::new(
            bytes.len() as u64,
            BufferUsage::Vertex,
            MemoryHint::CpuToGpu,
        )
            .with_label("editor_grid_vb"),
    )?;

    // Upload vertex data (pos + color). Without this the grid buffer stays zeroed and nothing renders.
    r.write_buffer(vb, 0, &bytes)?;

    Ok(vb)
}

pub(super) fn ensure_debug_line_pipeline(
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
    let compiler = shaderc::Compiler::new()
        .map_err(|e| EngineError::other(format!("shaderc: Compiler: {e}")))?;

    let vs_spv = compile_glsl(&compiler, ShaderKind::Vertex, "editor_debug_lines.vert", BUILTIN_DEBUG_LINES_VERT)?;
    let fs_spv = compile_glsl(&compiler, ShaderKind::Fragment, "editor_debug_lines.frag", BUILTIN_DEBUG_LINES_FRAG)?;

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
