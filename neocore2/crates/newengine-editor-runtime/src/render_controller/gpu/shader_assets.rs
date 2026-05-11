use newengine_assets::{wait_ready, AssetAccess, AssetServiceClient};
use newengine_core::render::{Extent2D, ShaderStage};
use newengine_core::{EngineError, EngineResult as CoreResult};
use newengine_plugin_host::default_host_api;

pub(super) fn load_text_asset(rel: &str) -> CoreResult<String> {
    // Hard rule: assets are loaded only through AssetManager/VFS so `.pak` layering works.
    // Renderer-side shader IO stays behind AssetManager/VFS so loose files and `.pak` remain interchangeable.
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

pub(super) const BUILTIN_DEBUG_LINES_VERT: &str = r#"#version 450
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

pub(super) const BUILTIN_DEBUG_LINES_FRAG: &str = r#"#version 450
layout(location = 0) in vec4 v_color;
layout(location = 0) out vec4 o_color;
void main() {
    o_color = v_color;
}
"#;

pub(super) fn compile_glsl(stage: ShaderStage, name: &str, src: &str) -> CoreResult<Vec<u32>> {
    newengine_shader_compiler::compile_glsl_to_spirv(stage, name, "main", src)
        .map_err(|e| EngineError::other(format!("shader compile failed: {e}")))
}

