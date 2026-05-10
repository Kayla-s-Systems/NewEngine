#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    AddressMode, BindGroupDesc, BindGroupLayoutDesc, BindingKind, BufferBinding, BufferDesc,
    BufferSlice, BufferUsage, DrawIndexedArgs, Extent2D, IndexFormat, MemoryHint, PipelineDesc,
    PrimitiveTopology, RasterCullMode, SamplerDesc, ShaderDesc, ShaderStage, TextureDesc,
    TextureFormat, TextureUsage, VertexAttribute, VertexFormat, VertexLayout,
};
use newengine_core::{EngineError, EngineResult as CoreResult};
use newengine_math::collections::FxHashMap;
use newengine_primitives::{PrimitiveId, PrimitiveMesh, PrimitiveRegistry, PrimitiveVertex};

use newengine_assets::{wait_ready, AssetAccess, AssetServiceClient};
use newengine_plugin_host::default_host_api;


fn load_text_asset(rel: &str) -> CoreResult<String> {
    // Hard rule: assets are loaded only through AssetManager/VFS so `.pak` layering works.
    // This codepath is kept for the legacy editor renderer and must not touch the filesystem.
    let assets = AssetServiceClient::new(default_host_api());

    let id = match assets.load(rel) {
        Ok(id) => id,
        Err(e) => {
            if let Some(fallback) = builtin_text_asset(rel) {
                log::warn!("asset.load failed, using builtin fallback path='{rel}' err='{e}'");
                return Ok(fallback.to_string());
            }
            return Err(EngineError::other(format!(
                "asset.load failed path='{rel}' err='{e}'"
            )));
        }
    };

    if let Err(e) = wait_ready(&assets, &id, std::time::Duration::from_secs(2)) {
        if let Some(fallback) = builtin_text_asset(rel) {
            log::warn!("asset not ready, using builtin fallback path='{rel}' err='{e:?}'");
            return Ok(fallback.to_string());
        }
        return Err(EngineError::other(format!(
            "asset not ready path='{rel}' id='{id}' err='{e:?}'"
        )));
    }

    let (_meta, payload) = match assets.blob_wire_v1(&id) {
        Ok(v) => v,
        Err(e) => {
            if let Some(fallback) = builtin_text_asset(rel) {
                log::warn!("asset.blob_wire_v1 failed, using builtin fallback path='{rel}' err='{e}'");
                return Ok(fallback.to_string());
            }
            return Err(EngineError::other(format!(
                "asset.blob_wire_v1 failed path='{rel}' err='{e}'"
            )));
        }
    };

    let s = std::str::from_utf8(&payload)
        .map_err(|_| EngineError::other(format!("asset is not utf8 path='{rel}'")))?
        .to_string();

    Ok(s)
}


#[cfg(feature = "texture-decode")]
pub(super) fn load_rgba_texture_asset(rel: &str) -> CoreResult<(Extent2D, Vec<u8>)> {
    let assets = AssetServiceClient::new(default_host_api());
    let id = assets.load(rel).map_err(|e| EngineError::other(format!("asset.load failed path='{rel}' err='{e}'")))?;
    wait_ready(&assets, &id, std::time::Duration::from_secs(3))
        .map_err(|e| EngineError::other(format!("asset not ready path='{rel}' err='{e:?}'")))?;
    let (_meta, payload) = assets
        .blob_wire_v1(&id)
        .map_err(|e| EngineError::other(format!("asset.blob_wire_v1 failed path='{rel}' err='{e}'")))?;
    let dyn_img = image::load_from_memory(&payload)
        .map_err(|e| EngineError::other(format!("image decode failed path='{rel}' err='{e}'")))?;
    let rgba = dyn_img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok((Extent2D::new(w, h), rgba.into_raw()))
}

