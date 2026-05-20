#version 450
#include "water_common.glsl"

layout(set = 0, binding = 0) uniform sampler2D u_reflection;
layout(set = 0, binding = 1) uniform sampler2D u_refraction;
layout(set = 0, binding = 2) uniform sampler2D u_depth;
layout(set = 0, binding = 3) uniform sampler2D u_normal_noise;

layout(push_constant) uniform WaterSurfacePush {
    mat4 world_view_proj;
    vec4 time_reflection;
    vec4 normal_params;
    vec4 material;
} pc;

layout(location = 0) in vec2 v_uv;
layout(location = 1) in vec3 v_world_normal;
layout(location = 2) in vec4 v_params;
layout(location = 0) out vec4 out_color;

void main() {
    float t = pc.time_reflection.x;
    vec2 uv_a = ne_water_scroll(v_uv, pc.normal_params.xy, t, 6.0);
    vec2 uv_b = ne_water_scroll(v_uv, pc.normal_params.zw, t, 11.0);
    vec3 n_a = ne_unpack_normal_xy(texture(u_normal_noise, uv_a).xy, 0.65);
    vec3 n_b = ne_unpack_normal_xy(texture(u_normal_noise, uv_b).xy, 0.35);
    vec3 n = normalize(v_world_normal + n_a + n_b);

    vec2 distortion = n.xy * pc.material.z;
    vec3 refl = texture(u_reflection, v_uv + distortion).rgb;
    vec3 refr = texture(u_refraction, v_uv - distortion * 0.45).rgb;
    float scene_depth = texture(u_depth, v_uv).r;
    float foam_noise = texture(u_normal_noise, uv_a * 0.37).z;
    float foam = ne_shoreline_foam(scene_depth, foam_noise, pc.material.w);

    float fresnel = ne_fresnel_schlick(abs(n.z), pc.material.y, 5.0);
    vec3 water = mix(refr, refl, fresnel);
    water = ne_apply_water_absorption(water, scene_depth * 12.0, vec3(0.74, 0.92, 0.98), vec3(0.03, 0.16, 0.24));
    water = mix(water, vec3(0.95, 0.98, 1.0), foam);
    out_color = vec4(water, clamp(0.45 + fresnel * 0.55, 0.0, 1.0));
}
