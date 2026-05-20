#version 450

// Native NewEngine GBuffer packing contract inspired by the reference Deferred/GBuffer layout:
// RT0 RGB=albedo, A=SSA/Occlusion
// RT1 RGB=encoded normal, A=normal twiddle/validity
// RT2 R=diffuse/spec mix, G=roughness/spec exponent proxy, B=fresnel/metallic, A=shadow
// Depth is written by the depth attachment, not packed into a color target.

#include "gbuffer_common.glsl"

layout(location = 0) in vec3 v_wpos;
layout(location = 1) in vec3 v_wnrm;
layout(location = 2) in vec4 v_base;
layout(location = 3) in vec2 v_uv;
layout(location = 4) in vec4 v_light_clip;

layout(set = 0, binding = 1) uniform sampler2D u_albedo;
layout(set = 0, binding = 2) uniform sampler2D u_metallic_roughness;
layout(set = 0, binding = 3) uniform sampler2D u_normal;
layout(set = 0, binding = 4) uniform sampler2D u_occlusion;

layout(location = 0) out vec4 o_gbuffer_albedo;
layout(location = 1) out vec4 o_gbuffer_normal;
layout(location = 2) out vec4 o_gbuffer_material;

void main() {
    vec4 base = texture(u_albedo, v_uv) * v_base;
    if (base.a < 0.33) {
        discard;
    }

    vec3 normal_sample = texture(u_normal, v_uv).xyz * 2.0 - 1.0;
    vec3 normal = normalize(mix(normalize(v_wnrm), normalize(v_wnrm + normal_sample * 0.35), 0.65));

    vec4 mr = texture(u_metallic_roughness, v_uv);
    float roughness = clamp(mr.g, 0.035, 1.0);
    float metallic = clamp(mr.b, 0.0, 1.0);
    float occlusion = clamp(texture(u_occlusion, v_uv).r, 0.0, 1.0);

    // Placeholder shadow factor: the renderer now owns the MRT contract; shadow resolve can
    // replace this channel with CSM/paraboloid sampling as the shadow passes mature.
    float shadow = 1.0;
    float diffuse_spec_mix = mix(0.25, 1.0, metallic);

    o_gbuffer_albedo = ne_pack_gbuffer_albedo(base.rgb, occlusion);
    o_gbuffer_normal = ne_pack_gbuffer_normal(normal, 1.0);
    o_gbuffer_material = ne_pack_gbuffer_material(diffuse_spec_mix, roughness, metallic, shadow);
}