#[cfg(not(feature = "texture-decode"))]
pub(super) fn load_rgba_texture_asset(rel: &str) -> CoreResult<(Extent2D, Vec<u8>)> {
    Err(EngineError::other(format!(
        "texture decode requested in runtime-core for '{rel}', but image decoding must go through AssetManager/imageImporter"
    )))
}
#[inline]
fn builtin_text_asset(rel: &str) -> Option<&'static str> {
    match rel {
        "shaders/editor_lit_shadowed_v3.vert" => Some(BUILTIN_EDITOR_LIT_VERT),
        "shaders/editor_lit_shadowed_v3.frag" => Some(BUILTIN_EDITOR_LIT_FRAG),
        "shaders/editor_shadow_depth_v1.vert" => Some(BUILTIN_EDITOR_SHADOW_DEPTH_VERT),
        "shaders/editor_shadow_depth_v1.frag" => Some(BUILTIN_EDITOR_SHADOW_DEPTH_FRAG),
        "shaders/editor_lit_v2.vert" => Some(BUILTIN_EDITOR_LIT_VERT),
        "shaders/editor_lit_v2.frag" => Some(BUILTIN_EDITOR_LIT_FRAG),
        "shaders/editor_grid.vert" => Some(BUILTIN_EDITOR_GRID_VERT),
        "shaders/editor_grid.frag" => Some(BUILTIN_EDITOR_GRID_FRAG),
        _ => None,
    }
}

// Minimal, robust Vulkan GLSL fallbacks (no shadows).
// Layout matches the std140 comment above and LIT_UBO_SIZE.
const BUILTIN_EDITOR_LIT_VERT: &str = r#"#version 450

layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_nrm;
layout(location = 2) in vec2 a_uv;

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
    vec4 u_uv_transform;
    vec4 u_material_params;
    mat4 u_light_mvp;
    vec4 u_shadow_params;
} ubo;

layout(location = 0) out vec3 v_wpos;
layout(location = 1) out vec3 v_wnrm;
layout(location = 2) out vec4 v_base;
layout(location = 3) out vec2 v_uv;
layout(location = 4) out vec4 v_light_clip;

void main() {
    vec4 wpos4 = ubo.u_model * vec4(a_pos, 1.0);
    v_wpos = wpos4.xyz;
    v_wnrm = mat3(ubo.u_model) * a_nrm;
    v_base = ubo.u_base_color;
    v_uv = a_uv * ubo.u_uv_transform.xy + ubo.u_uv_transform.zw;
    v_light_clip = ubo.u_light_mvp * wpos4;
    gl_Position = ubo.u_mvp * vec4(a_pos, 1.0);
}
"#;

const BUILTIN_EDITOR_LIT_FRAG: &str = r#"#version 450

layout(location = 0) in vec3 v_wpos;
layout(location = 1) in vec3 v_wnrm;
layout(location = 2) in vec4 v_base;
layout(location = 3) in vec2 v_uv;
layout(location = 4) in vec4 v_light_clip;

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
    vec4 u_uv_transform;
    vec4 u_material_params;
    mat4 u_light_mvp;
    vec4 u_shadow_params;
} ubo;
layout(set = 0, binding = 1) uniform texture2D u_base_tex;
layout(set = 0, binding = 2) uniform texture2D u_normal_tex;
layout(set = 0, binding = 3) uniform texture2D u_roughness_tex;
layout(set = 0, binding = 4) uniform texture2D u_shadow_tex;
layout(set = 0, binding = 5) uniform sampler u_material_sampler;

layout(location = 0) out vec4 o_color;

mat3 cotangent_frame(vec3 n, vec3 p, vec2 uv) {
    vec3 dp1 = dFdx(p);
    vec3 dp2 = dFdy(p);
    vec2 duv1 = dFdx(uv);
    vec2 duv2 = dFdy(uv);
    vec3 dp2perp = cross(dp2, n);
    vec3 dp1perp = cross(n, dp1);
    vec3 t = dp2perp * duv1.x + dp1perp * duv2.x;
    vec3 b = dp2perp * duv1.y + dp1perp * duv2.y;
    float invmax = inversesqrt(max(dot(t, t), dot(b, b)) + 1.0e-8);
    return mat3(t * invmax, b * invmax, n);
}

float sample_shadow(vec4 light_clip, vec3 nrm, vec3 light_dir_to_scene) {
    if (ubo.u_shadow_params.x < 0.5 || light_clip.w <= 0.0) {
        return 1.0;
    }

    vec3 ndc = light_clip.xyz / light_clip.w;
    vec2 uv = ndc.xy * 0.5 + 0.5;
    if (uv.x < 0.001 || uv.x > 0.999 || uv.y < 0.001 || uv.y > 0.999) {
        return 1.0;
    }

    float current = ndc.z;
    if (current < 0.0 || current > 1.0) {
        return 1.0;
    }

    float slope = 1.0 - max(dot(normalize(nrm), normalize(-light_dir_to_scene)), 0.0);
    float bias = ubo.u_shadow_params.y + slope * ubo.u_shadow_params.w;
    float strength = clamp(ubo.u_shadow_params.z, 0.0, 1.0);

    ivec2 sz = textureSize(sampler2D(u_shadow_tex, u_material_sampler), 0);
    vec2 texel = 1.0 / vec2(max(sz.x, 1), max(sz.y, 1));
    float lit = 0.0;
    for (int y = -1; y <= 1; ++y) {
        for (int x = -1; x <= 1; ++x) {
            float closest = texture(sampler2D(u_shadow_tex, u_material_sampler), uv + vec2(x, y) * texel).r;
            lit += (current - bias <= closest) ? 1.0 : 0.0;
        }
    }
    lit /= 9.0;
    return mix(1.0 - strength, 1.0, lit);
}

void main() {
    vec3 N = normalize(v_wnrm);
    vec3 map_n = texture(sampler2D(u_normal_tex, u_material_sampler), v_uv).xyz * 2.0 - 1.0;
    mat3 tbn = cotangent_frame(N, v_wpos, v_uv);
    float normal_scale = max(ubo.u_material_params.x, 0.0);
    N = normalize(tbn * vec3(map_n.xy * normal_scale, map_n.z));

    vec4 texel = texture(sampler2D(u_base_tex, u_material_sampler), v_uv);
    float roughness_sample = texture(sampler2D(u_roughness_tex, u_material_sampler), v_uv).r;
    float roughness = clamp(ubo.u_material_params.y * roughness_sample, 0.02, 1.0);
    vec3 base = (v_base * texel).rgb;
    vec3 emissive = ubo.u_emissive.rgb;

    vec3 lit = ubo.u_ambient.rgb * ubo.u_ambient.a;
    vec3 Ld = normalize(-ubo.u_dir_dir_intensity.xyz);
    float NdL = max(dot(N, Ld), 0.0);
    float rough_diffuse = mix(1.12, 0.82, roughness);
    float shadow = sample_shadow(v_light_clip, N, ubo.u_dir_dir_intensity.xyz);
    lit += shadow * NdL * rough_diffuse * ubo.u_dir_color.rgb * ubo.u_dir_dir_intensity.w;

    int point_count = int(ubo.u_point_count_pad.x + 0.5);
    for (int i = 0; i < point_count && i < 4; ++i) {
        vec3 toL = ubo.u_point_pos_range[i].xyz - v_wpos;
        float dist = length(toL);
        float range = max(ubo.u_point_pos_range[i].w, 0.0001);
        vec3 L = toL / max(dist, 0.0001);
        float atten = clamp(1.0 - (dist / range), 0.0, 1.0);
        float ndl = max(dot(N, L), 0.0);
        lit += ndl * atten * rough_diffuse * ubo.u_point_color_intensity[i].rgb * ubo.u_point_color_intensity[i].w;
    }

    vec3 color = base * lit + emissive;
    o_color = vec4(color, v_base.a * texel.a);
}
"#;

const BUILTIN_EDITOR_SHADOW_DEPTH_VERT: &str = r#"#version 450

layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_nrm;
layout(location = 2) in vec2 a_uv;

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
    vec4 u_uv_transform;
    vec4 u_material_params;
    mat4 u_light_mvp;
    vec4 u_shadow_params;
} ubo;

layout(location = 0) out float v_depth;

void main() {
    vec4 clip = ubo.u_mvp * vec4(a_pos, 1.0);
    gl_Position = clip;
    v_depth = clamp(clip.z / max(clip.w, 1.0e-6), 0.0, 1.0);
}
"#;

const BUILTIN_EDITOR_SHADOW_DEPTH_FRAG: &str = r#"#version 450
layout(location = 0) in float v_depth;
layout(location = 0) out vec4 o_color;
void main() {
    o_color = vec4(v_depth, v_depth, v_depth, 1.0);
}
"#;

const BUILTIN_EDITOR_GRID_VERT: &str = r#"#version 450

layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec4 a_color;

layout(set = 0, binding = 0, std140) uniform Ubo {
    mat4 u_mvp;
} ubo;

layout(location = 0) out vec4 v_color;

void main() {
    v_color = a_color;
    gl_Position = ubo.u_mvp * vec4(a_pos, 1.0);
}
"#;

const BUILTIN_EDITOR_GRID_FRAG: &str = r#"#version 450

layout(location = 0) in vec4 v_color;
layout(location = 0) out vec4 o_color;

void main() {
    o_color = v_color;
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
    pub white_texture: newengine_core::render::TextureId,
    pub flat_normal_texture: newengine_core::render::TextureId,
    pub repeat_sampler: newengine_core::render::SamplerId,
    pub clamp_sampler: newengine_core::render::SamplerId,
    #[allow(dead_code)]
    pub vs: newengine_core::render::ShaderId,
    #[allow(dead_code)]
    pub fs: newengine_core::render::ShaderId,
    #[allow(dead_code)]
    pub shadow_vs: newengine_core::render::ShaderId,
    #[allow(dead_code)]
    pub shadow_fs: newengine_core::render::ShaderId,
    pub pipeline: newengine_core::render::PipelineId,
    pub double_sided_pipeline: newengine_core::render::PipelineId,
    pub shadow_pipeline: newengine_core::render::PipelineId,
    pub shadow_double_sided_pipeline: newengine_core::render::PipelineId,
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
// vec4 uv_transform (16)
// vec4 material_params (16)
// mat4 light_mvp (64)
// vec4 shadow_params (16)
// Total: 464 bytes.
pub(super) const LIT_UBO_SIZE: u64 = 464;

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
    let vs_src = load_text_asset("shaders/editor_lit_shadowed_v3.vert")?;
    let fs_src = load_text_asset("shaders/editor_lit_shadowed_v3.frag")?;
    let shadow_vs_src = load_text_asset("shaders/editor_shadow_depth_v1.vert")?;
    let shadow_fs_src = load_text_asset("shaders/editor_shadow_depth_v1.frag")?;

    let vs_spv = compile_glsl(ShaderStage::Vertex, "editor_lit_shadowed_v3.vert", &vs_src)?;
    let fs_spv = compile_glsl(ShaderStage::Fragment, "editor_lit_shadowed_v3.frag", &fs_src)?;
    let shadow_vs_spv = compile_glsl(ShaderStage::Vertex, "editor_shadow_depth_v1.vert", &shadow_vs_src)?;
    let shadow_fs_spv = compile_glsl(ShaderStage::Fragment, "editor_shadow_depth_v1.frag", &shadow_fs_src)?;

    // Allocate GPU resources only after shader baking succeeds. Runtime shader
    // compilation is still optional during startup; a local glslc crash must not
    // leave half-created backend objects before the controller fails soft.
    let grid_ubo = r.create_buffer(
        BufferDesc::new(LIT_UBO_SIZE, BufferUsage::Uniform, MemoryHint::CpuToGpu)
            .with_label("editor_grid_ubo"),
    )?;

    let bgl = r.create_bind_group_layout(
        BindGroupLayoutDesc::new(vec![
            BindingKind::UniformBuffer,
            BindingKind::Texture2D,
            BindingKind::Texture2D,
            BindingKind::Texture2D,
            BindingKind::Texture2D,
            BindingKind::Sampler,
        ])
        .with_label("editor_lit_bgl"),
    )?;
    let white_texture = r.create_texture(
        TextureDesc::new(Extent2D::new(1, 1), TextureFormat::Rgba8Unorm, TextureUsage::Sampled)
            .with_label("editor_white_tex")
            .with_data(vec![255, 255, 255, 255]),
    )?;
    let flat_normal_texture = r.create_texture(
        TextureDesc::new(Extent2D::new(1, 1), TextureFormat::Rgba8Unorm, TextureUsage::Sampled)
            .with_label("editor_flat_normal_tex")
            .with_data(vec![128, 128, 255, 255]),
    )?;
    let repeat_sampler = r.create_sampler(
        SamplerDesc::default()
            .with_label("editor_repeat_sampler")
            .with_address_u(AddressMode::Repeat)
            .with_address_v(AddressMode::Repeat)
            .with_address_w(AddressMode::Repeat),
    )?;
    let clamp_sampler = r.create_sampler(
        SamplerDesc::default()
            .with_label("editor_clamp_sampler")
            .with_address_u(AddressMode::ClampToEdge)
            .with_address_v(AddressMode::ClampToEdge)
            .with_address_w(AddressMode::ClampToEdge),
    )?;
    let grid_bg = r.create_bind_group(
        BindGroupDesc::new(bgl)
            .with_label("editor_grid_bg")
            .with_uniform0(BufferBinding::new(grid_ubo, 0, LIT_UBO_SIZE))
            .with_texture0(white_texture)
            .with_texture1(flat_normal_texture)
            .with_texture2(white_texture)
            .with_texture3(white_texture)
            .with_sampler0(clamp_sampler),
    )?;

    let vs = r.create_shader(
        ShaderDesc::new(ShaderStage::Vertex, "main", vs_spv).with_label("editor_lit_vs"),
    )?;
    let fs = r.create_shader(
        ShaderDesc::new(ShaderStage::Fragment, "main", fs_spv).with_label("editor_lit_fs"),
    )?;
    let shadow_vs = r.create_shader(
        ShaderDesc::new(ShaderStage::Vertex, "main", shadow_vs_spv).with_label("editor_shadow_depth_vs"),
    )?;
    let shadow_fs = r.create_shader(
        ShaderDesc::new(ShaderStage::Fragment, "main", shadow_fs_spv).with_label("editor_shadow_depth_fs"),
    )?;

    let stride = std::mem::size_of::<PrimitiveVertex>() as u32;
    let layout = VertexLayout::new(
        stride,
        vec![
            VertexAttribute::new(0, 0, VertexFormat::Float32x3),
            VertexAttribute::new(1, 12, VertexFormat::Float32x3),
            VertexAttribute::new(2, 24, VertexFormat::Float32x2),
        ],
    );

    let pipeline = r.create_pipeline(
        PipelineDesc::new(vs, fs, TextureFormat::Bgra8Unorm)
            .with_label("editor_lit_pipeline")
            .with_topology(PrimitiveTopology::TriangleList)
            .with_vertex_layouts(vec![layout.clone()])
            .with_bind_group_layouts(vec![bgl])
            .with_depth(TextureFormat::Depth32Float),
    )?;

    let double_sided_pipeline = r.create_pipeline(
        PipelineDesc::new(vs, fs, TextureFormat::Bgra8Unorm)
            .with_label("editor_lit_pipeline_double_sided")
            .with_topology(PrimitiveTopology::TriangleList)
            .with_vertex_layouts(vec![layout.clone()])
            .with_bind_group_layouts(vec![bgl])
            .with_depth(TextureFormat::Depth32Float)
            .with_cull_mode(RasterCullMode::None),
    )?;

    let shadow_pipeline = r.create_pipeline(
        PipelineDesc::new(shadow_vs, shadow_fs, TextureFormat::Bgra8Unorm)
            .with_label("editor_shadow_depth_pipeline")
            .with_topology(PrimitiveTopology::TriangleList)
            .with_vertex_layouts(vec![layout.clone()])
            .with_bind_group_layouts(vec![bgl])
            .with_depth(TextureFormat::Depth32Float),
    )?;

    let shadow_double_sided_pipeline = r.create_pipeline(
        PipelineDesc::new(shadow_vs, shadow_fs, TextureFormat::Bgra8Unorm)
            .with_label("editor_shadow_depth_pipeline_double_sided")
            .with_topology(PrimitiveTopology::TriangleList)
            .with_vertex_layouts(vec![layout])
            .with_bind_group_layouts(vec![bgl])
            .with_depth(TextureFormat::Depth32Float)
            .with_cull_mode(RasterCullMode::None),
    )?;

    let p = LitPipeline {
        grid_ubo,
        grid_bg,
        bgl,
        white_texture,
        flat_normal_texture,
        repeat_sampler,
        clamp_sampler,
        vs,
        fs,
        shadow_vs,
        shadow_fs,
        pipeline,
        double_sided_pipeline,
        shadow_pipeline,
        shadow_double_sided_pipeline,
    };

    *cached = Some(p);
    Ok(p)
}

pub(super) fn upload_primitive_mesh(
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
    let gpu = upload_primitive_mesh(r, &mesh, "editor_prim")?;

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

    let vs_src = load_text_asset("shaders/editor_grid.vert")?;
    let fs_src = load_text_asset("shaders/editor_grid.frag")?;

    let vs_spv = compile_glsl(ShaderStage::Vertex, "editor_grid.vert", &vs_src)?;
    let fs_spv = compile_glsl(ShaderStage::Fragment, "editor_grid.frag", &fs_src)?;

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

    let vs_spv = compile_glsl(ShaderStage::Vertex, "editor_debug_lines.vert", BUILTIN_DEBUG_LINES_VERT)?;
    let fs_spv = compile_glsl(ShaderStage::Fragment, "editor_debug_lines.frag", BUILTIN_DEBUG_LINES_FRAG)?;

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

fn compile_glsl(stage: ShaderStage, name: &str, src: &str) -> CoreResult<Vec<u32>> {
    newengine_shader_compiler::compile_glsl_to_spirv(stage, name, "main", src)
        .map_err(|e| EngineError::other(format!("shader compile failed: {e}")))
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
